//! Per-provider detection status for the control surface. A missing tool is an
//! explicit state with the locations that were checked, never a silent absence.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDetection {
    pub provider_id: String,
    pub name: String,
    /// `found`, `configuration_only`, or `not_found`.
    pub status: String,
    pub connection_count: u32,
    pub executable_found: bool,
    pub diagnostics: Vec<String>,
    pub paths_checked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionReport {
    pub providers: Vec<ProviderDetection>,
}
