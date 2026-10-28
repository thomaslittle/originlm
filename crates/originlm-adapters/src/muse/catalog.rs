//! Muse metadata: `initialize`, `account/read`, `model/list` and `usage/read`
//! normalized into the kit's allowlisted connection record. Pure functions.
use super::{META_PROVIDER, PROVIDER, invalid, text};
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Display labels for MSP's closed reasoning-effort vocabulary.
fn effort_label(tier: &str) -> Option<&'static str> {
    Some(match tier {
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Max",
        "ultra" => "Ultra",
        _ => return None,
    })
}

fn source(init: &Value, reference: &str) -> EvidenceSource {
    let mut source = EvidenceSource::new(EvidenceKind::OfficialProtocol, PROVIDER, reference);
    source.client_version = text(&init["serverInfo"]["version"])
        .filter(|v| v.len() <= 64)
        .map(str::to_owned);
    source
}

/// Normalize one probe. `account` is the experimental `account/read` result
/// (absent when Muse does not offer it), `models` the `model/list` result and
/// `usage` the `usage/read` result.
pub fn parse_metadata(
    connection: &str,
    route: &str,
    init: &Value,
    account: Option<&Value>,
    models: &Value,
    usage: Option<&Value>,
) -> Result<ConnectionMetadata> {
    if !init.is_object() || connection.is_empty() || route.is_empty() {
        return Err(invalid());
    }
    let protocol = source(init, "Muse Session Protocol v1 (muse serve)");
    let account_source = source(
        init,
        "Muse Session Protocol v1 account/read (experimental surface; no credential is read)",
    );
    let mut diagnostics = Vec::new();
    let version = text(&init["serverInfo"]["version"])
        .filter(|v| v.len() <= 64)
        .map(|v| Observation::reported(v.to_owned(), protocol.clone()))
        .unwrap_or_else(|| Observation::unknown("CLIENT_VERSION_NOT_REPORTED"));
    let (auth, billing) = match account {
        None => {
            diagnostics.push("MUSE_ACCOUNT_STATE_UNAVAILABLE".into());
            (
                Observation::unknown("AUTH_NOT_REPORTED"),
                Observation::unknown("BILLING_NOT_REPORTED"),
            )
        }
        Some(account) => {
            let lane = account["state"].as_str().unwrap_or("");
            let required = account["credentialRequired"].as_bool();
            match (lane, required) {
                // A Meta-account login: Muse meters it as subscription usage
                // (`usage/read` reports subscription tiers and windows).
                ("accountLogin", _) => (
                    Observation::reported(AuthState::Authenticated, account_source.clone()),
                    Observation::reported(BillingMode::Subscription, account_source.clone()),
                ),
                ("apiKey" | "envKey", _) => (
                    Observation::reported(AuthState::Authenticated, account_source.clone()),
                    Observation::reported(BillingMode::ApiMetered, account_source.clone()),
                ),
                ("loggedOut", Some(false)) => {
                    diagnostics.push("MUSE_KEYLESS_DEPLOYMENT".into());
                    (
                        Observation::reported(AuthState::Authenticated, account_source.clone()),
                        Observation::unknown("BILLING_NOT_REPORTED"),
                    )
                }
                ("loggedOut", _) => (
                    Observation::reported(AuthState::Unauthenticated, account_source.clone()),
                    Observation::unknown("BILLING_NOT_REPORTED"),
                ),
                _ => {
                    diagnostics.push("MUSE_ACCOUNT_STATE_UNRECOGNIZED".into());
                    (
                        Observation::unknown("AUTH_NOT_REPORTED"),
                        Observation::unknown("BILLING_NOT_REPORTED"),
                    )
                }
            }
        }
    };
    let label = account
        .and_then(|a| text(&a["label"]))
        .filter(|l| l.len() <= 256);
    let account_record = label.map(|label| Account {
        id: stable_id(
            "account",
            &[
                connection,
                account
                    .and_then(|a| a["state"].as_str())
                    .unwrap_or("unknown"),
                &label.to_lowercase(),
            ],
        ),
        provider_id: META_PROVIDER.into(),
        display_label: Observation::reported(label.into(), account_source.clone()),
        identity_source: Observation::reported(
            "Connection-scoped Muse account/read label; no cross-client linking".into(),
            account_source.clone(),
        ),
        scopes: vec![],
    });
    let identity = account_record
        .as_ref()
        .map(|a| a.id.clone())
        .unwrap_or_else(|| "no-account".into());
    let mut report = ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version,
        auth,
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        billing,
        account: account_record,
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("INCLUDED_USAGE_PERMISSION_NOT_REPORTED"),
        diagnostics,
    };
    report.models = parse_models(route, &identity, init, models, &mut report.diagnostics)?;
    match usage.and_then(|u| u.get("usage")).filter(|u| u.is_object()) {
        Some(usage) => {
            let usage_source = source(init, "Muse Session Protocol v1 usage/read");
            if let Some(tier) = text(&usage["tier"]).filter(|t| t.len() <= 128) {
                report.plan = Observation::reported(
                    Plan {
                        native_id: tier.into(),
                        label: tier.into(),
                    },
                    usage_source.clone(),
                );
            }
            report.quota_pools = parse_usage(
                connection,
                route,
                report.account.as_ref().map(|a| a.id.clone()),
                usage,
                &usage_source,
            )?;
        }
        None => report.diagnostics.push("USAGE_NOT_YET_OBSERVED".into()),
    }
    Ok(report)
}

