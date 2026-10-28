//! Muse metadata probes and owned `muse serve` sessions.
use super::{
    META_PROVIDER, catalog,
    events::{Action, ApprovalAsk, Outcome, TurnPolicy, TurnState},
    invalid,
    launch::{self, Posture},
    wire::Wire,
};
use crate::{host_mcp::HostTools, process::LaunchSpec};
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

const STARTUP: Duration = Duration::from_secs(20);
/// After a turn that launched background work settles, how long to wait for
/// the report turn Muse starts by itself.
const FOLLOW_UP_GRACE: Duration = Duration::from_millis(1500);

/// Which Muse provider a session routes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Routing {
    /// Meta models from Muse's catalog (every kit session).
    #[default]
    Meta,
    /// Muse's deterministic, free `echo` provider. Used only by opt-in live
    /// tests: it consumes no account quota and has no model catalog.
    Echo,
}

/// What a session needs. Built by the trusted host from a validated selection.
#[derive(Clone)]
pub struct MuseOpen {
    pub model: Model,
    pub connection: String,
    pub route: String,
    /// The granted workspace; Muse's `workspaceRoot`.
    pub workspace: PathBuf,
    pub posture: Posture,
    /// Host reviews each approval (`reviewRequests`).
    pub review: bool,
    /// Muse runs without asking (`allowAll`); requires the native-policy grant.
    pub full_access: bool,
    pub host_mcp_servers: Vec<HostMcpServer>,
    /// Keep the session in Muse's history so it can be resumed.
    pub persistent: bool,
    /// Native session to reattach instead of starting one.
    pub resume: Option<String>,
    pub routing: Routing,
}

fn approval_mode(config: &MuseOpen) -> &'static str {
    if config.review {
        "promptUnmatched"
    } else if config.full_access {
        "allowAll"
    } else {
        "denyUnmatched"
    }
}

/// `SessionConfig.mcpServers`: host servers as optional streamable-HTTP
/// servers. Optional, as T3 Code attaches its own: a host server Muse cannot
/// reach leaves the turn running without those tools. The bearer token
/// travels only in this stdio frame.
pub fn session_config(servers: &[HostMcpServer]) -> Option<Value> {
    if servers.is_empty() {
        return None;
    }
    let mut map = serde_json::Map::new();
    for server in servers {
        map.insert(
            server.name.clone(),
            json!({
                "transport": "streamableHttp",
                "url": server.url,
                "headers": {"Authorization": format!("Bearer {}", server.bearer_token)},
                "mode": "optional",
            }),
        );
    }
    Some(json!({"mcpServers": map}))
}

fn routing_args(spec: &mut LaunchSpec, routing: Routing) {
    if routing == Routing::Echo {
        spec.args.extend(["--provider".into(), "echo".into()]);
    }
}

/// Metadata for a discovered Muse install. Starts a read-only, memory-only
/// `muse serve` in an empty directory; sends no prompt.
pub async fn probe(executable: &Path, connection: &str, route: &str) -> Result<ConnectionMetadata> {
    let executable = launch::resolve_executable(executable)?;
    let temp = tempfile::Builder::new()
        .prefix("originlm-muse-metadata-")
        .tempdir()
        .map_err(|_| {
            DomainError::new(
                ErrorCode::LocalAccessDenied,
                "Neutral metadata workspace unavailable",
            )
        })?;
    let spec = launch::serve_spec(
        executable,
        temp.path().into(),
        Posture::ReadOnly,
        false,
        false,
    );
    probe_spec(spec, connection, route).await
}

/// [`probe`] with an explicit launch (tests use a protocol fixture).
pub async fn probe_spec(
    spec: LaunchSpec,
    connection: &str,
    route: &str,
) -> Result<ConnectionMetadata> {
    let mut wire = tokio::time::timeout(STARTUP, Wire::connect(spec, true, false))
        .await
        .map_err(|_| DomainError::new(ErrorCode::Timeout, "Muse metadata timed out"))??;
    let outcome = tokio::time::timeout(STARTUP, async {
        let account = wire.call_raw("account/read", Value::Null).await?.ok();
        let models = wire.call("model/list", json!({})).await?;
        let usage = wire.call_raw("usage/read", json!({})).await?.ok();
        catalog::parse_metadata(
            connection,
            route,
            &wire.init,
            account.as_ref(),
            &models,
            usage.as_ref(),
        )
    })
    .await
    .unwrap_or_else(|_| {
        Err(DomainError::new(
            ErrorCode::Timeout,
            "Muse metadata timed out",
        ))
    });
    wire.shutdown().await;
    outcome
}

