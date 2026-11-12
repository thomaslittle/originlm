//! Plan-usage readers for clients whose usage is not part of their main
//! metadata protocol, each through the source the installed client itself
//! uses:
//!
//! - Grok: the ACP `_x.ai/billing` extension (called from the ACP probe).
//! - Antigravity: Google Code Assist `fetchAvailableModels` per-model quota,
//!   authorized by the managed profile's own `acp_token.json` (refreshed in
//!   memory with that file's own client; never written back).
//! - Gemini CLI: Code Assist `retrieveUserQuota` with the access token the CLI
//!   saved in `oauth_creds.json`, only while it is unexpired. The file carries
//!   no refresh client, so an expired login is reported, not refreshed.
//! - GitHub Copilot: `copilot_internal/user` quota snapshots for the GitHub
//!   account signed in to the `gh` CLI (`gh auth token`).
//! - Pi: OpenRouter key usage for an OpenRouter login in Pi's `auth.json`.
//!
//! Tokens stay in memory for one request, are never logged or serialized, and
//! no credential file is modified. Missing pieces become diagnostics, never
//! numbers.
use originlm_contracts::*;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_FILE: u64 = 256 * 1024;
const CODE_ASSIST: &str = "https://cloudcode-pa.googleapis.com/v1internal";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

fn unavailable(message: &str) -> DomainError {
    DomainError::new(ErrorCode::ProviderUnavailable, message)
}

fn source(kind: EvidenceKind, adapter: &str, reference: &str) -> EvidenceSource {
    EvidenceSource::new(kind, adapter, reference)
}

fn timestamp(text: &str) -> Option<Timestamp> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text).ok()?;
    Timestamp::try_from(parsed.with_timezone(&chrono::Utc).to_rfc3339()).ok()
}

fn decimal(value: f64) -> Option<DecimalString> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    DecimalString::try_from(if trimmed.is_empty() { "0" } else { trimmed }.to_owned()).ok()
}

/// A percent-used window. `None` when the provider value is not a percentage.
pub fn percent_window(
    pool_id: &str,
    key: &str,
    label: String,
    used_percent: f64,
    resets_at: Option<Timestamp>,
    duration_seconds: Option<u32>,
    source: &EvidenceSource,
) -> Option<UsageWindow> {
    if !(0.0..=1000.0).contains(&used_percent) {
        return None;
    }
    let reading = UsageReading::try_from(UsageInput {
        unit: "percent".into(),
        used: decimal(used_percent)?,
        limit: Some(DecimalString::try_from("100".to_owned()).ok()?),
    })
    .ok()?;
    Some(UsageWindow {
        id: stable_id("window", &[pool_id, key]),
        label,
        reading: Observation::reported(reading, source.clone()),
        resets_at,
        duration_seconds,
    })
}

fn pool(connection: &str, route: &str, account: Option<String>, key: &str) -> QuotaPool {
    QuotaPool {
        id: stable_id("quota", &[connection, key]),
        account_id: account,
        connection_ids: vec![connection.into()],
        route_ids: vec![route.into()],
        sharing: "unknown".into(),
        windows: vec![],
    }
}

async fn read_json(path: &Path) -> Option<Value> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let metadata = std::fs::metadata(&path).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_FILE {
            return None;
        }
        serde_json::from_slice(&std::fs::read(&path).ok()?).ok()
    })
    .await
    .ok()
    .flatten()
}

fn client() -> Result<reqwest::Client> {
    client_for(None)
}

/// Code Assist answers only requests whose User-Agent names the client the
/// login belongs to. The agent names that client first and OriginLM after it,
/// so the request stays identifiable as OriginLM's.
fn client_for(product: Option<&str>) -> Result<reqwest::Client> {
    let agent = match product {
        Some(product) => format!("{product} originlm/{}", env!("CARGO_PKG_VERSION")),
        None => format!("originlm/{}", env!("CARGO_PKG_VERSION")),
    };
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(agent)
        .build()
        .map_err(|_| unavailable("HTTP client unavailable"))
}

