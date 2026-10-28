//! Codex app-server metadata. Native subscriptions are never converted to API keys.
mod metadata;
mod models;
mod usage;
pub use metadata::parse_managed_metadata;
pub use metadata::parse_metadata;
pub use models::apply_configured_service_tier;
use originlm_contracts::{DomainError, ErrorCode, Result};
use serde_json::Value;
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Codex returned unsupported metadata",
    )
}
fn text(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    text(&value[key]).ok_or_else(invalid)
}
mod launch;
pub use launch::{managed_launch_spec, managed_metadata, probe, resolve_executable};

#[derive(Debug, Clone)]
pub struct ManagedIdentity {
    pub app_name: String,
    pub subject: String,
    pub client_id: String,
    pub email: Option<String>,
}