/// Open a session on a discovered Muse install.
pub async fn open(executable: &Path, config: MuseOpen) -> Result<Box<dyn Agent>> {
    let executable = launch::resolve_executable(executable)?;
    let mut spec = launch::serve_spec(
        executable,
        config.workspace.clone(),
        config.posture,
        config.persistent,
        config.review,
    );
    routing_args(&mut spec, config.routing);
    open_spec(spec, config).await
}

/// [`open`] with an explicit launch (tests use a protocol fixture).
pub async fn open_spec(spec: LaunchSpec, config: MuseOpen) -> Result<Box<dyn Agent>> {
    let workspace_root = launch::workspace_root(&config.workspace)?;
    let host = HostTools::new(&config.host_mcp_servers);
    let mut wire = tokio::time::timeout(STARTUP, Wire::connect(spec, true, !host.is_empty()))
        .await
        .map_err(|_| DomainError::new(ErrorCode::Timeout, "Muse startup timed out"))??;
    let outcome = tokio::time::timeout(STARTUP, start(&mut wire, &config, &workspace_root)).await;
    let session = match outcome {
        Ok(Ok(session)) => session,
        Ok(Err(error)) => {
            wire.shutdown().await;
            return Err(error);
        }
        Err(_) => {
            wire.shutdown().await;
            return Err(DomainError::new(
                ErrorCode::Timeout,
                "Muse session startup timed out",
            ));
        }
    };
    Ok(Box::new(MuseAgent {
        policy: TurnPolicy {
            posture: config.posture,
            host,
            review: config.review,
            full_access: config.full_access && !config.review,
        },
        wire: Some(wire),
        session,
        resumable: config.resume.is_some(),
        config,
    }))
}

/// Verify account and model, then start or reattach the native session.
async fn start(wire: &mut Wire, config: &MuseOpen, workspace_root: &str) -> Result<String> {
    if !config.host_mcp_servers.is_empty() && !wire.granted("sessionMcp") {
        return Err(crate::host_mcp::unsupported());
    }
    if config.routing == Routing::Meta {
        let account = wire.call_raw("account/read", Value::Null).await?.ok();
        let models = wire.call("model/list", json!({})).await?;
        let report = catalog::parse_metadata(
            &config.connection,
            &config.route,
            &wire.init,
            account.as_ref(),
            &models,
            None,
        )?;
        match report.auth.fresh_value() {
            Some(AuthState::Authenticated) => {}
            Some(AuthState::Unauthenticated | AuthState::Expired) => {
                return Err(DomainError::new(
                    ErrorCode::AuthRequired,
                    "Muse Code needs sign-in: run `muse login` on this computer",
                ));
            }
            _ => {
                return Err(DomainError::new(
                    ErrorCode::SelectionStale,
                    "Muse did not confirm the signed-in account",
                ));
            }
        }
        if !report
            .models
            .iter()
            .any(|m| m.id == config.model.id && m.revision == config.model.revision)
        {
            return Err(DomainError::new(
                ErrorCode::SelectionStale,
                "Muse's model catalog changed; refresh and select again",
            ));
        }
    }
    let mode = approval_mode(config);
    let mcp = session_config(&config.host_mcp_servers);
    match &config.resume {
        None => {
            let id = launch::uuid_v7()?;
            let mut params = json!({
                "sessionId": id,
                "workspaceRoot": workspace_root,
                "approvalMode": mode,
            });
            if config.routing == Routing::Meta {
                params["modelId"] = json!(config.model.native_id);
                params["providerId"] = json!(META_PROVIDER);
            }
            if let Some(mcp) = mcp {
                params["config"] = mcp;
            }
            let (_, result) = wire.command("session/start", params).await?;
            if result["session"]["sessionId"] != id.as_str() {
                return Err(DomainError::new(
                    ErrorCode::SessionRouteMismatch,
                    "Muse started a different session",
                ));
            }
            Ok(id)
        }
        Some(id) => {
            if !launch::valid_uuid(id) {
                return Err(DomainError::new(
                    ErrorCode::ResumeUnavailable,
                    "Invalid Muse session id",
                ));
            }
            let mut params = json!({"sessionId": id, "excludeItems": true});
            if let Some(mcp) = mcp {
                params["config"] = mcp;
            }
            let result = match wire.command_raw("session/resume", params).await? {
                Ok(result) => result,
                Err(error) => {
                    return Err(
                        if matches!(error.kind.as_str(), "sessionNotFound" | "notFound") {
                            DomainError::new(
                                ErrorCode::ResumeUnavailable,
                                "Muse no longer has this conversation",
                            )
                        } else {
                            error.domain("session/resume")
                        },
                    );
                }
            };
            if result["session"]["sessionId"] != id.as_str() {
                return Err(DomainError::new(
                    ErrorCode::SessionRouteMismatch,
                    "Muse loaded a different session",
                ));
            }
            wire.command(
                "session/setApprovalMode",
                json!({"sessionId": id, "mode": mode}),
            )
            .await?;
            if config.routing == Routing::Meta
                && result["session"]["modelId"] != config.model.native_id.as_str()
            {
                wire.command(
                    "session/setModel",
                    json!({"sessionId": id, "model": {"modelId": config.model.native_id, "providerId": META_PROVIDER}}),
                )
                .await?;
            }
            Ok(id.clone())
        }
    }
}