async fn json_response(request: reqwest::RequestBuilder) -> Result<Value> {
    let response = request
        .send()
        .await
        .map_err(|_| unavailable("Usage request failed"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| unavailable("Usage response unreadable"))?;
    if !status.is_success() {
        return Err(unavailable(&format!(
            "Usage request returned HTTP {}",
            status.as_u16()
        )));
    }
    if bytes.len() > MAX_BODY {
        return Err(unavailable("Usage response too large"));
    }
    serde_json::from_slice(&bytes).map_err(|_| unavailable("Usage response is not JSON"))
}

fn account(
    provider: &str,
    connection: &str,
    label: &str,
    how: &str,
    source: &EvidenceSource,
) -> Account {
    Account {
        id: stable_id(&format!("{provider}-account"), &[connection, label]),
        provider_id: provider.into(),
        display_label: Observation::reported(label.to_owned(), source.clone()),
        identity_source: Observation::reported(how.to_owned(), source.clone()),
        scopes: vec![],
    }
}

fn plan(label: &str, source: &EvidenceSource) -> Observation<Plan> {
    Observation::reported(
        Plan {
            native_id: label.to_ascii_lowercase().replace(' ', "-"),
            label: label.to_owned(),
        },
        source.clone(),
    )
}

pub fn empty_metadata(
    connection: &str,
    route: &str,
    diagnostics: Vec<String>,
) -> ConnectionMetadata {
    ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: Observation::unknown("NOT_PROBED"),
        auth: Observation::unknown("AUTH_NOT_REPORTED"),
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        billing: Observation::unknown("BILLING_NOT_REPORTED"),
        account: None,
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("NOT_REPORTED"),
        diagnostics,
    }
}

// ---------------------------------------------------------------- Grok

/// Parse Grok's `_x.ai/billing` result: the credit-usage percentage of the
/// current billing period and the subscription tier.
pub fn grok_billing(
    value: &Value,
    connection: &str,
    route: &str,
) -> (Option<QuotaPool>, Option<Observation<Plan>>) {
    let evidence = source(EvidenceKind::OfficialProtocol, "grok", "ACP _x.ai/billing");
    let config = &value["config"];
    let tier = value["subscription_tier"]
        .as_str()
        .filter(|tier| !tier.is_empty() && tier.len() <= 64)
        .map(|tier| plan(tier, &evidence));
    let Some(used) = config["creditUsagePercent"].as_f64() else {
        return (None, tier);
    };
    let period = &config["currentPeriod"];
    let start = period["start"]
        .as_str()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
    let end_text = period["end"]
        .as_str()
        .or_else(|| config["billingPeriodEnd"].as_str());
    let end = end_text.and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
    let duration = match (start, end) {
        (Some(start), Some(end)) => u32::try_from((end - start).num_seconds()).ok(),
        _ => None,
    };
    let label = match period["type"].as_str() {
        Some("USAGE_PERIOD_TYPE_WEEKLY") => "Weekly credits",
        Some("USAGE_PERIOD_TYPE_MONTHLY") => "Monthly credits",
        Some("USAGE_PERIOD_TYPE_DAILY") => "Daily credits",
        _ => "Credits",
    };
    let mut quota = pool(connection, route, None, "grok-billing");
    if let Some(window) = percent_window(
        &quota.id,
        "credits",
        label.into(),
        used,
        end_text.and_then(timestamp),
        duration,
        &evidence,
    ) {
        quota.windows.push(window);
        (Some(quota), tier)
    } else {
        (None, tier)
    }
}

// ------------------------------------------------- Code Assist (Google)

/// One model's remaining fraction from Code Assist, grouped for display.
pub struct ModelQuota {
    pub name: String,
    pub remaining_fraction: f64,
    pub reset: Option<String>,
}

fn family(name: &str) -> String {
    let first = name
        .split([' ', '-'])
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    match first.as_str() {
        "claude" => "Claude".into(),
        "gemini" => "Gemini".into(),
        "gpt" => "GPT-OSS".into(),
        _ => name.split(' ').next().unwrap_or(name).to_owned(),
    }
}

