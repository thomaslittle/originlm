//! On-disk local model inventory. Ids, names, sizes and a coarse format only;
//! never capability or quantization guesses.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalModel {
    pub provider_id: String,
    pub native_id: String,
    pub name: String,
    pub size_bytes: Option<u64>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalModelInventory {
    pub provider_id: String,
    pub roots: Vec<String>,
    pub models: Vec<LocalModel>,
    pub files_skipped: u32,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
}
