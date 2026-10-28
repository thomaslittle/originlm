use crate::{HealthSummary, Issue, Observation, OptionDescriptor, QuotaPool, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Length-prefix before hashing: route and model delimiters cannot collide.
pub fn stable_id(namespace: &str, parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for part in std::iter::once(&namespace).chain(parts.iter()) {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{namespace}_{:x}", hash.finalize())
}
pub fn route_model_key(route: &str, model: &str) -> String {
    stable_id("model", &[route, model])
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Authenticated,
    Unauthenticated,
    Expired,
    Unchecked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BillingMode {
    Subscription,
    ApiMetered,
    PrepaidCredits,
    Local,
    Mixed,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Supported,
    Unsupported,
    Conditional,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityOrigin {
    Model,
    AgentTool,
    Runtime,
    Transport,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub id: String,
    pub support: Support,
    pub origin: CapabilityOrigin,
    pub evidence: Observation<String>,
    pub conditions: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Installation {
    pub id: String,
    pub provider_id: String,
    pub environment_id: String,
    pub location_hint: String,
    pub version: Observation<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub installation_id: String,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountScope {
    /// `organization`, `workspace`, `project`, `region`, or a provider-native kind.
    pub kind: String,
    pub id: Observation<String>,
    pub label: Observation<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Account {
    pub id: String,
    pub provider_id: String,
    pub display_label: Observation<String>,
    pub identity_source: Observation<String>,
    /// Organization/project/region scope. Accounts with different verified
    /// scopes are never merged, pooled, or silently switched between.
    #[serde(default)]
    pub scopes: Vec<AccountScope>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub native_id: String,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub provider_id: String,
    pub installation_id: String,
    pub profile_id: String,
    pub display_name: String,
    pub account_id: Option<String>,
    pub auth: Observation<AuthState>,
    pub plan: Observation<Plan>,
    pub status: String,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessRoute {
    pub id: String,
    pub connection_id: String,
    pub provider_id: String,
    pub display_name: String,
    pub billing: Observation<BillingMode>,
    pub capabilities: Vec<Capability>,
    pub catalog_revision: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub route_id: String,
    pub native_id: String,
    pub name: String,
    pub revision: String,
    pub is_default: bool,
    pub capabilities: Vec<Capability>,
    pub options: Vec<OptionDescriptor>,
    pub context_window: Observation<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DataMode {
    Live,
    Synthetic,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub protocol_version: String,
    pub revision: String,
    pub observed_at: Timestamp,
    pub mode: DataMode,
    /// Opaque journal cursor at the committed revision. Subscribe with this
    /// value to obtain every inventory change committed after the snapshot.
    #[serde(default)]
    pub cursor: String,
    pub providers: Vec<Provider>,
    pub installations: Vec<Installation>,
    pub profiles: Vec<Profile>,
    pub accounts: Vec<Account>,
    pub connections: Vec<Connection>,
    pub routes: Vec<AccessRoute>,
    pub models: Vec<Model>,
    pub quota_pools: Vec<QuotaPool>,
    #[serde(default)]
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub health: HealthSummary,
    pub diagnostics: Vec<String>,
}
impl Snapshot {
    pub fn empty(mode: DataMode) -> Self {
        Self {
            protocol_version: "1.0".into(),
            revision: "0".into(),
            observed_at: Timestamp::now(),
            mode,
            cursor: "0:0".into(),
            providers: vec![],
            installations: vec![],
            profiles: vec![],
            accounts: vec![],
            connections: vec![],
            routes: vec![],
            models: vec![],
            quota_pools: vec![],
            issues: vec![],
            health: HealthSummary::default(),
            diagnostics: vec![],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub route_id: String,
    pub model_id: String,
    pub revision: String,
    #[serde(default)]
    pub options: BTreeMap<String, Value>,
}
