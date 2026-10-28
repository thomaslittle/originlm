//! Native Codex execution: no token extraction, shell launch, or generation on construction.
mod codex;
use crate::process::{LaunchSpec, Peer, exchange, exchange_with_identity};
use originlm_contracts::*;
use originlm_runtime::Agent;
use std::{path::PathBuf, sync::Arc, time::Duration};

pub struct ManagedCredential {
    pub access_token: String,
    pub subject: String,
    pub client_id: String,
    pub email: Option<String>,
}
pub type ManagedCredentialSource =
    Arc<dyn Fn() -> originlm_runtime::BoxFuture<'static, ManagedCredential> + Send + Sync>;
#[derive(Clone)]
pub struct CodexExecution {
    pub model: Model,
    pub connection: String,
    pub route: String,
    pub writable: bool,
    pub review_requests: bool,
    /// Preserve Codex's normal full native permission policy when explicitly granted.
    pub native_policy: bool,
    /// Validated host MCP servers attached to the thread.
    pub host_mcp_servers: Vec<HostMcpServer>,
    /// Non-ephemeral thread that `thread/resume` can reattach later.
    pub persistent: bool,
    /// Native thread id to reattach instead of starting a thread.
    pub resume_thread: Option<String>,
}
pub async fn open_codex(spec: LaunchSpec, config: CodexExecution) -> Result<Box<dyn Agent>> {
    open_codex_inner(spec, config, None).await
}

pub async fn open_managed_codex(
    spec: LaunchSpec,
    config: CodexExecution,
    identity: crate::codex::ManagedIdentity,
    source: ManagedCredentialSource,
) -> Result<Box<dyn Agent>> {
    let token = spec
        .env
        .get(std::ffi::OsStr::new("ACCESS_TOKEN"))
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| failure(ErrorCode::AuthRequired))?
        .to_owned();
    let mut template = spec.clone();
    template.env.remove(std::ffi::OsStr::new("ACCESS_TOKEN"));
    open_codex_inner(
        spec,
        config,
        Some(codex::ManagedRuntime {
            identity,
            source,
            template,
            current_token: zeroize::Zeroizing::new(token),
        }),
    )
    .await
}

async fn open_codex_inner(
    spec: LaunchSpec,
    config: CodexExecution,
    managed: Option<codex::ManagedRuntime>,
) -> Result<Box<dyn Agent>> {
    let workspace = spec.cwd.clone();
    let mut peer = Peer::spawn(spec).await?;
    let report = tokio::time::timeout(Duration::from_secs(20), async {
        match &managed {
            Some(managed) => {
                exchange_with_identity(
                    &mut peer,
                    &config.connection,
                    &config.route,
                    Some(&managed.identity),
                )
                .await
            }
            None => exchange(&mut peer, &config.connection, &config.route).await,
        }
    })
    .await
    .map_err(|_| failure(ErrorCode::Timeout))??;
    let model = report
        .models
        .iter()
        .find(|m| m.id == config.model.id)
        .ok_or_else(|| failure(ErrorCode::SelectionStale))?;
    if model.revision != config.model.revision
        || report.auth.fresh_value() != Some(&AuthState::Authenticated)
    {
        return Err(failure(ErrorCode::SelectionStale));
    }
    if report.included_usage_allowed.fresh_value() == Some(&false) {
        return Err(failure(ErrorCode::RateLimited));
    }
    let mut agent = codex::CodexAgent {
        peer,
        host: crate::host_mcp::HostTools::new(&config.host_mcp_servers),
        config,
        workspace,
        thread: None,
        resumable: false,
        managed,
    };
    if agent.config.resume_thread.is_some() {
        // Reattach now so a missing thread fails the resume, not a later turn.
        match tokio::time::timeout(Duration::from_secs(25), agent.ensure_thread()).await {
            Ok(Ok(_)) => agent.resumable = true,
            Ok(Err(error)) if error.code == ErrorCode::Timeout => {
                agent.peer.shutdown().await;
                return Err(error);
            }
            Ok(Err(_)) => {
                agent.peer.shutdown().await;
                return Err(DomainError::new(
                    ErrorCode::ResumeUnavailable,
                    "Codex could not reattach the native thread",
                ));
            }
            Err(_) => {
                agent.peer.shutdown().await;
                return Err(failure(ErrorCode::Timeout));
            }
        }
    }
    Ok(Box::new(agent))
}
fn failure(code: ErrorCode) -> DomainError {
    DomainError::new(code, "Native execution failed")
}
fn native_id(value: &serde_json::Value) -> Result<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(|| failure(ErrorCode::ProtocolIncompatible))
}
