use super::HttpKind;
use originlm_contracts::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Provider catalog format is invalid",
    )
}
fn text(v: &Value) -> Result<String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn evidence(kind: HttpKind) -> EvidenceSource {
    EvidenceSource::new(
        EvidenceKind::OfficialApi,
        kind.id(),
        "Provider model metadata endpoint",
    )
}
fn cap(kind: HttpKind, id: &str, support: Support) -> Capability {
    let unknown = support == Support::Unknown;
    Capability {
        id: id.into(),
        support,
        origin: CapabilityOrigin::Model,
        evidence: if unknown {
            Observation::unknown("NOT_REPORTED")
        } else {
            Observation::reported("Provider metadata".into(), evidence(kind))
        },
        conditions: vec![],
    }
}
pub fn parse_catalog(
    kind: HttpKind,
    connection: &str,
    route: &str,
    raw: Value,
    details: &BTreeMap<String, Value>,
    declared_text: &[String],
) -> Result<ConnectionMetadata> {
    if connection.is_empty() || route.is_empty() || declared_text.len() > 256 {
        return Err(invalid());
    }
    let key = if matches!(
        kind,
        HttpKind::Ollama | HttpKind::Lmstudio | HttpKind::Gemini
    ) {
        "models"
    } else {
        "data"
    };
    let entries = raw[key]
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or_else(invalid)?;
    let mut models = Vec::new();
    let mut seen = BTreeSet::new();
    let mut diagnostics = vec!["PLAN_AND_ACCOUNT_QUOTA_NOT_REPORTED".into()];
    for item in entries {
        if kind == HttpKind::Lmstudio && item["type"] != "llm" {
            continue;
        }
        let id_key = match kind {
            HttpKind::Ollama | HttpKind::Gemini => "name",
            HttpKind::Lmstudio => "key",
            _ => "id",
        };
        let native = text(&item[id_key])?;
        if !seen.insert(native.clone()) {
            return Err(invalid());
        }
        let detail = details.get(&native).unwrap_or(&Value::Null);
        if item.get("remote_host").is_some_and(|v| !v.is_null())
            || detail.get("remote_host").is_some_and(|v| !v.is_null())
        {
            diagnostics.push("REMOTE_MODELS_EXCLUDED".into());
            continue;
        }
        let name = item
            .get("display_name")
            .or_else(|| item.get("displayName"))
            .map(text)
            .transpose()?
            .unwrap_or_else(|| native.clone());
        let mut capabilities = Vec::new();
        let mut options = Vec::new();
        let mut context = Observation::unknown("NOT_REPORTED");
        let mut support = Support::Unknown;
        match kind {
            HttpKind::Ollama => {
                if let Some(caps) = detail["capabilities"].as_array() {
                    if caps.len() > 128 {
                        return Err(invalid());
                    }
                    support = if caps.iter().any(|v| v == "completion") {
                        Support::Supported
                    } else {
                        Support::Unsupported
                    };
                    for (native_cap, id) in [
                        ("vision", "input.image"),
                        ("tools", "tools.call"),
                        ("embedding", "generation.embedding"),
                    ] {
                        if caps.iter().any(|v| v == native_cap) {
                            capabilities.push(cap(kind, id, Support::Supported));
                        }
                    }
                }
                if let Some(architecture) = detail["model_info"]["general.architecture"].as_str()
                    && let Some(value) =
                        detail["model_info"][format!("{architecture}.context_length")].as_u64()
                {
                    context = Observation::reported(value.to_string(), evidence(kind));
                }
            }
            HttpKind::Lmstudio => {
                support = Support::Supported;
                if let Some(value) = item["max_context_length"].as_u64() {
                    context = Observation::reported(value.to_string(), evidence(kind));
                }
                if item["capabilities"]["vision"] == true {
                    capabilities.push(cap(kind, "input.image", Support::Supported));
                }
            }
            HttpKind::Anthropic => {
                support = Support::Supported;
                if let Some(value) = item["max_input_tokens"].as_u64() {
                    context = Observation::reported(value.to_string(), evidence(kind));
                }
                if let Some(efforts) = item["capabilities"]["effort"].as_object() {
                    let choices = efforts
                        .iter()
                        .filter(|(key, value)| *key != "supported" && value["supported"] == true)
                        .map(|(value, _)| Choice {
                            value: value.clone(),
                            label: value.clone(),
                            disabled_reason: None,
                        })
                        .collect::<Vec<_>>();
                    if !choices.is_empty() {
                        options.push(OptionDescriptor {
                            id: "effort".into(),
                            label: "Effort".into(),
                            scope: OptionScope::Turn,
                            revision: "pending".into(),
                            control: OptionControl::Select {
                                choices,
                                default_value: None,
                            },
                            current_value: None,
                            category: Some("reasoning".into()),
                            source: Some(evidence(kind)),
                            billing_sensitive: false,
                            permission_sensitive: false,
                        });
                    }
                }
            }
            HttpKind::Gemini => {
                if let Some(value) = item["inputTokenLimit"].as_u64() {
                    context = Observation::reported(value.to_string(), evidence(kind));
                }
                if !item["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|m| m == "generateContent"))
                {
                    support = Support::Unsupported;
                }
            }
            _ => {
                if let Some(value) = item["capabilities"]["text_generation"].as_bool() {
                    support = if value {
                        Support::Supported
                    } else {
                        Support::Unsupported
                    };
                }
            }
        }
        let mut text_cap = cap(kind, "generation.text", support.clone());
        if support == Support::Unknown && declared_text.contains(&native) {
            text_cap.support = Support::Supported;
            text_cap.evidence = Observation::reported(
                "Text compatibility explicitly declared by the host; not provider-verified".into(),
                EvidenceSource::new(
                    EvidenceKind::UserSupplied,
                    kind.id(),
                    "Explicit endpoint textModels configuration",
                ),
            );
        }
        capabilities.insert(0, text_cap);
        let revision = stable_id(
            "catalog",
            &[
                route,
                &native,
                &serde_json::to_string(&(item, detail, declared_text)).map_err(|_| invalid())?,
            ],
        );
        for option in &mut options {
            option.revision = revision.clone();
        }
        models.push(Model {
            id: route_model_key(route, &native),
            route_id: route.into(),
            native_id: native,
            name,
            revision,
            is_default: false,
            capabilities,
            options,
            context_window: context,
        });
    }
    if raw["has_more"] == true
        || raw
            .get("nextPageToken")
            .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
    {
        diagnostics.push("CATALOG_PARTIAL_PAGINATION_REQUIRED".into());
    }
    diagnostics.sort();
    diagnostics.dedup();
    Ok(ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: Observation::unknown("VERSION_NOT_REPORTED"),
        auth: if kind.local() {
            Observation::unknown("LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH")
        } else {
            Observation::reported(AuthState::Authenticated, evidence(kind))
        },
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        billing: Observation::reported(
            if kind.local() {
                BillingMode::Local
            } else {
                BillingMode::ApiMetered
            },
            EvidenceSource::new(
                EvidenceKind::UserSupplied,
                kind.id(),
                "Explicit local-runtime or metered-API endpoint registration",
            ),
        ),
        account: None,
        models,
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("QUOTA_NOT_REPORTED"),
        diagnostics,
    })
}
