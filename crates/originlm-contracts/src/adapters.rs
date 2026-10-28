//! Compiled adapter coverage, distinct from a user's account-level capability evidence.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdapterDescriptor {
    pub id: String,
    pub name: String,
    pub integration: String,
    pub passive_discovery: bool,
    pub metadata: bool,
    pub text_sessions: bool,
    pub native_tool_approval: bool,
    pub cancellation: bool,
    pub persistent_resumption: bool,
    /// Static implementation scope, not proof this machine/account has been qualified.
    pub qualification: String,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealth {
    pub status: String,
    pub mode: String,
    pub execution_available: bool,
    pub metadata_available: bool,
}
