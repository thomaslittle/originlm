//! Muse Code (`muse serve`, Muse Session Protocol v1 over stdio).
//!
//! The kit owns one `muse serve` process per metadata probe and per session.
//! The process speaks newline-delimited JSON-RPC 2.0 (MSP); the wire shapes
//! used here were checked against `muse schema generate-json-schema` from
//! Muse Code 1.4.7. Credentials stay with Muse: the kit never reads Muse's
//! credential store, never forwards `META_API_KEY` from its own environment,
//! and never places a host MCP bearer token on a command line.
//!
//! Integration behaviour follows T3 Code's Muse provider
//! (`pingdotgg/t3code`, MIT; see THIRD_PARTY_NOTICES.md).
pub mod catalog;
pub mod client;
pub mod events;
pub mod launch;
mod wire;

pub use catalog::parse_metadata;
pub use client::{MuseOpen, Routing, open, open_spec, probe, probe_spec};
pub use launch::{Posture, resolve_executable};

use originlm_contracts::{DomainError, ErrorCode};
use serde_json::Value;

/// The kit's provider id for Muse Code.
pub const PROVIDER: &str = "muse";
/// MSP routing for Meta models, the only provider the kit selects.
pub const META_PROVIDER: &str = "meta";

pub(crate) fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Muse Session Protocol data is invalid or unsupported",
    )
}

/// A bounded single-line string without control characters.
pub(crate) fn text(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control))
}

/// At most `limit` bytes of `value`, cut on a character boundary.
pub(crate) fn bounded(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_owned();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
