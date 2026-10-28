//! Cursor metadata and usage from the installed Cursor Agent CLI plus its
//! owner-only local cache.
//!
//! The CLI is the only client surface used for identity, plan and the model
//! catalog. Allowances are not exposed by the CLI, so they come from Cursor's
//! own dashboard service and only when the host explicitly grants
//! `--allow-usage cursor`. That request uses the session token Cursor itself
//! stored in `state.vscdb`; the token is never logged, serialized, exported or
//! written anywhere.
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
use rusqlite::{Connection, OpenFlags, OptionalExtension, limits::Limit};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use zeroize::Zeroizing;

const MAX_CACHE_DATABASE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const DASHBOARD: &str = "https://api2.cursor.sh/aiserver.v1.DashboardService";
const MAX_HTTP_BODY: usize = 256 * 1024;

fn unavailable() -> DomainError {
    DomainError::new(
        ErrorCode::ProviderUnavailable,
        "Cursor's local account cache is unavailable",
    )
}

fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Cursor returned an unexpected response",
    )
}

fn cached<T>(value: T) -> Observation<T> {
    let mut o = Observation::reported(
        value,
        EvidenceSource::new(
            EvidenceKind::LocalConfig,
            "cursor",
            "Cursor ItemTable cached identity/plan; immutable on-disk snapshot",
        ),
    );
    o.mark_stale();
    o
}

fn inferred<T>(value: T, source: EvidenceSource) -> Observation<T> {
    Observation::Known {
        value,
        source,
        observed_at: Timestamp::now(),
        freshness: Freshness::Fresh,
        verification: Verification::Inferred,
        expires_at: None,
    }
}

struct CacheFields {
    email: Option<String>,
    membership: Option<String>,
    token: Option<Zeroizing<String>>,
}

fn field(db: &Connection, key: &str, max: usize) -> Option<String> {
    db.query_row(
        "SELECT substr(value,1,?) FROM ItemTable WHERE key=?2 LIMIT 1",
        rusqlite::params![max as i64, key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .filter(|s| !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control))
}

fn open_cache(path: &Path) -> Result<Connection> {
    let info = path.symlink_metadata().map_err(|_| unavailable())?;
    if !info.is_file() || info.file_type().is_symlink() || info.len() > MAX_CACHE_DATABASE_BYTES {
        return Err(unavailable());
    }
    let mut url = reqwest::Url::from_file_path(path).map_err(|_| unavailable())?;
    url.query_pairs_mut()
        .append_pair("mode", "ro")
        .append_pair("immutable", "1");
    let db = Connection::open_with_flags(
        url.as_str(),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| unavailable())?;
    db.busy_timeout(Duration::from_millis(200))
        .map_err(|_| unavailable())?;
    db.pragma_update(None, "cache_size", -1024)
        .map_err(|_| unavailable())?;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, 65536);
    db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 4096);
    // Large Cursor databases are common. Require SQLite to use an index on the
    // exact key before reading, so a cache-size allowance never permits a table scan.
    let indexed = {
        let mut plan = db
            .prepare(
                "EXPLAIN QUERY PLAN SELECT substr(value,1,1024) FROM ItemTable WHERE key=?1 LIMIT 1",
            )
            .map_err(|_| unavailable())?;
        let details = plan
            .query_map(["cursorAuth/cachedEmail"], |row| row.get::<_, String>(3))
            .map_err(|_| unavailable())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| unavailable())?;
        details
            .iter()
            .any(|detail| detail.contains("USING INDEX") || detail.contains("USING COVERING INDEX"))
    };
    if !indexed {
        return Err(unavailable());
    }
    Ok(db)
}

