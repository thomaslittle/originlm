use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
/// Owned in-memory result. No public method accepts a filesystem path.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactReadParams {
    pub session_id: String,
    pub artifact_id: String,
    #[serde(default)]
    pub offset: String,
    pub max_bytes: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactChunk {
    pub artifact_id: String,
    pub session_id: String,
    pub display_name: String,
    pub media_type: String,
    pub total_bytes: String,
    pub offset: String,
    pub next_offset: String,
    pub data_base64: String,
    pub eof: bool,
}
