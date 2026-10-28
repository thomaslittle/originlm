//! Provider integrations. Importing this crate performs no I/O.
pub mod billing_declarations;
pub mod claude;
pub mod codex;
pub mod execution;
pub mod host_mcp;
pub mod process;

pub mod acp;

pub mod grok;

pub mod http;
pub mod http_provider;
pub mod muse;
pub mod opencode;

pub mod cursor;

pub mod native_clients;

/// Semantic revision input for option descriptors. Revisions pin a host's
/// selection across probes and helper processes, so they hash only what
/// changes behaviour (ids, controls/choices/defaults, category, scope,
/// sensitivity) and never evidence sources, client versions, timestamps,
/// display labels, current values or previously computed revisions.
pub(crate) fn option_signature(
    options: &[originlm_contracts::OptionDescriptor],
) -> serde_json::Value {
    options
        .iter()
        .map(|o| {
            serde_json::json!({
                "id": o.id,
                "scope": o.scope,
                "control": o.control,
                "category": o.category,
                "billing": o.billing_sensitive,
                "permission": o.permission_sensitive,
            })
        })
        .collect()
}