fn read_fields(profile_home: &Path) -> Option<CacheFields> {
    let db = open_cache(&profile_home.join("User/globalStorage/state.vscdb")).ok()?;
    let token = field(&db, "cursorAuth/accessToken", 8192).filter(|value| {
        value.len() <= 8192
            && value.split('.').count() == 3
            && value.bytes().all(|b| b.is_ascii_graphic())
    });
    Some(CacheFields {
        email: field(&db, "cursorAuth/cachedEmail", 256),
        membership: field(&db, "cursorAuth/stripeMembershipType", 256),
        token: token.map(Zeroizing::new),
    })
}

fn account_for(connection: &str, email: Option<&str>, source: EvidenceSource) -> Option<Account> {
    email.map(|label| Account {
        id: stable_id("cursor-account", &[connection, label]),
        provider_id: "cursor".into(),
        display_label: Observation::reported(label.to_string(), source.clone()),
        identity_source: Observation::reported("Cursor Agent CLI".into(), source),
        scopes: vec![],
    })
}

pub fn read_cached(
    located: LocatedClient,
    connection: &str,
    route: &str,
) -> Result<ConnectionMetadata> {
    if located.provider_id != "cursor" || !located.profile_home.is_absolute() {
        return Err(unavailable());
    }
    let fields = read_fields(&located.profile_home).ok_or_else(unavailable)?;
    let account = fields.email.map(|label| Account {
        id: stable_id("cached-account", &[connection, &label]),
        provider_id: "cursor".into(),
        display_label: cached(label),
        identity_source: cached("Cursor cached account".into()),
        scopes: vec![],
    });
    Ok(ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: Observation::unknown("NATIVE_VERSION_NOT_PROBED"),
        auth: Observation::unknown("CACHED_ACCOUNT_DOES_NOT_VERIFY_AUTHENTICATION"),
        billing: Observation::unknown("CACHED_PLAN_DOES_NOT_VERIFY_BILLING"),
        plan: fields
            .membership
            .map(|label| {
                cached(Plan {
                    native_id: label.to_ascii_lowercase(),
                    label,
                })
            })
            .unwrap_or_else(|| Observation::unknown("CACHED_PLAN_NOT_REPORTED")),
        account,
        models: vec![],
        providers: vec![],
        additional_routes: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("CURSOR_QUOTA_NOT_READ"),
        diagnostics: vec![
            "UNVERIFIED_LOCAL_CACHE_ONLY".into(),
            "IMMUTABLE_DATABASE_EXCLUDES_LIVE_WAL_UPDATES".into(),
            "NO_SUBSCRIPTION_TOKEN_OR_PRIVATE_ENDPOINT_USED".into(),
        ],
    })
}

/// The agent CLI is the only supported probe executable. It must be the
/// discovery-resolved absolute path, never a PATH lookup performed here.
fn cli(located: &LocatedClient) -> Result<(PathBuf, PathBuf)> {
    if located.provider_id != "cursor" || !located.profile_home.is_absolute() {
        return Err(unavailable());
    }
    let executable = located
        .executable
        .clone()
        .filter(|path| path.is_absolute() && path.is_file())
        .ok_or_else(|| {
            DomainError::new(ErrorCode::RuntimeMissing, "Cursor Agent CLI is unavailable")
        })?;
    Ok((executable, located.profile_home.clone()))
}

async fn run(executable: &Path, cwd: &Path, args: &[&str]) -> Result<String> {
    crate::process::capture(
        crate::process::LaunchSpec {
            executable: executable.to_path_buf(),
            args: args.iter().map(std::ffi::OsString::from).collect(),
            cwd: cwd.to_path_buf(),
            env: Default::default(),
        },
        Duration::from_secs(20),
    )
    .await
}

/// `cursor-agent status` prints `✓ Logged in as user@example`. Anything else is
/// treated as unreported rather than a sign-out.
pub(crate) fn parse_login(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let lower = line.to_ascii_lowercase();
        let index = lower.find("logged in as ")?;
        let email = line[index + "logged in as ".len()..].trim();
        (!email.is_empty() && email.len() <= 256 && !email.chars().any(char::is_control))
            .then(|| email.to_string())
    })
}

