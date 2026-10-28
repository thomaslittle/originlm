use chrono::{DateTime, SecondsFormat, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Validated RFC 3339 timestamp, kept as text on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct Timestamp(String);
impl Timestamp {
    pub fn now() -> Self {
        Self(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Timestamp {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        if value.len() > 64 || DateTime::parse_from_rfc3339(&value).is_err() {
            return Err("expected an RFC 3339 timestamp".into());
        }
        Ok(Self(value))
    }
}
impl From<Timestamp> for String {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    OfficialProtocol,
    OfficialApi,
    LocalConfig,
    LocalLog,
    CommunityEndpoint,
    UserSupplied,
    Derived,
    Synthetic,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceSource {
    pub kind: EvidenceKind,
    pub adapter_id: String,
    pub adapter_version: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reference: Option<String>,
}
impl EvidenceSource {
    pub fn new(kind: EvidenceKind, adapter: &str, reference: &str) -> Self {
        Self {
            kind,
            adapter_id: adapter.into(),
            adapter_version: env!("CARGO_PKG_VERSION").into(),
            client_version: None,
            reference: Some(reference.into()),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Reported,
    Observed,
    Inferred,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Observation<T> {
    Known {
        value: T,
        source: EvidenceSource,
        observed_at: Timestamp,
        freshness: Freshness,
        verification: Verification,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        expires_at: Option<Timestamp>,
    },
    Unknown {
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        checked_at: Option<Timestamp>,
    },
    Unavailable {
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        checked_at: Option<Timestamp>,
    },
}
impl<T> Observation<T> {
    pub fn unknown(reason: &str) -> Self {
        Self::Unknown {
            reason: reason.into(),
            checked_at: None,
        }
    }
    pub fn unavailable(reason: &str) -> Self {
        Self::Unavailable {
            reason: reason.into(),
            checked_at: Some(Timestamp::now()),
        }
    }
    pub fn reported(value: T, source: EvidenceSource) -> Self {
        Self::Known {
            value,
            source,
            observed_at: Timestamp::now(),
            freshness: Freshness::Fresh,
            verification: Verification::Reported,
            expires_at: None,
        }
    }
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Known { value, .. } => Some(value),
            _ => None,
        }
    }
    pub fn fresh_value(&self) -> Option<&T> {
        match self {
            Self::Known {
                value,
                freshness: Freshness::Fresh,
                ..
            } => Some(value),
            _ => None,
        }
    }
    pub fn mark_stale(&mut self) {
        if let Self::Known { freshness, .. } = self {
            *freshness = Freshness::Stale;
        }
    }
}
