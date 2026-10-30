//! Shared ACP v1 parsing and protocol transport.
mod catalog;
mod config;
pub use catalog::parse_catalog;
use crate::process::LaunchSpec;
use originlm_contracts::*;
use serde_json::Value;
pub use wire::acp_mcp_servers;

/// Run one ACP `authenticate` exchange. The caller supplies a browser capture
/// script in the launch environment; the server writes the authorization URL
/// there. The future completes when the provider confirms the sign-in.
pub async fn authenticate(
    spec: LaunchSpec,
    provider: String,
    route: String,
    method: &str,
) -> Result<()> {
    let mut wire = wire::Wire::connect_auth(spec, provider, route, method, &[]).await?;
    wire.close().await;
    Ok(())
}
pub struct AcpCatalog {
    pub session_id: String,
    pub models: Vec<Model>,
    pub config_options: Vec<OptionDescriptor>,
    pub load_session: bool,
    /// `agentCapabilities.promptCapabilities.image` from the ACP initialize response.
    pub image_prompt: bool,
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "ACP metadata is invalid or unsupported",
    )
}
fn text(v: &Value) -> Result<String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn evidence(provider: &str) -> EvidenceSource {
    EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        provider,
        "ACP v1 authenticated client metadata",
    )
}

pub mod client;

mod events;
mod wire;

pub mod metadata;