pub(crate) fn parse_models(output: &str, route: &str, revision: &str) -> Vec<Model> {
    output
        .lines()
        .filter_map(|line| {
            let (id, name) = line.split_once(" - ")?;
            let id = id.trim();
            let mut name = name.trim();
            let is_default = name.contains("(current, default)");
            if let Some(index) = name.find(" (current") {
                name = name[..index].trim();
            }
            if id.is_empty()
                || id.len() > 128
                || name.is_empty()
                || name.len() > 256
                || id.chars().any(char::is_whitespace)
                || id.chars().any(char::is_control)
            {
                return None;
            }
            Some(Model {
                id: route_model_key(route, id),
                route_id: route.to_string(),
                native_id: id.to_string(),
                name: name.to_string(),
                revision: revision.to_string(),
                is_default,
                capabilities: vec![],
                options: vec![],
                context_window: Observation::unknown("CURSOR_CONTEXT_WINDOW_NOT_REPORTED"),
            })
        })
        .take(512)
        .collect()
}

pub(crate) fn parse_tier(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(char::is_whitespace)?;
        (key.trim().eq_ignore_ascii_case("Subscription")
            && value.trim_start().to_ascii_lowercase().starts_with("tier"))
        .then(|| {
            value
                .trim_start()
                .trim_start_matches(|c: char| c.is_alphabetic())
                .trim()
                .to_string()
        })
        .filter(|tier| !tier.is_empty() && tier.len() <= 64)
    })
}

struct UsageSnapshot {
    total_percent: Option<f64>,
    api_percent: Option<f64>,
    resets_at: Option<Timestamp>,
    period_seconds: Option<u32>,
}

fn percent(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|p| p.is_finite() && (0.0..=1000.0).contains(p))
}

fn epoch_ms(value: &Value) -> Option<Timestamp> {
    let millis = value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.parse::<i64>().ok()))?;
    let dt = chrono::DateTime::from_timestamp_millis(millis)?;
    Timestamp::try_from(dt.to_rfc3339()).ok()
}

fn period_seconds(value: Option<&str>) -> Option<u32> {
    match value?.to_ascii_uppercase().as_str() {
        "INCLUDED_USAGE_PERIOD_WEEKLY" => Some(7 * 24 * 3600),
        "INCLUDED_USAGE_PERIOD_MONTHLY" => Some(30 * 24 * 3600),
        "INCLUDED_USAGE_PERIOD_DAILY" => Some(24 * 3600),
        _ => None,
    }
}