/// Collapse per-model quotas that share a family, remaining fraction and
/// reset into one window ("Claude models"); split a family only where its
/// models actually differ.
pub fn model_windows(
    pool_id: &str,
    models: &[ModelQuota],
    source: &EvidenceSource,
) -> Vec<UsageWindow> {
    use std::collections::BTreeMap;
    /// Remaining fraction (in millionths) and reset time: models sharing both
    /// share one window.
    type Bucket = (i64, Option<String>);
    let mut families: BTreeMap<String, BTreeMap<Bucket, Vec<&str>>> = BTreeMap::new();
    for model in models {
        if !(0.0..=1.0).contains(&model.remaining_fraction) {
            continue;
        }
        let key = (
            (model.remaining_fraction * 1_000_000.0).round() as i64,
            model.reset.clone(),
        );
        families
            .entry(family(&model.name))
            .or_default()
            .entry(key)
            .or_default()
            .push(&model.name);
    }
    let mut windows = vec![];
    for (family, groups) in families {
        let single = groups.len() == 1;
        for ((fraction, reset), mut names) in groups {
            names.sort_unstable();
            names.dedup();
            let label = if single {
                format!("{family} models")
            } else if names.len() <= 2 {
                names.join(", ")
            } else {
                format!("{}, {} +{}", names[0], names[1], names.len() - 2)
            };
            let used = (1.0 - fraction as f64 / 1_000_000.0) * 100.0;
            if let Some(window) = percent_window(
                pool_id,
                &format!("{family}:{}", names.join(",")),
                label,
                used,
                reset.as_deref().and_then(timestamp),
                None,
                source,
            ) {
                windows.push(window);
            }
        }
    }
    windows
}

fn tier_label(load: &Value) -> Option<String> {
    ["paidTier", "currentTier"]
        .iter()
        .filter_map(|key| load[key]["name"].as_str())
        .find(|name| !name.is_empty() && name.len() <= 64)
        .map(str::to_owned)
}

async fn code_assist(
    client: &reqwest::Client,
    token: &str,
    method: &str,
    body: Value,
) -> Result<Value> {
    json_response(
        client
            .post(format!("{CODE_ASSIST}:{method}"))
            .bearer_auth(token)
            .json(&body),
    )
    .await
}

