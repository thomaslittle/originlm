pub mod client;
pub mod launch;
pub use launch::probe;
mod metadata;
mod usage;
pub use metadata::parse_metadata;
use originlm_contracts::*;
use serde_json::Value;
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Claude metadata is unavailable",
    )
}
fn text(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 1024 && !v.chars().any(char::is_control))
}
