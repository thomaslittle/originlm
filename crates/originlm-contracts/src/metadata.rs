use crate::{
    AccessRoute, Account, AuthState, BillingMode, Model, Observation, Plan, Provider, QuotaPool,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Allowlisted adapter output. The service projects this into authorized snapshots.
/// This record never contains native credential material or private filesystem paths.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionMetadata {
    pub connection_id: String,
    pub route_id: String,
    pub version: Observation<String>,
    pub auth: Observation<AuthState>,
    pub plan: Observation<Plan>,
    pub billing: Observation<BillingMode>,
    pub account: Option<Account>,
    pub models: Vec<Model>,
    #[serde(default)]
    pub additional_routes: Vec<AccessRoute>,
    #[serde(default)]
    pub providers: Vec<Provider>,
    pub quota_pools: Vec<QuotaPool>,
    pub included_usage_allowed: Observation<bool>,
    pub diagnostics: Vec<String>,
}