/// Where a managed Antigravity profile keeps its Google credential.
pub fn antigravity_token_file(profile_home: &Path) -> Option<PathBuf> {
    let mut candidates = vec![
        profile_home.join("acp_token.json"),
        profile_home.join("antigravity-acp").join("acp_token.json"),
    ];
    if let Some(parent) = profile_home.parent() {
        candidates.push(parent.join("antigravity-acp").join("acp_token.json"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// Per-model Antigravity quota for one managed profile.
pub async fn antigravity(
    profile_home: &Path,
    connection: &str,
    route: &str,
) -> (Vec<QuotaPool>, Option<Observation<Plan>>, Vec<String>) {
    let Some(path) = antigravity_token_file(profile_home) else {
        return (
            vec![],
            None,
            vec!["ANTIGRAVITY_CREDENTIAL_NOT_FOUND".into()],
        );
    };
    let Some(file) = read_json(&path).await else {
        return (
            vec![],
            None,
            vec!["ANTIGRAVITY_CREDENTIAL_UNREADABLE".into()],
        );
    };
    let fields = ["client_id", "client_secret", "refresh_token", "project_id"].map(|key| {
        file[key]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 4096)
    });
    let [Some(client_id), Some(secret), Some(refresh), Some(project)] = fields else {
        return (
            vec![],
            None,
            vec!["ANTIGRAVITY_CREDENTIAL_INCOMPLETE".into()],
        );
    };
    // Only Google's token endpoint may receive the client secret.
    if file["token_uri"]
        .as_str()
        .is_some_and(|uri| uri != GOOGLE_TOKEN_URI)
    {
        return (
            vec![],
            None,
            vec!["ANTIGRAVITY_TOKEN_URI_UNEXPECTED".into()],
        );
    }
    let Ok(client) = client_for(Some("antigravity")) else {
        return (
            vec![],
            None,
            vec!["ANTIGRAVITY_USAGE_REQUEST_FAILED".into()],
        );
    };
    let token = json_response(client.post(GOOGLE_TOKEN_URI).form(&[
        ("client_id", client_id),
        ("client_secret", secret),
        ("refresh_token", refresh),
        ("grant_type", "refresh_token"),
    ]))
    .await
    .ok()
    .and_then(|value| {
        value["access_token"]
            .as_str()
            .map(|t| zeroize::Zeroizing::new(t.to_owned()))
    });
    let Some(token) = token else {
        return (
            vec![],
            None,
            vec!["ANTIGRAVITY_LOGIN_REFRESH_FAILED".into()],
        );
    };
    let (models, load) = tokio::join!(
        code_assist(
            &client,
            &token,
            "fetchAvailableModels",
            json!({"project": project})
        ),
        code_assist(
            &client,
            &token,
            "loadCodeAssist",
            json!({"cloudaicompanionProject": project,
                   "metadata": {"ideType": "ANTIGRAVITY", "pluginType": "GEMINI", "duetProject": project}})
        ),
    );
    let evidence = source(
        EvidenceKind::CommunityEndpoint,
        "antigravity",
        "Code Assist fetchAvailableModels quotaInfo",
    );
    let tier = load
        .ok()
        .as_ref()
        .and_then(tier_label)
        .map(|label| plan(&label, &evidence));
    let Ok(models) = models else {
        return (
            vec![],
            tier,
            vec!["ANTIGRAVITY_USAGE_REQUEST_FAILED".into()],
        );
    };
    let quotas: Vec<ModelQuota> = models["models"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, model)| {
            // Internal ids without a display name (tab completion, routing
            // buckets) are not models a person picks; they are left out.
            let name = model["displayName"].as_str().filter(|n| !n.is_empty())?;
            let quota = &model["quotaInfo"];
            Some(ModelQuota {
                name: name.to_owned(),
                remaining_fraction: quota["remainingFraction"].as_f64()?,
                reset: quota["resetTime"].as_str().map(str::to_owned),
            })
        })
        .collect();
    let mut quota = pool(connection, route, None, "antigravity-models");
    quota.windows = model_windows(&quota.id, &quotas, &evidence);
    if quota.windows.is_empty() {
        return (vec![], tier, vec!["ANTIGRAVITY_QUOTA_NOT_REPORTED".into()]);
    }
    (
        vec![quota],
        tier,
        vec!["ANTIGRAVITY_USAGE_FROM_CODE_ASSIST".into()],
    )
}

/// The auth type Gemini CLI is configured with (`settings.json`).
pub fn gemini_auth_type(settings: &Value) -> Option<&str> {
    settings["security"]["auth"]["selectedType"]
        .as_str()
        .or_else(|| settings["selectedAuthType"].as_str())
}

/// Gemini CLI per-model quota, while its saved access token is still valid.
///
/// Only a Google-login profile has a plan quota. With a Gemini API key or
/// Vertex AI the CLI is metered on that API, which publishes no usage
/// endpoint for a key, so that is reported instead of a number.
pub async fn gemini(
    profile_home: &Path,
    connection: &str,
    route: &str,
) -> (Vec<QuotaPool>, Option<Observation<Plan>>, Vec<String>) {
    if let Some(settings) = read_json(&profile_home.join("settings.json")).await {
        match gemini_auth_type(&settings) {
            Some("gemini-api-key") => {
                return (vec![], None, vec!["GEMINI_API_KEY_AUTH_NO_QUOTA".into()]);
            }
            Some("vertex-ai") => {
                return (vec![], None, vec!["GEMINI_VERTEX_AUTH_NO_QUOTA".into()]);
            }
            _ => {}
        }
    }
    let Some(file) = read_json(&profile_home.join("oauth_creds.json")).await else {
        return (vec![], None, vec!["GEMINI_LOGIN_NOT_FOUND".into()]);
    };
    let expiry_ms = file["expiry_date"].as_i64().unwrap_or(0);
    if expiry_ms <= chrono::Utc::now().timestamp_millis() + 60_000 {
        return (vec![], None, vec!["GEMINI_LOGIN_EXPIRED".into()]);
    }
    let Some(token) = file["access_token"]
        .as_str()
        .filter(|t| !t.is_empty() && t.len() <= 4096)
        .map(|t| zeroize::Zeroizing::new(t.to_owned()))
    else {
        return (vec![], None, vec!["GEMINI_LOGIN_NOT_FOUND".into()]);
    };
    let Ok(client) = client_for(Some("GeminiCLI")) else {
        return (vec![], None, vec!["GEMINI_USAGE_REQUEST_FAILED".into()]);
    };
    let evidence = source(
        EvidenceKind::CommunityEndpoint,
        "gemini_cli",
        "Code Assist retrieveUserQuota",
    );
    let Ok(load) = code_assist(
        &client,
        &token,
        "loadCodeAssist",
        json!({"metadata": {"ideType": "IDE_UNSPECIFIED", "platform": "PLATFORM_UNSPECIFIED", "pluginType": "GEMINI"}}),
    )
    .await
    else {
        return (vec![], None, vec!["GEMINI_USAGE_REQUEST_FAILED".into()]);
    };
    let tier = tier_label(&load).map(|label| plan(&label, &evidence));
    let project = load["cloudaicompanionProject"]
        .as_str()
        .or_else(|| load["cloudaicompanionProject"]["id"].as_str());
    let Some(project) = project else {
        return (vec![], tier, vec!["GEMINI_PROJECT_NOT_REPORTED".into()]);
    };
    let Ok(quota_value) = code_assist(
        &client,
        &token,
        "retrieveUserQuota",
        json!({"project": project}),
    )
    .await
    else {
        return (vec![], tier, vec!["GEMINI_USAGE_REQUEST_FAILED".into()]);
    };
    let quotas: Vec<ModelQuota> = quota_value["buckets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|bucket| {
            Some(ModelQuota {
                name: bucket["modelId"].as_str()?.to_owned(),
                remaining_fraction: bucket["remainingFraction"].as_f64()?,
                reset: bucket["resetTime"].as_str().map(str::to_owned),
            })
        })
        .collect();
    let mut quota = pool(connection, route, None, "gemini-models");
    quota.windows = model_windows(&quota.id, &quotas, &evidence);
    if quota.windows.is_empty() {
        return (vec![], tier, vec!["GEMINI_QUOTA_NOT_REPORTED".into()]);
    }
    (
        vec![quota],
        tier,
        vec!["GEMINI_USAGE_FROM_CODE_ASSIST".into()],
    )
}

// ------------------------------------------------------- GitHub Copilot

async fn gh_token() -> Option<zeroize::Zeroizing<String>> {
    let mut command = tokio::process::Command::new("gh");
    command
        .args(["auth", "token"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!token.is_empty() && token.len() <= 512 && !token.contains(char::is_whitespace))
        .then(|| zeroize::Zeroizing::new(token))
}

/// Parse `copilot_internal/user` into plan, account and quota windows.
pub fn copilot_report(value: &Value, connection: &str, route: &str) -> ConnectionMetadata {
    let evidence = source(
        EvidenceKind::CommunityEndpoint,
        "github_copilot",
        "api.github.com/copilot_internal/user",
    );
    let mut diagnostics = vec!["COPILOT_USAGE_FROM_GH_LOGIN".to_owned()];
    let login = value["login"]
        .as_str()
        .filter(|l| !l.is_empty() && l.len() <= 128);
    let account = login.map(|login| {
        account(
            "github_copilot",
            connection,
            login,
            "GitHub CLI login",
            &evidence,
        )
    });
    let sku = value["access_type_sku"].as_str().unwrap_or_default();
    let plan_label = if sku.contains("free") {
        Some("Copilot Free".to_owned())
    } else {
        value["copilot_plan"]
            .as_str()
            .filter(|p| !p.is_empty() && p.len() <= 64)
            .map(|p| format!("Copilot ({p})"))
    };
    let reset = value["quota_reset_date_utc"]
        .as_str()
        .and_then(timestamp)
        .or_else(|| {
            value["quota_reset_date"]
                .as_str()
                .and_then(|d| timestamp(&format!("{d}T00:00:00Z")))
        });
    let mut quota = pool(
        connection,
        route,
        account.as_ref().map(|a| a.id.clone()),
        "copilot",
    );
    for (key, label) in [
        ("premium_interactions", "Premium requests"),
        ("chat", "Chat messages"),
        ("completions", "Code completions"),
    ] {
        let snapshot = &value["quota_snapshots"][key];
        if !snapshot.is_object() {
            continue;
        }
        let code = key.to_ascii_uppercase();
        if snapshot["unlimited"] == true {
            diagnostics.push(format!("COPILOT_{code}_UNLIMITED"));
            continue;
        }
        if snapshot["entitlement"].as_f64().is_some_and(|e| e <= 0.0) {
            diagnostics.push(format!("COPILOT_{code}_NOT_IN_PLAN"));
            continue;
        }
        if let Some(remaining) = snapshot["percent_remaining"]
            .as_f64()
            .filter(|p| (0.0..=100.0).contains(p))
            && let Some(window) = percent_window(
                &quota.id,
                key,
                label.into(),
                100.0 - remaining,
                reset.clone(),
                None,
                &evidence,
            )
        {
            quota.windows.push(window);
        }
    }
    let mut metadata = empty_metadata(connection, route, diagnostics);
    metadata.auth = Observation::reported(AuthState::Authenticated, evidence.clone());
    metadata.plan = plan_label
        .map(|label| plan(&label, &evidence))
        .unwrap_or_else(|| Observation::unknown("PLAN_NOT_REPORTED"));
    metadata.account = account;
    if !quota.windows.is_empty() {
        metadata.quota_pools.push(quota);
    }
    metadata
}

pub async fn copilot(connection: &str, route: &str) -> Result<ConnectionMetadata> {
    let Some(token) = gh_token().await else {
        return Ok(empty_metadata(
            connection,
            route,
            vec!["COPILOT_GH_LOGIN_UNAVAILABLE".into()],
        ));
    };
    let client = client()?;
    match json_response(
        client
            .get("https://api.github.com/copilot_internal/user")
            .header("Authorization", format!("token {}", token.as_str()))
            .header("Accept", "application/json")
            .header("X-GitHub-Api-Version", "2025-04-01"),
    )
    .await
    {
        Ok(value) => Ok(copilot_report(&value, connection, route)),
        Err(_) => Ok(empty_metadata(
            connection,
            route,
            vec!["COPILOT_USAGE_REQUEST_FAILED".into()],
        )),
    }
}

// ------------------------------------------------------------------ Pi

fn diagnostic_name(name: &str) -> String {
    name.chars()
        .take(32)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Parse OpenRouter's `/api/v1/key` result into a dollar-spend window.
pub fn openrouter_window(
    pool_id: &str,
    value: &Value,
    source: &EvidenceSource,
) -> Option<UsageWindow> {
    let data = &value["data"];
    let used = decimal(data["usage"].as_f64()?)?;
    let limit = data["limit"].as_f64().and_then(decimal);
    let reading = UsageReading::try_from(UsageInput {
        unit: "usd".into(),
        used,
        limit,
    })
    .ok()?;
    let label = if data["is_free_tier"] == true {
        "OpenRouter key (free tier)"
    } else {
        "OpenRouter key"
    };
    Some(UsageWindow {
        id: stable_id("window", &[pool_id, "openrouter"]),
        label: label.into(),
        reading: Observation::reported(reading, source.clone()),
        resets_at: None,
        duration_seconds: None,
    })
}

pub async fn pi(profile_home: &Path, connection: &str, route: &str) -> Result<ConnectionMetadata> {
    let Some(auth) = read_json(&profile_home.join("auth.json")).await else {
        return Ok(empty_metadata(
            connection,
            route,
            vec!["PI_NO_SAVED_LOGINS".into()],
        ));
    };
    let Some(entries) = auth.as_object().filter(|entries| !entries.is_empty()) else {
        return Ok(empty_metadata(
            connection,
            route,
            vec!["PI_NO_SAVED_LOGINS".into()],
        ));
    };
    let evidence = source(EvidenceKind::OfficialApi, "pi", "openrouter.ai/api/v1/key");
    let mut diagnostics = vec![];
    let mut quota = pool(connection, route, None, "pi-openrouter");
    for (name, entry) in entries.iter().take(32) {
        if name != "openrouter" {
            // Pi stores the login; usage belongs to that provider's own
            // account and Pi exposes no usage call for it.
            diagnostics.push(format!("PI_{}_NO_USAGE_SOURCE", diagnostic_name(name)));
            continue;
        }
        let Some(key) = entry["access"]
            .as_str()
            .or_else(|| entry["key"].as_str())
            .filter(|k| !k.is_empty() && k.len() <= 512)
        else {
            diagnostics.push("PI_OPENROUTER_KEY_MISSING".into());
            continue;
        };
        let request = client()?
            .get("https://openrouter.ai/api/v1/key")
            .bearer_auth(key);
        match json_response(request).await {
            Ok(value) => match openrouter_window(&quota.id, &value, &evidence) {
                Some(window) => {
                    quota.windows.push(window);
                    diagnostics.push("PI_USAGE_FROM_OPENROUTER".into());
                }
                None => diagnostics.push("PI_OPENROUTER_USAGE_NOT_REPORTED".into()),
            },
            Err(_) => diagnostics.push("PI_OPENROUTER_USAGE_REQUEST_FAILED".into()),
        }
    }
    let mut metadata = empty_metadata(connection, route, diagnostics);
    metadata.auth = Observation::reported(
        AuthState::Authenticated,
        source(EvidenceKind::LocalConfig, "pi", "auth.json"),
    );
    if !quota.windows.is_empty() {
        metadata.quota_pools.push(quota);
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(window: &UsageWindow) -> String {
        match &window.reading {
            Observation::Known { value, .. } => value.used.as_str().to_owned(),
            _ => "unknown".into(),
        }
    }

    #[test]
    fn grok_billing_becomes_a_weekly_credit_window_and_plan() {
        let value = json!({"config":{"creditUsagePercent":3,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY",
            "start":"2026-11-08T18:53:38.284+00:00","end":"2026-11-15T18:53:38.284+00:00"},"prepaidBalance":{"val":0}},
            "subscription_tier":"SuperGrok"});
        let (pool, tier) = grok_billing(&value, "c", "r");
        let pool = pool.unwrap();
        assert_eq!(pool.windows[0].label, "Weekly credits");
        assert_eq!(used(&pool.windows[0]), "3");
        assert_eq!(pool.windows[0].duration_seconds, Some(7 * 86_400));
        assert!(pool.windows[0].resets_at.is_some());
        assert!(
            matches!(tier, Some(Observation::Known { value, .. }) if value.label == "SuperGrok")
        );
        // No percentage: no window, never a zero.
        let (pool, _) = grok_billing(
            &json!({"config":{},"subscription_tier":"SuperGrok"}),
            "c",
            "r",
        );
        assert!(pool.is_none());
    }

    #[test]
    fn model_quotas_group_by_family_and_split_where_they_differ() {
        let evidence = source(EvidenceKind::Synthetic, "t", "t");
        let q = |name: &str, f: f64| ModelQuota {
            name: name.into(),
            remaining_fraction: f,
            reset: Some("2026-11-11T21:55:53Z".into()),
        };
        let windows = model_windows(
            "p",
            &[
                q("Claude Opus 5.5 (High)", 1.0),
                q("Claude Sonnet 5.5 (Low)", 1.0),
                q("Gemini 3 Flash", 0.8),
                q("Gemini 2.5 Pro", 0.25),
                q("GPT-OSS 120B (Medium)", 1.0),
            ],
            &evidence,
        );
        let labels: Vec<_> = windows
            .iter()
            .map(|w| (w.label.as_str(), used(w)))
            .collect();
        assert_eq!(
            labels,
            [
                ("Claude models", "0".into()),
                ("GPT-OSS models", "0".into()),
                ("Gemini 2.5 Pro", "75".into()),
                ("Gemini 3 Flash", "20".into())
            ]
        );
        assert!(model_windows("p", &[q("x", 1.5)], &evidence).is_empty());
    }

    #[test]
    fn copilot_snapshots_become_windows_and_unincluded_quota_is_not_zero() {
        let value = json!({"login":"octo","copilot_plan":"individual","access_type_sku":"free_limited_copilot",
            "quota_reset_date_utc":"2026-12-01T00:00:00.000Z","quota_snapshots":{
              "chat":{"entitlement":200,"remaining":200,"percent_remaining":100.0,"unlimited":false},
              "completions":{"entitlement":2000,"remaining":1996,"percent_remaining":99.8,"unlimited":false},
              "premium_interactions":{"entitlement":0,"remaining":0,"percent_remaining":0.0,"unlimited":false}}});
        let metadata = copilot_report(&value, "c", "r");
        let windows = &metadata.quota_pools[0].windows;
        let rows: Vec<_> = windows
            .iter()
            .map(|w| (w.label.as_str(), used(w)))
            .collect();
        assert_eq!(
            rows,
            [
                ("Chat messages", "0".into()),
                ("Code completions", "0.2".into())
            ]
        );
        assert!(
            metadata
                .diagnostics
                .contains(&"COPILOT_PREMIUM_INTERACTIONS_NOT_IN_PLAN".into())
        );
        assert!(
            matches!(&metadata.plan, Observation::Known { value, .. } if value.label == "Copilot Free")
        );
        assert_eq!(metadata.account.unwrap().provider_id, "github_copilot");
    }

    #[test]
    fn openrouter_spend_without_a_limit_has_no_percentage() {
        let evidence = source(EvidenceKind::Synthetic, "t", "t");
        let window = openrouter_window(
            "p",
            &json!({"data":{"usage":0.166547361,"limit":null,"is_free_tier":true}}),
            &evidence,
        )
        .unwrap();
        assert_eq!(window.label, "OpenRouter key (free tier)");
        let Observation::Known { value, .. } = &window.reading else {
            panic!()
        };
        assert_eq!(value.unit, "usd");
        assert!(value.limit.is_none() && value.used_fraction.is_none());
        let limited =
            openrouter_window("p", &json!({"data":{"usage":2.5,"limit":10}}), &evidence).unwrap();
        let Observation::Known { value, .. } = &limited.reading else {
            panic!()
        };
        assert_eq!(value.used_fraction, Some(0.25));
    }

    #[tokio::test]
    async fn gemini_never_uses_an_expired_login() {
        let home = tempfile::tempdir().unwrap();
        // An API-key profile has no plan quota at all, whatever login is saved.
        std::fs::write(
            home.path().join("settings.json"),
            json!({"security":{"auth":{"selectedType":"gemini-api-key"}}}).to_string(),
        )
        .unwrap();
        let (pools, _, diagnostics) = gemini(home.path(), "c", "r").await;
        assert!(pools.is_empty());
        assert_eq!(diagnostics, ["GEMINI_API_KEY_AUTH_NO_QUOTA"]);
        std::fs::remove_file(home.path().join("settings.json")).unwrap();
        std::fs::write(
            home.path().join("oauth_creds.json"),
            json!({"access_token":"x","expiry_date":1000}).to_string(),
        )
        .unwrap();
        let (pools, _, diagnostics) = gemini(home.path(), "c", "r").await;
        assert!(pools.is_empty());
        assert_eq!(diagnostics, ["GEMINI_LOGIN_EXPIRED"]);
        let (pools, _, diagnostics) = gemini(&home.path().join("missing"), "c", "r").await;
        assert!(pools.is_empty());
        assert_eq!(diagnostics, ["GEMINI_LOGIN_NOT_FOUND"]);
    }

    #[tokio::test]
    async fn antigravity_refuses_a_foreign_token_endpoint() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(
            home.path().join("acp_token.json"),
            json!({"client_id":"a","client_secret":"b","refresh_token":"c","project_id":"d",
                   "token_uri":"https://example.com/token"})
            .to_string(),
        )
        .unwrap();
        let (pools, _, diagnostics) = antigravity(home.path(), "c", "r").await;
        assert!(pools.is_empty());
        assert_eq!(diagnostics, ["ANTIGRAVITY_TOKEN_URI_UNEXPECTED"]);
    }

    #[tokio::test]
    async fn pi_names_logins_without_a_usage_source() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(
            home.path().join("auth.json"),
            json!({"radius":{"type":"oauth"}}).to_string(),
        )
        .unwrap();
        let metadata = pi(home.path(), "c", "r").await.unwrap();
        assert!(metadata.quota_pools.is_empty());
        assert_eq!(metadata.diagnostics, ["PI_RADIUS_NO_USAGE_SOURCE"]);
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(
            pi(empty.path(), "c", "r").await.unwrap().diagnostics,
            ["PI_NO_SAVED_LOGINS"]
        );
    }
}