fn parse_models(
    route: &str,
    identity: &str,
    init: &Value,
    catalog: &Value,
    diagnostics: &mut Vec<String>,
) -> Result<Vec<Model>> {
    let rows = catalog["models"]
        .as_array()
        .filter(|rows| rows.len() <= 2048)
        .ok_or_else(invalid)?;
    let origin = catalog["source"].as_str().unwrap_or("");
    if origin == "fakeCatalog" {
        // A test fake is never presented as a live catalog.
        diagnostics.push("MUSE_FAKE_CATALOG_IGNORED".into());
        return Ok(vec![]);
    }
    if catalog["providerId"] != META_PROVIDER {
        diagnostics.push("MUSE_CATALOG_PROVIDER_NOT_META".into());
        return Ok(vec![]);
    }
    let reference = format!(
        "Muse Session Protocol v1 model/list ({})",
        text(&catalog["source"])
            .filter(|s| s.len() <= 64)
            .unwrap_or("unspecified source")
    );
    let catalog_source = source(init, &reference);
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    for row in rows {
        if row["providerId"] != META_PROVIDER {
            continue;
        }
        let id = text(&row["modelId"])
            .filter(|id| id.len() <= 256)
            .ok_or_else(invalid)?;
        if !seen.insert(id) {
            continue;
        }
        let name = text(&row["displayLabel"])
            .filter(|n| n.len() <= 256)
            .unwrap_or(id);
        let mut options = Vec::new();
        // `"unknown"` (or an absent list) means Muse did not report the tiers:
        // no effort control is invented.
        if let Some(variants) = row["variants"].as_array() {
            if variants.len() > 16 {
                return Err(invalid());
            }
            let mut tiers = BTreeSet::new();
            let mut choices = Vec::new();
            for tier in variants {
                let tier = text(tier).ok_or_else(invalid)?;
                let Some(label) = effort_label(tier) else {
                    continue;
                };
                if tiers.insert(tier) {
                    choices.push(Choice {
                        value: tier.into(),
                        label: label.into(),
                        disabled_reason: None,
                    });
                }
            }
            if !choices.is_empty() {
                let default_value = row["defaultReasoningEffort"]
                    .as_str()
                    .filter(|d| tiers.contains(d))
                    .map(str::to_owned);
                options.push(OptionDescriptor {
                    id: "reasoningEffort".into(),
                    label: "Reasoning effort".into(),
                    scope: OptionScope::Turn,
                    revision: String::new(),
                    control: OptionControl::Select {
                        choices,
                        default_value,
                    },
                    current_value: None,
                    category: Some("reasoning".into()),
                    source: Some(catalog_source.clone()),
                    billing_sensitive: false,
                    permission_sensitive: false,
                });
            }
        }
        let capabilities = vec![
            Capability {
                id: "generation.text".into(),
                support: Support::Supported,
                origin: CapabilityOrigin::Model,
                evidence: Observation::reported(
                    "Listed by Muse model/list".into(),
                    catalog_source.clone(),
                ),
                conditions: vec![],
            },
            // MSP `turn/start` carries base64 `image` parts; Muse forwards
            // them to the session's model. model/list does not report
            // per-model vision, so a model that cannot see images fails
            // the turn with Muse's own error.
            Capability {
                id: "input.image".into(),
                support: Support::Supported,
                origin: CapabilityOrigin::Runtime,
                evidence: Observation::reported(
                    "Muse Session Protocol turn/start accepts image parts".into(),
                    source(init, "Muse Session Protocol v1 TurnInputPart image"),
                ),
                conditions: vec!["Per-model vision support is not reported by Muse".into()],
            },
        ];
        let context = row["contextLimit"]
            .as_u64()
            .filter(|n| *n > 0 && *n < 1 << 40);
        let revision = stable_id(
            "catalog",
            &[
                route,
                identity,
                id,
                &json!({
                    "options": crate::option_signature(&options),
                    "capabilities": capabilities
                        .iter()
                        .map(|c| json!([c.id, c.support]))
                        .collect::<Vec<_>>(),
                    "context": context,
                })
                .to_string(),
            ],
        );
        for option in &mut options {
            option.revision = revision.clone();
        }
        models.push(Model {
            id: route_model_key(route, id),
            route_id: route.into(),
            native_id: id.into(),
            name: name.into(),
            revision,
            is_default: row["isDefault"].as_bool().unwrap_or(false),
            capabilities,
            options,
            context_window: context
                .map(|n| Observation::reported(n.to_string(), catalog_source.clone()))
                .unwrap_or_else(|| Observation::unknown("CONTEXT_NOT_REPORTED_BY_NATIVE_CATALOG")),
        });
    }
    if models.is_empty() {
        diagnostics.push("MUSE_MODEL_CATALOG_EMPTY".into());
    }
    Ok(models)
}

