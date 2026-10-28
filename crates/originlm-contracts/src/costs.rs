//! Cost evidence is distinct from quota percentages and subscription charges.
use crate::{DecimalString, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelPrice {
    pub provider_id: String,
    pub native_model_id: String,
    pub input_usd_per_million: DecimalString,
    pub output_usd_per_million: DecimalString,
    pub reviewed_at: String,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CostUsage {
    pub session_id: String,
    pub turn_id: String,
    pub connection_id: String,
    pub route_id: String,
    pub model_id: String,
    pub observed_at: Timestamp,
    pub input_tokens: Option<DecimalString>,
    pub output_tokens: Option<DecimalString>,
    pub reported_usd: Option<DecimalString>,
    pub complete: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CostRecord {
    pub usage: CostUsage,
    pub provider_id: String,
    pub native_model_id: String,
    /// Null when either token count or a matching price is missing.
    pub amount_usd: Option<DecimalString>,
    /// provider_reported, estimated, or unknown. Never an invoice total.
    pub basis: String,
    pub price: Option<ModelPrice>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CostReport {
    pub records: Vec<CostRecord>,
    pub prices: Vec<ModelPrice>,
    pub scope: String,
    pub can_edit_prices: bool,
    pub prices_persisted: bool,
    pub storage_error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetPricesParams {
    pub prices: Vec<ModelPrice>,
}

/// Exact decimal arithmetic; called by the native service, not duplicated in SDKs.
pub fn estimate_cost(
    input: &DecimalString,
    output: &DecimalString,
    price: &ModelPrice,
) -> Option<DecimalString> {
    let amount = (input.decimal() * price.input_usd_per_million.decimal()
        + output.decimal() * price.output_usd_per_million.decimal())
        / bigdecimal::BigDecimal::from(1_000_000);
    DecimalString::try_from(amount.normalized().to_plain_string()).ok()
}
