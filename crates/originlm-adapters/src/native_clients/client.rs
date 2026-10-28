use super::{
    config::{NativeProfile, NativeProvider},
    launch::{prepare, prepare_execution},
};
use crate::acp::{
    client as acp_client,
    metadata::{self, AccountEvidence},
};
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub fn profile_evidence(profile: &NativeProfile) -> AccountEvidence {
    let provider = profile.provider.id();
    AccountEvidence {
        auth: Observation::reported(
            AuthState::Authenticated,
            EvidenceSource::new(
                EvidenceKind::OfficialProtocol,
                provider,
                "authenticated native ACP session established",
            ),
        ),
        billing: match &profile.billing {
            Some(mode) => Observation::reported(
                mode.clone(),
                EvidenceSource::new(
                    EvidenceKind::UserSupplied,
                    provider,
                    "host-declared billing mode for this native profile",
                ),
            ),
            None => Observation::unknown("BILLING_NOT_REPORTED"),
        },
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
    }
}

pub async fn probe(
    profile: &NativeProfile,
    located: &LocatedClient,
    connection: &str,
    route: &str,
    browser: Option<&Path>,
) -> Result<ConnectionMetadata> {
    if profile.provider.id() != located.provider_id
        || profile.profile_home != located.profile_home
        || Some(&profile.executable) != located.executable.as_ref()
    {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Native profile no longer matches discovered client",
        ));
    }
    let workspace = tempfile::Builder::new()
        .prefix("originlm-native-probe-")
        .tempdir()
        .map_err(|_| {
            DomainError::new(
                ErrorCode::ProviderUnavailable,
                "Native metadata workspace unavailable",
            )
        })?;
    let prepared = prepare(profile, workspace.path(), browser)?;
    let auth = prepared.auth_method.clone();
    let result = metadata::probe_with_auth(
        prepared.spec,
        profile.provider.id(),
        connection,
        route,
        profile_evidence(profile),
        auth.as_deref(),
    )
    .await;
    drop(prepared.scratch);
    result
}

/// Whether the kit can launch this client with every native tool removed.
///
/// Grok accepts `--tools=` (no built-in tools), `--disable-web-search`,
/// `--no-subagents` and `--permission-mode=dontAsk`. Gemini CLI and the
/// Antigravity ACP server have no launch flag that removes their built-in
/// tools: read-only tools (file read, glob, grep, search) run inside the
/// client without an ACP permission request, so a text-only session cannot be
/// guaranteed. Those clients are refused for text-only execution rather than
/// silently allowed to use tools; a host may still open a reviewed session
/// where write/exec requests are mediated one at a time.
pub fn text_only_capable(provider: NativeProvider) -> bool {
    matches!(provider, NativeProvider::Grok)
}
fn text_only_refused(provider: NativeProvider) -> DomainError {
    DomainError::new(
        ErrorCode::IntegrationRestricted,
        format!(
            "{} cannot disable its built-in tools; text-only sessions are refused",
            provider.id()
        ),
    )
}
pub struct NativeOpen {
    pub route: String,
    pub model: Model,
    pub review: bool,
    /// Host MCP servers; attached only to peers advertising `mcpCapabilities.http`.
    pub host_mcp_servers: Vec<HostMcpServer>,
}
pub async fn open(
    profile: &NativeProfile,
    located: &LocatedClient,
    workspace: &Path,
    browser: Option<&Path>,
    request: NativeOpen,
) -> Result<Box<dyn Agent>> {
    open_internal(profile, located, workspace, browser, request, None).await
}
pub async fn open_resume(
    profile: &NativeProfile,
    located: &LocatedClient,
    workspace: &Path,
    browser: Option<&Path>,
    request: NativeOpen,
    native_session_id: &str,
) -> Result<Box<dyn Agent>> {
    open_internal(
        profile,
        located,
        workspace,
        browser,
        request,
        Some(native_session_id),
    )
    .await
}
async fn open_internal(
    profile: &NativeProfile,
    located: &LocatedClient,
    workspace: &Path,
    browser: Option<&Path>,
    request: NativeOpen,
    resume: Option<&str>,
) -> Result<Box<dyn Agent>> {
    if profile.provider.id() != located.provider_id
        || profile.profile_home != located.profile_home
        || Some(&profile.executable) != located.executable.as_ref()
    {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Native profile no longer matches discovered client",
        ));
    }
    if !request.review && !text_only_capable(profile.provider) {
        return Err(text_only_refused(profile.provider));
    }
    let prepared = prepare_execution(profile, workspace, browser)?;
    let auth = prepared.auth_method.clone();
    let NativeOpen {
        route,
        model,
        review,
        host_mcp_servers,
    } = request;
    let inner = match resume {
        Some(native_session_id) => {
            acp_client::open_resume_with_host_mcp(
                prepared.spec,
                profile.provider.id().into(),
                route,
                model,
                review,
                auth.as_deref(),
                native_session_id,
                &host_mcp_servers,
            )
            .await?
        }
        None => {
            acp_client::open_with_host_mcp(
                prepared.spec,
                profile.provider.id().into(),
                route,
                model,
                review,
                auth.as_deref(),
                &host_mcp_servers,
            )
            .await?
        }
    };
    Ok(Box::new(OwnedNativeAgent {
        inner,
        _scratch: prepared.scratch,
    }))
}

struct OwnedNativeAgent {
    inner: Box<dyn Agent>,
    _scratch: tempfile::TempDir,
}
impl Agent for OwnedNativeAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        context: TurnContext,
    ) -> BoxFuture<'a, ()> {
        self.inner.turn(prompt, options, context)
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        self.inner.close()
    }
    fn native_resumption(&self) -> bool {
        self.inner.native_resumption()
    }
    fn image_input(&self) -> bool {
        self.inner.image_input()
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        self.inner.suspend()
    }
}
