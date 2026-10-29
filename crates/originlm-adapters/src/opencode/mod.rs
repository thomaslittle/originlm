//! OpenCode gateway integration; only allowlisted metadata crosses the adapter boundary.
pub mod catalog_v2;
pub mod client;
pub mod client_v2;
pub mod server;
pub mod v2_service;
mod version;
use originlm_contracts::*;
use originlm_discovery::ProfileScope;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
};

/// Environment for an exact OpenCode account. Global account roots are never
/// inherited; only executable/temp lookup variables come from the process.
pub(crate) fn scope_environment(scope: &ProfileScope) -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();
    for (key, value) in [
        ("HOME", &scope.config_home),
        ("USERPROFILE", &scope.config_home),
        ("OPENCODE_CONFIG_DIR", &scope.config_home),
        ("XDG_CONFIG_HOME", &scope.config_home),
        ("XDG_DATA_HOME", &scope.data_home),
        ("XDG_STATE_HOME", &scope.state_home),
    ] {
        environment.insert(OsString::from(key), value.as_os_str().to_owned());
    }
    // Keep any Windows app-data fallback inside the scoped config root.
    environment.insert(
        OsString::from("APPDATA"),
        scope.config_home.join("AppData/Roaming").into_os_string(),
    );
    environment.insert(
        OsString::from("LOCALAPPDATA"),
        scope.config_home.join("AppData/Local").into_os_string(),
    );
    for key in [
        "PATH",
        "Path",
        "SystemRoot",
        "SYSTEMROOT",
        "WINDIR",
        "TMPDIR",
        "TEMP",
        "TMP",
    ] {
        if let Some(value) = std::env::var_os(key) {
            environment.insert(OsString::from(key), value);
        }
    }
    environment
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "OpenCode metadata is invalid",
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
        "OpenCode /provider catalog",
    )
}