async fn dashboard(token: &str, method: &str) -> Result<Value> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| unavailable())?;
    let response = client
        .post(format!("{DASHBOARD}/{method}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .json(&json!({}))
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(unavailable());
    }
    let bytes = response.bytes().await.map_err(|_| unavailable())?;
    if bytes.len() > MAX_HTTP_BODY {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

async fn usage(token: &str) -> Result<UsageSnapshot> {
    let (period, plan) = tokio::join!(
        dashboard(token, "GetCurrentPeriodUsage"),
        dashboard(token, "GetPlanInfo")
    );
    let period = period?;
    let plan = plan?;
    let plan_info = &plan["planInfo"];
    let resets_at = epoch_ms(&plan_info["includedUsageResetsAt"])
        .or_else(|| epoch_ms(&period["billingCycleEnd"]));
    Ok(UsageSnapshot {
        total_percent: percent(&period["planUsage"]["totalPercentUsed"]),
        api_percent: percent(&period["planUsage"]["apiPercentUsed"]),
        resets_at,
        period_seconds: period_seconds(plan_info["includedUsagePeriod"].as_str()),
    })
}

fn percent_reading(percent_used: f64) -> Option<UsageReading> {
    let rounded = (percent_used * 100.0).round() / 100.0;
    let used = DecimalString::try_from(format!("{rounded:.2}")).ok()?;
    UsageReading::try_from(UsageInput {
        unit: "percent".into(),
        used,
        limit: Some(DecimalString::try_from("100".to_owned()).ok()?),
    })
    .ok()
}

fn quota_pool(
    connection: &str,
    route: &str,
    account: Option<&str>,
    snapshot: &UsageSnapshot,
    source: &EvidenceSource,
) -> Option<QuotaPool> {
    let total = snapshot.total_percent?;
    let pool_id = stable_id(
        "quota",
        &[
            connection,
            account.unwrap_or("no-account"),
            "cursor-included",
        ],
    );
    let mut windows = vec![UsageWindow {
        id: stable_id("window", &[&pool_id, "total"]),
        label: "Included usage".into(),
        reading: Observation::reported(percent_reading(total)?, source.clone()),
        resets_at: snapshot.resets_at.clone(),
        duration_seconds: snapshot.period_seconds,
    }];
    if let Some(api) = snapshot.api_percent.filter(|api| *api > 0.0)
        && let Some(reading) = percent_reading(api)
    {
        windows.push(UsageWindow {
            id: stable_id("window", &[&pool_id, "api"]),
            label: "API usage".into(),
            reading: Observation::reported(reading, source.clone()),
            resets_at: snapshot.resets_at.clone(),
            duration_seconds: snapshot.period_seconds,
        });
    }
    Some(QuotaPool {
        id: pool_id,
        account_id: account.map(str::to_owned),
        connection_ids: vec![connection.into()],
        route_ids: vec![route.into()],
        sharing: "unknown".into(),
        windows,
    })
}

pub async fn probe(
    located: LocatedClient,
    connection: &str,
    route: &str,
    allow_usage: bool,
) -> Result<ConnectionMetadata> {
    let (executable, profile_home) = cli(&located)?;
    let cached_fields = {
        let home = profile_home.clone();
        tokio::task::spawn_blocking(move || read_fields(&home))
            .await
            .ok()
            .flatten()
    };
    let status = run(&executable, &profile_home, &["status"]).await.ok();
    let email = status.as_deref().and_then(parse_login);
    let Some(email) = email else {
        // The CLI did not confirm a session. Keep the honest cache view instead
        // of claiming a sign-out we did not observe.
        let mut fallback = read_cached(located, connection, route)?;
        fallback.diagnostics.push("CURSOR_CLI_NOT_SIGNED_IN".into());
        return Ok(fallback);
    };
    let source = EvidenceSource {
        kind: EvidenceKind::OfficialProtocol,
        adapter_id: "cursor".into(),
        adapter_version: env!("CARGO_PKG_VERSION").into(),
        client_version: None,
        reference: Some("cursor-agent status, models, about".into()),
    };
    let (models, about, version) = tokio::join!(
        run(&executable, &profile_home, &["models"]),
        run(&executable, &profile_home, &["about"]),
        run(&executable, &profile_home, &["--version"]),
    );
    let version_text = version
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.len() <= 64);
    let tier = about
        .ok()
        .as_deref()
        .and_then(parse_tier)
        .or_else(|| cached_fields.as_ref().and_then(|f| f.membership.clone()));
    let revision = version_text.clone().unwrap_or_else(|| "unknown".into());
    let models = models
        .map(|output| parse_models(&output, route, &revision))
        .unwrap_or_default();
    let account = account_for(connection, Some(&email), source.clone());
    let account_id = account.as_ref().map(|account| account.id.clone());

    let mut quota_pools = Vec::new();
    let mut diagnostics = vec![
        "CURSOR_CLI_VERIFIED_SESSION".into(),
        "CURSOR_MODEL_CATALOG_FROM_CLIENT".into(),
    ];
    if allow_usage {
        if let Some(token) = cached_fields
            .as_ref()
            .and_then(|fields| fields.token.as_ref())
        {
            match usage(token).await {
                Ok(snapshot) => {
                    let usage_source = EvidenceSource {
                        kind: EvidenceKind::CommunityEndpoint,
                        adapter_id: "cursor".into(),
                        adapter_version: env!("CARGO_PKG_VERSION").into(),
                        client_version: version_text.clone(),
                        reference: Some(
                            "Cursor dashboard service GetCurrentPeriodUsage/GetPlanInfo".into(),
                        ),
                    };
                    if let Some(pool) = quota_pool(
                        connection,
                        route,
                        account_id.as_deref(),
                        &snapshot,
                        &usage_source,
                    ) {
                        quota_pools.push(pool);
                        diagnostics.push("CURSOR_USAGE_FROM_DASHBOARD_SERVICE".into());
                    } else {
                        diagnostics.push("CURSOR_USAGE_PERCENT_NOT_REPORTED".into());
                    }
                }
                Err(_) => diagnostics.push("CURSOR_USAGE_REQUEST_FAILED".into()),
            }
        } else {
            diagnostics.push("CURSOR_SESSION_TOKEN_UNAVAILABLE".into());
        }
    } else {
        diagnostics.push("CURSOR_USAGE_NOT_GRANTED".into());
    }

    let has_usage = !quota_pools.is_empty();
    Ok(ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: version_text
            .clone()
            .map(|value| Observation::reported(value, source.clone()))
            .unwrap_or_else(|| Observation::unknown("CURSOR_CLI_VERSION_NOT_REPORTED")),
        auth: Observation::reported(AuthState::Authenticated, source.clone()),
        billing: inferred(BillingMode::Subscription, source.clone()),
        plan: tier
            .map(|label| {
                Observation::reported(
                    Plan {
                        native_id: label.to_ascii_lowercase().replace(' ', "-"),
                        label,
                    },
                    source.clone(),
                )
            })
            .unwrap_or_else(|| Observation::unknown("CURSOR_PLAN_NOT_REPORTED")),
        account,
        models,
        providers: vec![],
        additional_routes: vec![],
        quota_pools,
        included_usage_allowed: if !has_usage {
            Observation::unknown("CURSOR_INCLUDED_USAGE_NOT_REPORTED")
        } else {
            Observation::reported(true, source)
        },
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_line_is_parsed_without_claiming_a_sign_out() {
        assert_eq!(
            parse_login("✓ Logged in as someone@example.test\n").as_deref(),
            Some("someone@example.test")
        );
        assert_eq!(parse_login("Not logged in"), None);
        assert_eq!(parse_login(""), None);
    }

    #[test]
    fn models_keep_native_ids_and_mark_the_current_default() {
        let output = "Available models\n\nauto - Auto (current, default)\ngpt-5.3-codex-low - Codex 5.3 Low\nnot a model line\n";
        let models = parse_models(output, "route_1", "2026.10.22");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].native_id, "auto");
        assert_eq!(models[0].name, "Auto");
        assert!(models[0].is_default);
        assert_eq!(models[1].native_id, "gpt-5.3-codex-low");
        assert_eq!(models[1].name, "Codex 5.3 Low");
        assert!(!models[1].is_default);
        assert!(models[1].capabilities.is_empty());
    }

    #[test]
    fn tier_line_is_parsed() {
        let about = "About Cursor CLI\n\nCLI Version         2026.10.22\nSubscription Tier   Premium\nUser Email          someone@example.test\n";
        assert_eq!(parse_tier(about).as_deref(), Some("Premium"));
        assert_eq!(parse_tier("no tier here"), None);
    }

    #[test]
    fn usage_percent_is_bounded_and_formatted() {
        let reading = percent_reading(27.670639).unwrap();
        assert!((reading.used_fraction.unwrap() - 0.2767).abs() < 1e-9);
        assert!(percent_reading(f64::NAN).is_none());
    }
}
