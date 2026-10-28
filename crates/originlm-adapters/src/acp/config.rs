use super::{evidence, invalid, text};
use originlm_contracts::*;
use serde_json::Value;
use std::collections::BTreeSet;
fn choices(values: &Value) -> Result<Vec<Choice>> {
    let values = values
        .as_array()
        .filter(|v| v.len() <= 2048)
        .ok_or_else(invalid)?;
    let grouped = values.first().is_some_and(|v| v.get("group").is_some());
    let mut result = Vec::new();
    let mut groups = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for entry in values {
        let children = if grouped {
            let group = text(&entry["group"])?;
            text(&entry["name"])?;
            if !groups.insert(group) || entry.get("value").is_some() {
                return Err(invalid());
            }
            entry["options"].as_array().ok_or_else(invalid)?.as_slice()
        } else {
            if entry.get("group").is_some() {
                return Err(invalid());
            }
            std::slice::from_ref(entry)
        };
        for child in children {
            if result.len() >= 2048 {
                return Err(invalid());
            }
            let value = text(&child["value"])?;
            let label = text(&child["name"])?;
            if !ids.insert(value.clone()) {
                return Err(invalid());
            }
            result.push(Choice {
                value,
                label,
                disabled_reason: None,
            });
        }
    }
    Ok(result)
}
pub(super) fn parse(provider: &str, value: &Value) -> Result<Vec<OptionDescriptor>> {
    let values = value
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or_else(invalid)?;
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for entry in values {
        let id = text(&entry["id"])?;
        if id.len() > 256 || !ids.insert(id.clone()) {
            return Err(invalid());
        }
        let label = text(&entry["name"])?;
        let category = entry
            .get("category")
            .filter(|v| !v.is_null())
            .map(text)
            .transpose()?;
        let kind = text(&entry["type"])?;
        let current = entry.get("currentValue").ok_or_else(invalid)?.clone();
        let control = match kind.as_str() {
            "select" => {
                let choices = choices(&entry["options"])?;
                let selected = text(&current)?;
                if !choices.iter().any(|c| c.value == selected) {
                    return Err(invalid());
                }
                OptionControl::Select {
                    choices,
                    default_value: None,
                }
            }
            "boolean" if current.is_boolean() => OptionControl::Boolean,
            "boolean" => return Err(invalid()),
            _ => OptionControl::Unknown { native_type: kind },
        };
        let revision = stable_id(
            "option",
            &[
                &id,
                &serde_json::to_string(&(&control, &category)).map_err(|_| invalid())?,
            ],
        );
        let permission_sensitive = !matches!(
            category.as_deref(),
            Some("model" | "thought_level" | "model_config")
        );
        let billing_sensitive = category.as_deref() == Some("model_config");
        result.push(OptionDescriptor {
            id,
            label,
            scope: OptionScope::Session,
            revision,
            control,
            current_value: if current.is_string() || current.is_boolean() {
                Some(current)
            } else {
                None
            },
            category,
            source: Some(evidence(provider)),
            billing_sensitive,
            permission_sensitive,
        });
    }
    Ok(result)
}
