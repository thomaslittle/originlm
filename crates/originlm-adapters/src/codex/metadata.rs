use super::{invalid, models, text, usage};
use originlm_contracts::*;
use serde_json::Value;

pub fn parse_metadata(
    connection: &str,
    route: &str,
    initialize: &Value,
    account_response: &Value,
    native_models: &[Value],
    rate_limits: Option<&Value>,
) -> Result<ConnectionMetadata> {
    if connection.is_empty() || route.is_empty() || native_models.len() > 2048 {
        return Err(invalid());
    }
    let mut source = EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        "codex",
        "https://developers.openai.com/codex/app-server/",
    );
    let version = text(&initialize["userAgent"])
        .and_then(|agent| {
            agent
                .split_once('/')
                .map(|(_, v)| v.split_whitespace().next().unwrap_or(""))
        })
        .filter(|v| {
            !v.is_empty()
                && v.len() < 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
        });
    source.client_version = version.map(str::to_owned);
    let mut report = ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: version
            .map(|v| Observation::reported(v.to_owned(), source.clone()))
            .unwrap_or_else(|| Observation::unknown("CLIENT_VERSION_NOT_REPORTED")),
        auth: Observation::unknown("AUTH_NOT_REPORTED"),
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        billing: Observation::unknown("BILLING_NOT_REPORTED"),
        account: None,
        models: Vec::new(),
        additional_routes: vec![],
        providers: vec![],
        quota_pools: Vec::new(),
        included_usage_allowed: Observation::unknown("INCLUDED_USAGE_PERMISSION_NOT_REPORTED"),
        diagnostics: Vec::new(),
    };
    if !account_response["requiresOpenaiAuth"].is_boolean() {
        return Err(invalid());
    }
    let account = &account_response["account"];
    if account.is_null() {
        report.auth = if account_response["requiresOpenaiAuth"] == true {
            Observation::reported(AuthState::Unauthenticated, source)
        } else {
            Observation::unknown("NATIVE_AUTH_CONTEXT_UNAVAILABLE")
        };
        return Ok(report);
    }
    let kind = text(&account["type"]).ok_or_else(invalid)?;
    let billing = match kind {
        "chatgpt" => BillingMode::Subscription,
        "apiKey" | "amazonBedrock" => BillingMode::ApiMetered,
        _ => return Err(invalid()),
    };
    report.auth = Observation::reported(AuthState::Authenticated, source.clone());
    report.billing = Observation::reported(billing, source.clone());
    let email = text(&account["email"]);
    // This is a connection-scoped identity observation, not cross-client account deduplication.
    let identity = stable_id(
        "account",
        &[connection, kind, email.unwrap_or("not-reported")],
    );
    if let Some(email) = email {
        report.account = Some(Account {
            id: identity.clone(),
            provider_id: "openai".into(),
            display_label: Observation::reported(email.into(), source.clone()),
            identity_source: Observation::reported(
                "Connection-scoped native account/read; no cross-client linking".into(),
                source.clone(),
            ),
            scopes: vec![],
        });
    }
    if kind == "chatgpt"
        && let Some(plan) = text(&account["planType"]).filter(|plan| *plan != "unknown")
    {
        report.plan = Observation::reported(
            Plan {
                native_id: plan.into(),
                label: plan_label(plan).into(),
            },
            source.clone(),
        );
    }
    report.models = models::parse_models(route, &identity, native_models, &source)?;
    if let Some(rates) = rate_limits {
        report.quota_pools = usage::parse_usage(
            connection,
            route,
            report.account.as_ref().map(|a| a.id.clone()),
            rates,
            &source,
        )?;
        if let Some(allowed) = rates["ordinaryUsageAllowed"].as_bool() {
            report.included_usage_allowed = Observation::reported(allowed, source.clone());
        }
        if rates.get("rateLimitResetCredits").is_some() {
            report
                .diagnostics
                .push("RESET_CREDITS_NOT_ACTIONABLE_BY_TOOLKIT".into());
        }
    } else {
        report.diagnostics.push("USAGE_UNAVAILABLE".into());
    }
    Ok(report)
}

/// Display names for ChatGPT plan codes Codex reports on `account/read`.
/// The native id stays the code. Unlisted codes keep that code as the label
/// instead of a guessed product name. `unknown` is omitted by the caller.
/// Reimplemented from T3 Code `codexPlanLabel` at
/// [33f1fe5ebe](https://github.com/pingdotgg/t3code/commit/33f1fe5ebe).
fn plan_label(plan: &str) -> &str {
    match plan {
        "free" => "ChatGPT Free Subscription",
        "go" => "ChatGPT Go Subscription",
        "plus" => "ChatGPT Plus Subscription",
        "pro" => "ChatGPT Pro 200 Subscription",
        "prolite" => "ChatGPT Pro 100 Subscription",
        "promax" => "ChatGPT Pro 500 Subscription",
        "self_serve_business_prolite" => "ChatGPT Business Premium Subscription",
        "team" | "self_serve_business_usage_based" | "business" => "ChatGPT Business Subscription",
        "ent26" | "enterprise_cbp_automation" | "enterprise_cbp_usage_based" | "enterprise" => {
            "ChatGPT Enterprise Subscription"
        }
        "edu" | "edu_plus" | "edu_pro" => "ChatGPT Edu Subscription",
        other => other,
    }
}

/// SIWC identity is validated by the host-managed OAuth flow. Codex's custom
/// Responses provider deliberately reports no native account, so its model
/// list is a catalog and does not establish entitlement or quota.
pub fn parse_managed_metadata(
    connection: &str,
    route: &str,
    initialize: &Value,
    identity: &super::ManagedIdentity,
    native_models: &[Value],
) -> Result<ConnectionMetadata> {
    if connection.is_empty()
        || route.is_empty()
        || identity.subject.is_empty()
        || identity.client_id.is_empty()
        || native_models.len() > 2048
    {
        return Err(invalid());
    }
    let version = text(&initialize["userAgent"])
        .and_then(|agent| agent.split_once('/').map(|(_, v)| v))
        .ok_or_else(invalid)?;
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(invalid)?;
    let minor = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(invalid)?;
    if major == 0 && minor < 157 {
        return Err(invalid());
    }
    let mut source = EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        "openai_chatgpt_plan",
        "https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server",
    );
    source.client_version = Some(version.into());
    let account_id = stable_id(
        "account",
        &[connection, &identity.client_id, &identity.subject],
    );
    let display_label = identity.email.as_deref().unwrap_or("ChatGPT account");
    Ok(ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: Observation::reported(version.into(), source.clone()),
        auth: Observation::reported(AuthState::Authenticated, source.clone()),
        plan: Observation::unknown("PLAN_NOT_REPORTED_BY_SIWC"),
        billing: Observation::reported(BillingMode::Subscription, source.clone()),
        account: Some(Account {
            id: account_id.clone(),
            provider_id: "openai".into(),
            display_label: Observation::reported(display_label.into(), source.clone()),
            identity_source: Observation::reported(
                "Verified SIWC ID token subject and issued client registration".into(),
                source.clone(),
            ),
            scopes: vec![],
        }),
        models: models::parse_models(route, &account_id, native_models, &source)?,
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("SIWC_USAGE_PERMISSION_NOT_REPORTED"),
        diagnostics: vec![
            "MODEL_CATALOG_NOT_ENTITLEMENT".into(),
            "USAGE_UNAVAILABLE".into(),
        ],
    })
}
