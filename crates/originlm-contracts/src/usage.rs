use crate::{Observation, Timestamp};
use bigdecimal::{BigDecimal, ToPrimitive};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Nonnegative, finite decimal, represented losslessly across all SDKs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct DecimalString(String);
impl TryFrom<String> for DecimalString {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        if value.is_empty() || value.len() > 128 {
            return Err("decimal length out of bounds".into());
        }
        let mut parts = value.split('.');
        let integer = parts.next().unwrap_or("");
        let fraction = parts.next();
        if integer.is_empty()
            || !integer.bytes().all(|b| b.is_ascii_digit())
            || parts.next().is_some()
            || fraction.is_some_and(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("expected a nonnegative plain decimal string".into());
        }
        Ok(Self(value))
    }
}
impl From<DecimalString> for String {
    fn from(value: DecimalString) -> Self {
        value.0
    }
}
impl DecimalString {
    /// Normalize a provider JSON number, including scientific notation, without
    /// allocating an unbounded decimal expansion. Protocol values remain plain strings.
    pub fn from_json_number(value: &serde_json::Number) -> Option<Self> {
        let encoded = value.to_string();
        if encoded.len() > 128 {
            return None;
        }
        let decimal = BigDecimal::from_str(&encoded).ok()?;
        let (_, scale) = decimal.as_bigint_and_exponent();
        if !(-128..=128).contains(&scale) {
            return None;
        }
        Self::try_from(decimal.normalized().to_plain_string()).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn decimal(&self) -> BigDecimal {
        BigDecimal::from_str(&self.0).expect("validated decimal")
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageInput {
    pub unit: String,
    pub used: DecimalString,
    pub limit: Option<DecimalString>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageReading {
    pub unit: String,
    pub used: DecimalString,
    pub limit: Option<DecimalString>,
    pub remaining: Option<DecimalString>,
    pub used_fraction: Option<f64>,
}
impl TryFrom<UsageInput> for UsageReading {
    type Error = String;
    fn try_from(input: UsageInput) -> Result<Self, String> {
        if input.unit.is_empty() || input.unit.len() > 64 {
            return Err("invalid usage unit".into());
        }
        let used = input.used.decimal();
        let zero = BigDecimal::from(0);
        let (remaining, used_fraction) = match &input.limit {
            Some(limit) => {
                let total = limit.decimal();
                let remaining = (&total - &used)
                    .max(zero.clone())
                    .normalized()
                    .to_plain_string();
                let fraction = if total > zero {
                    (&used / total).to_f64().filter(|x| x.is_finite())
                } else {
                    None
                };
                (Some(DecimalString::try_from(remaining)?), fraction)
            }
            None => (None, None),
        };
        Ok(Self {
            unit: input.unit,
            used: input.used,
            limit: input.limit,
            remaining,
            used_fraction,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageWindow {
    pub id: String,
    pub label: String,
    pub reading: Observation<UsageReading>,
    pub resets_at: Option<Timestamp>,
    pub duration_seconds: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotaPool {
    pub id: String,
    pub account_id: Option<String>,
    pub connection_ids: Vec<String>,
    pub route_ids: Vec<String>,
    pub sharing: String,
    pub windows: Vec<UsageWindow>,
}

/// Normalized token totals from a provider's local session history.
///
/// `input_tokens` is fresh input only. Cache reads and cache writes are
/// separate fields; `reasoning_tokens` is a subset of `output_tokens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

impl UsageTotals {
    pub fn add(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }

    pub fn total_input_tokens(self) -> u64 {
        self.input_tokens
            .saturating_add(self.cached_input_tokens)
            .saturating_add(self.cache_creation_tokens)
    }

    pub fn is_zero(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelUsage {
    pub provider_id: String,
    pub native_model_id: String,
    pub sessions: u32,
    pub messages: u32,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderUsage {
    pub provider_id: String,
    pub roots: Vec<String>,
    pub files_read: u32,
    pub files_skipped: u32,
    pub malformed_lines: u32,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
    pub models: Vec<ModelUsage>,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageHistory {
    pub providers: Vec<ProviderUsage>,
}
