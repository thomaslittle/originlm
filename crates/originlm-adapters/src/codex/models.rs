use super::{invalid, required, text};
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn parse_models(
    route: &str,
    identity: &str,
    values: &[Value],
    source: &EvidenceSource,
) -> Result<Vec<Model>> {
    let mut models = Vec::new();
    let mut seen = BTreeSet::new();
    for native in values {
        let id = required(native, "model")?;
        if !seen.insert(id) {
            return Err(invalid());
        }
        if native["hidden"].as_bool() == Some(true) {
            continue;
        }
        let name = required(native, "displayName")?;
        let mut options = Vec::new();
        let effort_choices = choices(
            native.get("supportedReasoningEfforts"),
            "reasoningEffort",
            "reasoningEffort",
        )?;
        let tier_choices = choices(native.get("serviceTiers"), "id", "name")?;
        for (key, label, choices, default_key, billing) in [
            (
                "reasoningEffort",
                "Reasoning effort",
                effort_choices,
                "defaultReasoningEffort",
                false,
            ),
            (
                "serviceTier",
                "Service tier",
                tier_choices,
                "defaultServiceTier",
                true,
            ),
        ] {
            if choices.is_empty() {
                continue;
            }
            let default = text(&native[default_key])
                .filter(|value| choices.iter().any(|choice| choice.value == *value))
                .map(str::to_owned);
            options.push(OptionDescriptor {
                id: key.into(),
                label: label.into(),
                scope: OptionScope::Turn,
                revision: String::new(),
                control: OptionControl::Select {
                    choices,
                    default_value: default,
                },
                current_value: None,
                category: Some(key.into()),
                source: Some(source.clone()),
                billing_sensitive: billing,
                permission_sensitive: false,
            });
        }
        let mut capabilities = vec![Capability {
            id: "generation.text".into(),
            support: Support::Supported,
            origin: CapabilityOrigin::Model,
            evidence: Observation::reported(
                "Listed by native Codex model catalog".into(),
                source.clone(),
            ),
            conditions: Vec::new(),
        }];
        if let Some(modalities) = native["inputModalities"].as_array() {
            if modalities.len() > 32 {
                return Err(invalid());
            }
            for item in modalities {
                if let Some(value) = text(item).filter(|s| {
                    s.len() <= 32 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                }) {
                    capabilities.push(Capability {
                        id: format!("input.{value}"),
                        support: Support::Supported,
                        origin: CapabilityOrigin::Model,
                        evidence: Observation::reported(
                            "Native model input modality".into(),
                            source.clone(),
                        ),
                        conditions: Vec::new(),
                    });
                }
            }
        }
        let rev = stable_id(
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
                })
                .to_string(),
            ],
        );
        for option in &mut options {
            option.revision = rev.clone();
        }
        models.push(Model {
            id: route_model_key(route, id),
            route_id: route.into(),
            native_id: id.into(),
            name: name.into(),
            revision: rev,
            is_default: native["isDefault"].as_bool().unwrap_or(false),
            capabilities,
            options,
            context_window: Observation::unknown("CONTEXT_NOT_REPORTED_BY_NATIVE_CATALOG"),
        });
    }
    Ok(models)
}
/// Point the service-tier control at the tier Codex has saved in `config/read`.
/// Only a tier the model already advertises is selected. `fast` is the saved
/// name for the `priority` choice. Reimplemented from T3 Code
/// `applyConfiguredCodexServiceTier` at
/// [3b844dab5c](https://github.com/pingdotgg/t3code/commit/3b844dab5c).
pub fn apply_configured_service_tier(models: &mut [Model], configured: &str) {
    let configured = configured.trim();
    if configured.is_empty() || configured.len() > 64 {
        return;
    }
    for model in models {
        for option in &mut model.options {
            if option.id != "serviceTier" {
                continue;
            }
            let OptionControl::Select { choices, .. } = &option.control else {
                continue;
            };
            let selected = choices
                .iter()
                .find(|choice| choice.disabled_reason.is_none() && choice.value == configured)
                .or_else(|| {
                    if configured == "fast" {
                        choices.iter().find(|choice| {
                            choice.disabled_reason.is_none() && choice.value == "priority"
                        })
                    } else {
                        None
                    }
                });
            if let Some(choice) = selected {
                option.current_value = Some(Value::String(choice.value.clone()));
            }
        }
    }
}
fn choices(value: Option<&Value>, key: &str, label: &str) -> Result<Vec<Choice>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or_else(invalid)?;
    let mut seen = BTreeSet::new();
    let mut choices = Vec::new();
    for item in items {
        let id = required(item, key)?;
        if !seen.insert(id) {
            return Err(invalid());
        }
        choices.push(Choice {
            value: id.into(),
            label: text(&item[label]).unwrap_or(id).into(),
            disabled_reason: None,
        });
    }
    Ok(choices)
}