struct MuseAgent {
    wire: Option<Wire>,
    session: String,
    policy: TurnPolicy,
    config: MuseOpen,
    /// A turn completed (or the session was reattached), so history exists.
    resumable: bool,
}

/// MSP `TurnInputPart`s: the prompt, then one inline image part per staged image.
pub fn turn_input(prompt: &str, images: &[originlm_runtime::ImageInput]) -> Value {
    let mut parts = vec![json!({"type":"text","text":prompt})];
    parts.extend(images.iter().map(|image| {
        json!({
            "type": "image",
            "base64Data": image.base64(),
            "mediaType": image.media_type.as_str(),
            "width": image.width,
            "height": image.height,
        })
    }));
    Value::Array(parts)
}

impl MuseAgent {
    async fn execute(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        for (id, value) in &options {
            self.config
                .model
                .options
                .iter()
                .find(|d| &d.id == id)
                .ok_or_else(|| {
                    DomainError::new(ErrorCode::OptionUnsupported, "Unsupported Muse option")
                })?
                .validate(value)
                .map_err(|_| {
                    DomainError::new(ErrorCode::OptionUnsupported, "Invalid Muse option value")
                })?;
        }
        let wire = self.wire.as_mut().ok_or_else(invalid)?;
        let mut params = json!({
            "sessionId": self.session,
            "input": turn_input(&prompt, cx.images()),
            "ifBusy": "queue",
        });
        if let Some(effort) = options.get("reasoningEffort").and_then(Value::as_str) {
            params["reasoningEffort"] = json!(effort);
        }
        let started = wire
            .command_raw("turn/start", params)
            .await?
            .map_err(|error| error.domain("turn/start"))?;
        let own = started["turnId"]
            .as_str()
            .filter(|t| launch::valid_uuid(t))
            .ok_or_else(invalid)?;
        if !matches!(
            started["disposition"].as_str(),
            Some("started" | "queued") | None
        ) {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                "Muse folded the prompt into another turn",
            ));
        }
        let mut state = TurnState::new(&self.session, own, self.policy.clone());
        let mut grace: Option<tokio::time::Instant> = None;
        loop {
            if let Some(outcome) = state.own_outcome() {
                match outcome {
                    Outcome::Completed => {}
                    Outcome::Cancelled => {
                        return Err(DomainError::new(
                            ErrorCode::Cancelled,
                            "Muse cancelled the turn",
                        ));
                    }
                    Outcome::Failed(error) => return Err(error.clone()),
                }
            }
            if state.settled() {
                if !state.expects_follow_up() {
                    break;
                }
                grace.get_or_insert_with(|| tokio::time::Instant::now() + FOLLOW_UP_GRACE);
            } else {
                grace = None;
            }
            let wire = self.wire.as_mut().ok_or_else(invalid)?;
            let frame = match wire.pop_queued() {
                Some(frame) => frame,
                None => {
                    tokio::select! {
                        frame = wire.read_frame() => {
                            let frame = frame?;
                            wire.acknowledge(&frame).await?;
                            frame
                        }
                        _ = cx.cancelled() => {
                            interrupt(wire, &self.session, &state).await;
                            return Err(DomainError::new(ErrorCode::Cancelled, "Muse turn cancelled"));
                        }
                        _ = async {
                            match grace {
                                Some(deadline) => tokio::time::sleep_until(deadline).await,
                                None => std::future::pending().await,
                            }
                        } => break,
                    }
                }
            };
            for action in state.handle(&frame) {
                match action {
                    Action::Emit(payload) => cx.emit(payload)?,
                    Action::Fail(error) => return Err(error),
                    Action::DeclineQuestion { user_input_id } => {
                        let wire = self.wire.as_mut().ok_or_else(invalid)?;
                        let _ = wire
                            .command_raw(
                                "userInput/cancel",
                                json!({
                                    "sessionId": self.session,
                                    "userInputId": user_input_id,
                                    "reason": "This host cannot answer Muse questions",
                                }),
                            )
                            .await?;
                    }
                    Action::Approval(ask) => self.decide(&cx, ask).await?,
                }
            }
        }
        for payload in state.summary() {
            cx.emit(payload)?;
        }
        self.resumable = true;
        Ok(())
    }

    /// Answer one native approval: the host decides in reviewed sessions,
    /// full access approves, and everything else is declined.
    async fn decide(&mut self, cx: &TurnContext, ask: ApprovalAsk) -> Result<()> {
        let allow = if !ask.owned {
            false
        } else if self.policy.review {
            cx.approve(ask.action.clone(), ask.detail.clone()).await?
        } else {
            self.policy.full_access
        };
        if !allow && !self.policy.review && ask.owned {
            cx.emit(EventPayload::Warning {
                code: "NATIVE_REQUEST_DECLINED".into(),
                message: format!("{}; this session does not allow it", ask.action),
            })?;
        }
        let choice = if allow {
            ask.allow_choice.clone().or_else(|| ask.deny_choice.clone())
        } else {
            ask.deny_choice.clone()
        };
        let Some(choice) = choice else {
            return Err(DomainError::new(
                ErrorCode::ProtocolIncompatible,
                "Muse offered no once-only decision for an approval",
            ));
        };
        let wire = self.wire.as_mut().ok_or_else(invalid)?;
        let decided = wire
            .command_raw(
                "approval/decide",
                json!({
                    "sessionId": self.session,
                    "approvalId": ask.approval_id,
                    "requirementId": ask.requirement,
                    "choiceId": choice,
                }),
            )
            .await?;
        match decided {
            Ok(_) => Ok(()),
            // Settled meanwhile (another stage, policy or timeout): Muse's
            // own `approval/resolved` is authoritative.
            Err(error)
                if matches!(
                    error.kind.as_str(),
                    "approvalAlreadyResolved" | "approvalRequirementStale" | "approvalNotFound"
                ) =>
            {
                Ok(())
            }
            Err(error) => Err(error.domain("approval/decide")),
        }
    }
}