/// MSP `SubscriptionUsage`: a current window and a rolling weekly block,
/// each a provider-reported percent. Percent windows are not token counts.
fn parse_usage(
    connection: &str,
    route: &str,
    account: Option<String>,
    usage: &Value,
    source: &EvidenceSource,
) -> Result<Vec<QuotaPool>> {
    let pool_id = stable_id(
        "quota",
        &[
            connection,
            account.as_deref().unwrap_or("no-account"),
            "muse",
        ],
    );
    let mut windows = Vec::new();
    for (key, label) in [
        ("window", "Muse · current window"),
        ("weekly", "Muse · weekly"),
    ] {
        let Some(window) = usage.get(key).filter(|w| w.is_object()) else {
            continue;
        };
        let percent = window["usedPercent"]
            .as_u64()
            .filter(|p| *p <= 10_000)
            .ok_or_else(invalid)?;
        let reading = UsageReading::try_from(UsageInput {
            unit: "percent".into(),
            used: DecimalString::try_from(percent.to_string()).map_err(|_| invalid())?,
            limit: Some(DecimalString::try_from("100".to_owned()).map_err(|_| invalid())?),
        })
        .map_err(|_| invalid())?;
        let resets_at = window["resetsAtMs"]
            .as_i64()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .and_then(|dt| Timestamp::try_from(dt.to_rfc3339()).ok());
        let duration_seconds = window["windowDurationMins"]
            .as_u64()
            .and_then(|mins| mins.checked_mul(60))
            .and_then(|secs| u32::try_from(secs).ok());
        windows.push(UsageWindow {
            id: stable_id("window", &[&pool_id, key]),
            label: label.into(),
            reading: Observation::reported(reading, source.clone()),
            resets_at,
            duration_seconds,
        });
    }
    if windows.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![QuotaPool {
        id: pool_id,
        account_id: account,
        connection_ids: vec![connection.into()],
        route_ids: vec![route.into()],
        sharing: "unknown".into(),
        windows,
    }])
}
