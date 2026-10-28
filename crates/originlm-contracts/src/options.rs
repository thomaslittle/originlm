use crate::EvidenceSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OptionScope {
    Model,
    Session,
    Turn,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OptionControl {
    Select {
        choices: Vec<Choice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_value: Option<String>,
    },
    Boolean,
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    Text {
        max_length: usize,
    },
    Unknown {
        native_type: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptionDescriptor {
    pub id: String,
    pub label: String,
    pub scope: OptionScope,
    pub revision: String,
    pub control: OptionControl,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EvidenceSource>,
    #[serde(default)]
    pub billing_sensitive: bool,
    #[serde(default)]
    pub permission_sensitive: bool,
}
impl OptionDescriptor {
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        if self.id.is_empty() || self.id.len() > 256 || self.revision.is_empty() {
            return Err("invalid option descriptor identity".into());
        }
        let valid = match &self.control {
            OptionControl::Select { choices, .. } => value.as_str().is_some_and(|v| {
                choices
                    .iter()
                    .any(|c| c.value == v && c.disabled_reason.is_none())
            }),
            OptionControl::Boolean => value.is_boolean(),
            OptionControl::Number { min, max, step } => value.as_f64().is_some_and(|v| {
                if !min.is_finite()
                    || !max.is_finite()
                    || !step.is_finite()
                    || *step <= 0.0
                    || min > max
                    || v < *min
                    || v > *max
                {
                    return false;
                }
                let n = (v - min) / step;
                (n.round() - n).abs() <= 1e-8
            }),
            OptionControl::Text { max_length } => value
                .as_str()
                .is_some_and(|s| s.chars().count() <= *max_length && !s.contains('\0')),
            OptionControl::Unknown { .. } => false,
        };
        if valid {
            Ok(())
        } else {
            Err(format!("OPTION_UNSUPPORTED: {}", self.id))
        }
    }
}