/// Interrupt the owned Muse turns and cancel owned workflows. Bounded: the
/// runtime closes the process shortly after a cancelled turn anyway.
async fn interrupt(wire: &mut Wire, session: &str, state: &TurnState) {
    let _ = tokio::time::timeout(Duration::from_millis(1500), async {
        let mut turns = vec![state.own_turn().to_owned()];
        turns.extend(
            state
                .active_turns()
                .into_iter()
                .filter(|t| t != state.own_turn()),
        );
        for turn in turns {
            let _ = wire
                .command_raw(
                    "turn/interrupt",
                    json!({"sessionId": session, "turnId": turn}),
                )
                .await;
        }
        for run in state.running_workflows() {
            let _ = wire
                .command_raw(
                    "workflow/cancel",
                    json!({"sessionId": session, "workflowRunId": run}),
                )
                .await;
        }
    })
    .await;
}

impl Agent for MuseAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(self.execute(prompt, options, cx))
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if let Some(mut wire) = self.wire.take() {
                wire.shutdown().await;
            }
            Ok(())
        })
    }
    fn native_resumption(&self) -> bool {
        self.config.persistent
    }
    fn image_input(&self) -> bool {
        true
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            if !(self.config.persistent && self.resumable) {
                return Err(DomainError::new(
                    ErrorCode::Conflict,
                    "Nothing to resume until a turn completes",
                ));
            }
            if let Some(mut wire) = self.wire.take() {
                wire.shutdown().await;
            }
            Ok(self.session.clone())
        })
    }
}
