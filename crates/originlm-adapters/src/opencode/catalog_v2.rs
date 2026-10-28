//! Public OpenCode 2 model inventory, kept separate from authentication.
//! Configured credentials and model availability are not a login validation.
use originlm_contracts::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "OpenCode 2 inventory is invalid",
    )
}
fn text(v: &Value) -> Result<String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn evidence() -> EvidenceSource {
    EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        "opencode",
        "OpenCode 2 /api/model inventory",
    )
}
fn array(value: &Value, max: usize) -> Result<&[Value]> {
    value
        .as_array()
        .filter(|items| items.len() <= max)
        .map(Vec::as_slice)
        .ok_or_else(invalid)
}
fn modalities(value: &Value) -> Result<Vec<String>> {
    array(value, 32)?.iter().map(text).collect()
}
fn capability(id: &str, supported: bool) -> Capability {
    Capability {
        id: id.into(),
        support: if supported {
            Support::Supported
        } else {
            Support::Unsupported
        },
        origin: CapabilityOrigin::Model,
        evidence: Observation::reported("Native gateway capability".into(), evidence()),
        conditions: vec![],
    }
}

/// Normalizes an observed provider/model inventory. This function never claims
/// authentication: /api/integration only lists configured connections and
/// process-local failures, so absence of an error cannot authorize execution.
pub fn catalog(
    connection: &str,
    root_route: &str,
    providers: &Value,
    models: &Value,
    billing: &BTreeMap<String, BillingMode>,
) -> Result<ConnectionMetadata> {
    let mut names = BTreeMap::new();
    let mut disabled = BTreeSet::new();
    for provider in array(&providers["data"], 512)? {
        let id = text(&provider["id"])?;
        let name = text(&provider["name"])?;
        if names.insert(id.clone(), name).is_some() {
            return Err(invalid());
        }
        match provider["activation"].as_str() {
            Some("disabled") => {
                disabled.insert(id);
            }
            Some("auto" | "enabled") => {}
            _ => return Err(invalid()),
        }
    }
    let mut result = ConnectionMetadata {
        connection_id: connection.into(),
        route_id: root_route.into(),
        version: Observation::unknown("NOT_REPORTED"),
        auth: Observation::unknown("OPENCODE_2_AUTH_NOT_VERIFIED"),
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        billing: Observation::unknown("GATEWAY_BILLING_IS_ROUTE_SPECIFIC"),
        account: None,
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("GATEWAY_QUOTA_NOT_REPORTED"),
        diagnostics: vec![
            "OPENCODE_2_AUTH_NOT_VERIFIED".into(),
            "OPENCODE_2_MANAGED_LAUNCH_UNAVAILABLE".into(),
        ],
    };
    let mut seen = BTreeSet::new();
    let mut used = BTreeSet::new();
    for item in array(&models["data"], 4096)? {
        let provider = text(&item["providerID"])?;
        if !names.contains_key(&provider) {
            return Err(invalid());
        }
        let native = text(&item["id"])?;
        if !seen.insert((provider.clone(), native.clone())) {
            return Err(invalid());
        }
        let enabled = item["enabled"].as_bool().ok_or_else(invalid)?;
        let input = modalities(&item["capabilities"]["input"])?;
        let output = modalities(&item["capabilities"]["output"])?;
        let tools = item["capabilities"]["tools"]
            .as_bool()
            .ok_or_else(invalid)?;
        if !enabled || disabled.contains(&provider) || !output.iter().any(|kind| kind == "text") {
            continue;
        }
        let route = stable_id("route", &[root_route, &provider]);
        let mut variant_ids = BTreeSet::new();
        let mut choices = vec![];
        for variant in array(&item["variants"], 128)? {
            let id = text(&variant["id"])?;
            if !variant_ids.insert(id.clone()) {
                return Err(invalid());
            }
            choices.push(Choice {
                value: id.clone(),
                label: id,
                disabled_reason: None,
            });
        }
        let mut options = if choices.is_empty() {
            vec![]
        } else {
            vec![OptionDescriptor {
                id: "variant".into(),
                label: "Native variant".into(),
                scope: OptionScope::Turn,
                revision: "pending".into(),
                control: OptionControl::Select {
                    choices,
                    default_value: None,
                },
                current_value: None,
                category: None,
                source: Some(evidence()),
                billing_sensitive: false,
                permission_sensitive: false,
            }]
        };
        let capabilities = vec![
            capability("generation.text", true),
            capability("input.image", input.iter().any(|kind| kind == "image")),
            capability("tools.call", tools),
        ];
        let context = item["limit"]["context"]
            .as_u64()
            .map(|value| Observation::reported(value.to_string(), evidence()))
            .unwrap_or_else(|| Observation::unknown("NOT_REPORTED"));
        let revision = stable_id(
            "catalog",
            &[
                &route,
                &native,
                &serde_json::to_string(&(
                    input,
                    output,
                    tools,
                    item["limit"]["context"].as_u64(),
                    crate::option_signature(&options),
                ))
                .map_err(|_| invalid())?,
            ],
        );
        for option in &mut options {
            option.revision = revision.clone();
        }
        result.models.push(Model {
            id: route_model_key(&route, &native),
            route_id: route,
            native_id: native,
            name: text(&item["name"])?,
            revision,
            is_default: false,
            capabilities,
            options,
            context_window: context,
        });
        used.insert(provider);
    }
    for provider in used {
        let name = names.get(&provider).ok_or_else(invalid)?;
        let route = stable_id("route", &[root_route, &provider]);
        let revision = stable_id(
            "catalog",
            &[
                &route,
                &result
                    .models
                    .iter()
                    .filter(|model| model.route_id == route)
                    .map(|model| model.revision.as_str())
                    .collect::<Vec<_>>()
                    .join("|"),
            ],
        );
        result.providers.push(Provider {
            id: provider.clone(),
            name: name.clone(),
            kind: "gateway_upstream".into(),
        });
        result.additional_routes.push(AccessRoute {
            id: route,
            connection_id: connection.into(),
            provider_id: provider.clone(),
            display_name: format!("OpenCode / {name}"),
            billing: super::inferred_gateway_billing(&provider)
                .or_else(|| {
                    billing
                        .get(&provider)
                        .filter(|mode| **mode != BillingMode::Unknown)
                        .map(|mode| {
                            Observation::reported(
                                mode.clone(),
                                EvidenceSource::new(
                                    EvidenceKind::UserSupplied,
                                    "opencode",
                                    "Explicit providerBilling declaration; not authentication evidence",
                                ),
                            )
                        })
                })
                .unwrap_or_else(|| Observation::unknown("UPSTREAM_BILLING_NOT_REPORTED")),
            capabilities: vec![],
            catalog_revision: revision,
        });
    }
    Ok(result)
}
