use super::{invalid, text, usage};
use originlm_contracts::*;
use serde_json::Value;
use std::collections::BTreeSet;
pub fn parse_metadata(
    connection: &str,
    route: &str,
    init: &Value,
    usage: Option<&Value>,
) -> Result<ConnectionMetadata> {
    if !init.is_object() || connection.is_empty() || route.is_empty() {
        return Err(invalid());
    }
    let source = EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        "claude_code",
        "https://github.com/anthropics/claude-agent-sdk-python",
    );
    let account = &init["account"];
    let subscription = text(&account["subscriptionType"]);
    let token = text(&account["tokenSource"]);
    let api = text(&account["apiProvider"]);
    let metered = matches!(api, Some("bedrock" | "vertex" | "foundry"))
        || matches!(
            token,
            Some("apiKey" | "ANTHROPIC_API_KEY" | "anthropicApiKey" | "api_key")
        );
    let included = !metered && api == Some("firstParty") && subscription.is_some();
    let verified = metered || included;
    let identity = stable_id(
        "account",
        &[
            connection,
            api.unwrap_or("unknown"),
            token.unwrap_or("unknown"),
            text(&account["email"]).unwrap_or("unreported"),
        ],
    );
    let mut report = ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: Observation::unknown("CLIENT_VERSION_NOT_REPORTED"),
        auth: if verified {
            Observation::reported(AuthState::Authenticated, source.clone())
        } else {
            Observation::unknown("AUTH_NOT_REPORTED")
        },
        plan: subscription
            .map(|p| {
                Observation::reported(
                    Plan {
                        native_id: p.into(),
                        label: p.into(),
                    },
                    source.clone(),
                )
            })
            .unwrap_or_else(|| Observation::unknown("PLAN_NOT_REPORTED")),
        billing: if included {
            Observation::reported(BillingMode::Subscription, source.clone())
        } else if metered {
            Observation::reported(BillingMode::ApiMetered, source.clone())
        } else {
            Observation::unknown("BILLING_NOT_REPORTED")
        },
        account: None,
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("INCLUDED_USAGE_PERMISSION_NOT_REPORTED"),
        diagnostics: vec![],
    };
    if let Some(email) = text(&account["email"]) {
        report.account = Some(Account {
            id: identity.clone(),
            provider_id: "anthropic".into(),
            display_label: Observation::reported(email.into(), source.clone()),
            identity_source: Observation::reported(
                "Connection-scoped native initialization; no cross-client linking".into(),
                source.clone(),
            ),
            scopes: vec![],
        });
    }
    if let Some(models) = init.get("models") {
        let models = models
            .as_array()
            .filter(|m| m.len() <= 2048)
            .ok_or_else(invalid)?;
        let mut seen = BTreeSet::new();
        for entry in models {
            let id = text(&entry["value"]).ok_or_else(invalid)?;
            if !seen.insert(id) {
                return Err(invalid());
            }
            let revision = stable_id(
                "catalog",
                &[
                    &identity,
                    id,
                    &entry["resolvedModel"].to_string(),
                    &entry["supportedEffortLevels"].to_string(),
                ],
            );
            let mut options = vec![];
            if entry["supportsEffort"] == true
                && let Some(levels) = entry["supportedEffortLevels"].as_array()
            {
                if levels.len() > 64 {
                    return Err(invalid());
                }
                let mut names = BTreeSet::new();
                let choices = levels
                    .iter()
                    .map(|v| {
                        let value = text(v).filter(|v| v.len() <= 64).ok_or_else(invalid)?;
                        if !names.insert(value) {
                            return Err(invalid());
                        }
                        Ok(Choice {
                            value: value.into(),
                            label: value.into(),
                            disabled_reason: None,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                if !choices.is_empty() {
                    options.push(OptionDescriptor {
                        id: "reasoningEffort".into(),
                        label: "Reasoning effort (session startup)".into(),
                        scope: OptionScope::Session,
                        revision: revision.clone(),
                        control: OptionControl::Select {
                            choices,
                            default_value: None,
                        },
                        current_value: None,
                        category: Some("reasoning".into()),
                        source: Some(source.clone()),
                        billing_sensitive: false,
                        permission_sensitive: false,
                    });
                }
            }
            report.models.push(Model {
                id: route_model_key(route, id),
                route_id: route.into(),
                native_id: id.into(),
                name: text(&entry["displayName"]).unwrap_or(id).into(),
                revision,
                is_default: id == "default",
                capabilities: vec![
                    Capability {
                        id: "generation.text".into(),
                        support: Support::Supported,
                        origin: CapabilityOrigin::Model,
                        evidence: Observation::reported(
                            "Native client text model catalog".into(),
                            source.clone(),
                        ),
                        conditions: vec![],
                    },
                    // Claude Code forwards base64 image content blocks in the
                    // stream-json user message; every Claude model accepts them.
                    Capability {
                        id: "input.image".into(),
                        support: Support::Supported,
                        origin: CapabilityOrigin::Model,
                        evidence: Observation::reported(
                            "Claude models accept image content blocks".into(),
                            EvidenceSource::new(
                                EvidenceKind::OfficialApi,
                                "anthropic",
                                "https://docs.claude.com/en/docs/build-with-claude/vision",
                            ),
                        ),
                        conditions: vec![],
                    },
                ],
                options,
                context_window: Observation::unknown("CONTEXT_NOT_REPORTED_AS_STRUCTURED_DATA"),
            });
        }
    } else {
        report.diagnostics.push("MODEL_CATALOG_NOT_REPORTED".into());
    }
    if let Some(usage) = usage {
        report.quota_pools = usage::parse(
            connection,
            route,
            report.account.as_ref().map(|a| a.id.clone()),
            usage,
            &source,
        )?;
    } else {
        report.diagnostics.push("USAGE_UNAVAILABLE".into());
    }
    Ok(report)
}