/// OpenCode's Go product is a subscription gateway on the same `opencode`
/// connection, published under the exact provider id `opencode-go`. Token
/// prices in its catalog measure allowance consumption; they do not make it a
/// pay-as-you-go API. Classify only the exact official id: `opencode` (Console
/// PAYG) and lookalikes must stay unknown until billing is declared.
///
/// This is route classification, not entitlement verification: auth, quota,
/// and the Console Extra usage toggle remain separately unknown. It therefore
/// never fabricates an included-quota or zero-overage claim.
pub(crate) fn inferred_gateway_billing(provider_id: &str) -> Option<Observation<BillingMode>> {
    (provider_id == "opencode-go").then(|| Observation::Known {
        value: BillingMode::Subscription,
        source: EvidenceSource::new(
            EvidenceKind::Derived,
            "opencode",
            "OpenCode Go provider id; subscription route class per official OpenCode docs; entitlement, quota and extra usage are not verified",
        ),
        observed_at: Timestamp::now(),
        freshness: Freshness::Fresh,
        verification: Verification::Inferred,
        expires_at: None,
    })
}
fn capability(id: &str, v: &Value) -> Capability {
    let support = match v.as_bool() {
        Some(true) => Support::Supported,
        Some(false) => Support::Unsupported,
        None => Support::Unknown,
    };
    Capability {
        id: id.into(),
        support,
        evidence: if v.is_boolean() {
            Observation::reported("Native gateway capability".into(), evidence())
        } else {
            Observation::unknown("NOT_REPORTED")
        },
        origin: CapabilityOrigin::Model,
        conditions: vec![],
    }
}
pub fn catalog(
    connection: &str,
    root_route: &str,
    raw: &Value,
    billing: &BTreeMap<String, BillingMode>,
) -> Result<ConnectionMetadata> {
    let connected = raw["connected"]
        .as_array()
        .filter(|v| v.len() <= 512)
        .ok_or_else(invalid)?;
    let all = raw["all"]
        .as_array()
        .filter(|v| v.len() <= 512)
        .ok_or_else(invalid)?;
    let gateway_auth = if connected.is_empty() {
        Observation::unknown("OPENCODE_NO_CONNECTED_PROVIDERS")
    } else {
        Observation::reported(
            AuthState::Authenticated,
            EvidenceSource::new(
                EvidenceKind::OfficialProtocol,
                "opencode",
                "OpenCode /provider connected list",
            ),
        )
    };
    let mut result = ConnectionMetadata {
        connection_id: connection.into(),
        route_id: root_route.into(),
        version: Observation::unknown("NOT_REPORTED"),
        auth: gateway_auth,
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        billing: Observation::unknown("GATEWAY_BILLING_IS_ROUTE_SPECIFIC"),
        account: None,
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("GATEWAY_QUOTA_NOT_REPORTED"),
        diagnostics: vec![
            "GATEWAY_PLAN_AND_USAGE_UNAVAILABLE".into(),
            "UPSTREAM_ACCESS_NOT_INDEPENDENTLY_VALIDATED".into(),
        ],
    };
    let mut seen = BTreeSet::new();
    for provider in all {
        let id = text(&provider["id"])?;
        if !seen.insert(id.clone()) {
            return Err(invalid());
        }
        if !connected.iter().any(|v| v.as_str() == Some(&id)) {
            continue;
        }
        let name = text(&provider["name"])?;
        result.providers.push(Provider {
            id: id.clone(),
            name: name.clone(),
            kind: "gateway_upstream".into(),
        });
        let route_id = stable_id("route", &[root_route, &id]);
        let entries = provider["models"]
            .as_object()
            .filter(|m| m.len() <= 4096)
            .ok_or_else(invalid)?;
        let mut native_seen = BTreeSet::new();
        for item in entries.values() {
            if result.models.len() >= 4096 {
                return Err(invalid());
            }
            let native = text(&item["id"])?;
            if !native_seen.insert(native.clone()) || item["providerID"] != id {
                return Err(invalid());
            }
            let name = text(&item["name"])?;
            let mut options = vec![];
            if let Some(variants) = item["variants"].as_object() {
                if variants.len() > 128 {
                    return Err(invalid());
                }
                let choices = variants
                    .iter()
                    .filter(|(_, v)| v["disabled"] != true)
                    .map(|(key, _)| {
                        Ok(Choice {
                            value: text(&Value::String(key.clone()))?,
                            label: key.clone(),
                            disabled_reason: None,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                if !choices.is_empty() {
                    options.push(OptionDescriptor {
                        id: "variant".into(),
                        label: "Native variant".into(),
                        scope: OptionScope::Turn,
                        revision: "pending".into(),
                        control: OptionControl::Select {
                            choices,
                            default_value: None,
                        },
                        current_value: None,
                        category: Some("reasoning".into()),
                        source: Some(evidence()),
                        billing_sensitive: false,
                        permission_sensitive: false,
                    });
                }
            }
            let capabilities = vec![
                capability("generation.text", &item["capabilities"]["output"]["text"]),
                capability("input.image", &item["capabilities"]["input"]["image"]),
                capability("tools.call", &item["capabilities"]["toolcall"]),
            ];
            let context = item["limit"]["context"]
                .as_u64()
                .map(|n| Observation::reported(n.to_string(), evidence()))
                .unwrap_or_else(|| Observation::unknown("NOT_REPORTED"));
            // Hash only the public semantic descriptor, never gateway keys/headers/private options.
            let revision = stable_id(
                "catalog",
                &[
                    &route_id,
                    &native,
                    &serde_json::to_string(&(
                        &item["capabilities"],
                        &item["limit"],
                        crate::option_signature(&options),
                    ))
                    .map_err(|_| invalid())?,
                ],
            );
            for option in &mut options {
                option.revision = revision.clone();
            }
            result.models.push(Model {
                id: route_model_key(&route_id, &native),
                route_id: route_id.clone(),
                native_id: native.clone(),
                name,
                revision,
                is_default: raw["default"][&id] == native,
                capabilities,
                options,
                context_window: context,
            });
        }
        let revision = stable_id(
            "catalog",
            &[
                &route_id,
                &result
                    .models
                    .iter()
                    .filter(|m| m.route_id == route_id)
                    .map(|m| m.revision.as_str())
                    .collect::<Vec<_>>()
                    .join("|"),
            ],
        );
        result.additional_routes.push(AccessRoute {
            id: route_id,
            connection_id: connection.into(),
            provider_id: id.clone(),
            display_name: format!("OpenCode / {name}"),
            billing: inferred_gateway_billing(&id)
                .or_else(|| {
                    billing
                        .get(&id)
                        .filter(|b| **b != BillingMode::Unknown)
                        .map(|b| {
                            Observation::reported(
                                b.clone(),
                                EvidenceSource::new(
                                    EvidenceKind::UserSupplied,
                                    "opencode",
                                    "Explicit providerBilling declaration; not an inferred subscription",
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
