use super::{invalid, parse_metadata};
use crate::{
    host_mcp::{HostTools, native_tool_refused},
    process::{LaunchSpec, Peer},
};
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, ImageInput, TurnContext};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
async fn control(peer: &mut Peer, request: Value) -> Result<Value> {
    peer.sequence += 1;
    let id = format!("originlm-{}", peer.sequence);
    peer.send(json!({"type":"control_request","request_id":id,"request":request}))
        .await?;
    for _ in 0..128 {
        let response = peer.read().await?;
        match response["type"].as_str() {
            Some("control_response") if response["response"]["request_id"] == id => {
                return if response["response"]["subtype"] == "success" {
                    Ok(response["response"]["response"].clone())
                } else {
                    Err(DomainError::new(
                        ErrorCode::CapabilityUnsupported,
                        "Native control operation unavailable",
                    ))
                };
            }
            Some("system" | "rate_limit_event") => {}
            _ => return Err(invalid()),
        }
    }
    Err(invalid())
}
async fn initialize(peer: &mut Peer) -> Result<Value> {
    initialize_policy(peer, false).await
}
async fn initialize_policy(peer: &mut Peer, native_policy: bool) -> Result<Value> {
    let value = control(
        peer,
        json!({"subtype":"initialize","hooks":null,"skills":[]}),
    )
    .await?;
    if !native_policy
        && value["current_permission_mode"]
            .as_str()
            .is_some_and(|p| p != "dontAsk" && p != "plan")
    {
        return Err(DomainError::new(
            ErrorCode::IntegrationRestricted,
            "Native permission mode mismatch",
        ));
    }
    Ok(value)
}
fn native_approval_detail(request: &Value) -> String {
    let tool = request["tool_name"]
        .as_str()
        .filter(|name| !name.is_empty() && name.len() <= 256)
        .unwrap_or("Native client operation");
    let input = &request["input"];
    let context = if matches!(
        tool,
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit"
    ) {
        input["file_path"]
            .as_str()
            .or_else(|| input["path"].as_str())
            .map(|path| format!(" {path}"))
    } else if tool.eq_ignore_ascii_case("Bash") || tool.eq_ignore_ascii_case("PowerShell") {
        input["command"]
            .as_str()
            .or_else(|| request["command"].as_str())
            .map(|command| format!(" {command}"))
    } else {
        None
    };
    let detail = format!("{tool}{}", context.unwrap_or_default());
    originlm_contracts::redact_diagnostic_detail(&detail)
        .chars()
        .take(1024)
        .collect()
}
/// Waits until every host MCP server reports `connected` via the
/// `mcp_status` control request (no user turn, no inference). A server that
/// fails, needs auth, or stays pending fails the session typed.
async fn wait_host_mcp(peer: &mut Peer, host: &HostTools) -> Result<()> {
    if host.is_empty() {
        return Ok(());
    }
    let unavailable =
        |message: &'static str| DomainError::new(ErrorCode::ProviderUnavailable, message);
    for _ in 0..40 {
        let status = control(peer, json!({"subtype":"mcp_status"})).await?;
        let servers = status["mcpServers"]
            .as_array()
            .filter(|servers| servers.len() <= 64)
            .ok_or_else(invalid)?;
        let mut pending = false;
        for name in host.server_names() {
            match servers
                .iter()
                .find(|server| server["name"] == name)
                .and_then(|server| server["status"].as_str())
            {
                Some("connected") => {}
                Some("pending") | None => pending = true,
                Some(_) => return Err(unavailable("Host MCP server did not connect")),
            }
        }
        if !pending {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(unavailable("Host MCP server did not connect in time"))
}
/// Starts the client with host MCP servers attached and confirms they connect,
/// without sending a prompt. Lets a host verify wiring before any turn.
pub async fn probe_host_mcp(spec: LaunchSpec, host: &HostTools) -> Result<()> {
    probe_host_mcp_policy(spec, host, false).await
}
pub async fn probe_host_mcp_policy(
    spec: LaunchSpec,
    host: &HostTools,
    native_policy: bool,
) -> Result<()> {
    let mut peer = Peer::spawn(spec).await?;
    let outcome = tokio::time::timeout(Duration::from_secs(30), async {
        initialize_policy(&mut peer, native_policy).await?;
        wait_host_mcp(&mut peer, host).await
    })
    .await
    .unwrap_or_else(|_| {
        Err(DomainError::new(
            ErrorCode::Timeout,
            "Host MCP startup timed out",
        ))
    });
    peer.shutdown().await;
    outcome
}
pub async fn probe_spec(
    spec: LaunchSpec,
    connection: &str,
    route: &str,
) -> Result<ConnectionMetadata> {
    let mut peer = Peer::spawn(spec).await?;
    let outcome = tokio::time::timeout(Duration::from_secs(20), async {
        let init = initialize(&mut peer).await?;
        let usage = tokio::time::timeout(
            Duration::from_secs(4),
            control(&mut peer, json!({"subtype":"get_usage"})),
        )
        .await
        .ok()
        .and_then(Result::ok);
        parse_metadata(connection, route, &init, usage.as_ref())
    })
    .await
    .map_err(|_| DomainError::new(ErrorCode::Timeout, "Claude metadata timed out"))?;
    peer.shutdown().await;
    outcome
}
pub fn executor(
    spec: LaunchSpec,
    model: Model,
    connection: String,
    route: String,
) -> Box<dyn Agent> {
    host_executor(
        super::launch::HostMcpLaunch {
            spec,
            config_dir: None,
        },
        HostTools::default(),
        model,
        connection,
        route,
    )
}
/// A text-only session whose only tools are the host's MCP tools.
pub fn host_executor(
    launch: super::launch::HostMcpLaunch,
    host: HostTools,
    model: Model,
    connection: String,
    route: String,
) -> Box<dyn Agent> {
    host_executor_policy(launch, host, model, connection, route, false, false)
}
pub fn host_executor_policy(
    launch: super::launch::HostMcpLaunch,
    host: HostTools,
    model: Model,
    connection: String,
    route: String,
    native_policy: bool,
    review_requests: bool,
) -> Box<dyn Agent> {
    let mut result = agent(launch, host, model, connection, route);
    result.native_policy = native_policy;
    result.review_requests = review_requests;
    Box::new(result)
}
/// A persisted conversation. `launch.spec` must already carry
/// [`super::launch::persist`] for `session_id`; `resumed` marks a reattached
/// conversation that already has history.
pub fn persistent_executor(
    launch: super::launch::HostMcpLaunch,
    host: HostTools,
    model: Model,
    connection: String,
    route: String,
    session_id: String,
    resumed: bool,
) -> Box<dyn Agent> {
    persistent_executor_policy(
        launch, host, model, connection, route, session_id, resumed, false, false,
    )
}
// Keep this policy-aware factory aligned with `host_executor_policy` and the
// persisted-session inputs; these distinct values are part of the adapter API.
#[allow(clippy::too_many_arguments)]
pub fn persistent_executor_policy(
    launch: super::launch::HostMcpLaunch,
    host: HostTools,
    model: Model,
    connection: String,
    route: String,
    session_id: String,
    resumed: bool,
    native_policy: bool,
    review_requests: bool,
) -> Box<dyn Agent> {
    let mut agent = agent(launch, host, model, connection, route);
    agent.native_policy = native_policy;
    agent.review_requests = review_requests;
    agent.session = Some(session_id);
    agent.persistent = true;
    agent.resumable = resumed;
    Box::new(agent)
}
fn agent(
    launch: super::launch::HostMcpLaunch,
    host: HostTools,
    model: Model,
    connection: String,
    route: String,
) -> ClaudeAgent {
    ClaudeAgent {
        spec: Some(launch.spec),
        mcp_config: launch.config_dir,
        host,
        native_policy: false,
        review_requests: false,
        tool_uses: BTreeMap::new(),
        peer: None,
        model,
        connection,
        route,
        options: None,
        session: None,
        persistent: false,
        resumable: false,
    }
}
struct ClaudeAgent {
    spec: Option<LaunchSpec>,
    /// Private directory holding the per-session MCP config; deleted on close.
    mcp_config: Option<tempfile::TempDir>,
    host: HostTools,
    native_policy: bool,
    review_requests: bool,
    /// Host tool_use ids of the current turn, for completion events.
    tool_uses: BTreeMap<String, String>,
    peer: Option<Peer>,
    model: Model,
    connection: String,
    route: String,
    options: Option<BTreeMap<String, Value>>,
    session: Option<String>,
    /// Launched with `--session-id`/`--resume` instead of `--no-session-persistence`.
    persistent: bool,
    /// History exists (a turn completed, or the session was resumed).
    resumable: bool,
}
impl ClaudeAgent {
    async fn execute(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        for (id, value) in &options {
            self.model
                .options
                .iter()
                .find(|d| &d.id == id)
                .ok_or_else(invalid)?
                .validate(value)
                .map_err(|_| invalid())?;
        }
        if let Some(previous) = &self.options
            && previous != &options
        {
            return Err(DomainError::new(
                ErrorCode::OptionUnsupported,
                "This client applies effort at session startup; create a new session",
            ));
        }
        if let Some(mut spec) = self.spec.take() {
            spec.args
                .push(format!("--model={}", self.model.native_id).into());
            if let Some(effort) = options.get("reasoningEffort").and_then(Value::as_str) {
                spec.args.push(format!("--effort={effort}").into());
            }
            let mut peer = Peer::spawn(spec).await?;
            let init = tokio::time::timeout(
                Duration::from_secs(20),
                initialize_policy(&mut peer, self.native_policy),
            )
            .await
            .map_err(|_| {
                DomainError::new(ErrorCode::Timeout, "Claude initialization timed out")
            })??;
            let metadata = parse_metadata(&self.connection, &self.route, &init, None)?;
            tokio::time::timeout(
                Duration::from_secs(15),
                wait_host_mcp(&mut peer, &self.host),
            )
            .await
            .map_err(|_| DomainError::new(ErrorCode::Timeout, "Host MCP startup timed out"))??;
            if metadata.auth.fresh_value() != Some(&AuthState::Authenticated)
                || !metadata
                    .models
                    .iter()
                    .any(|m| m.id == self.model.id && m.revision == self.model.revision)
            {
                return Err(DomainError::new(
                    ErrorCode::SelectionStale,
                    "Native account or model selection changed",
                ));
            }
            self.peer = Some(peer);
            self.options = Some(options);
        }
        let peer = self.peer.as_mut().ok_or_else(invalid)?;
        let content = user_content(&prompt, cx.images());
        peer.send(json!({"type":"user","message":{"role":"user","content":content},"parent_tool_use_id":null,"session_id":self.session.as_deref().unwrap_or(""),"client_composed":true})).await?;
        let mut streamed = false;
        loop {
            let response = tokio::select! {value=peer.read()=>value?,_=cx.cancelled()=>{peer.sequence+=1;let _=peer.send(json!({"type":"control_request","request_id":format!("cancel-{}",peer.sequence),"request":{"subtype":"interrupt"}})).await;return Err(DomainError::new(ErrorCode::Cancelled,"Native turn cancelled"));}};
            if let Some(id) = response["session_id"].as_str() {
                if id.len() > 256 || id.chars().any(char::is_control) {
                    return Err(invalid());
                }
                if let Some(expected) = &self.session {
                    if !id.is_empty() && id != expected {
                        return Err(DomainError::new(
                            ErrorCode::SessionRouteMismatch,
                            "Native session changed",
                        ));
                    }
                } else if !id.is_empty() {
                    self.session = Some(id.into());
                }
            }
            match response["type"].as_str() {
                Some("control_request") => {
                    let id = response["request_id"]
                        .as_str()
                        .filter(|s| s.len() <= 256)
                        .ok_or_else(invalid)?;
                    let request = &response["request"];
                    let detail = native_approval_detail(request);
                    let allowed = self.native_policy
                        && self.review_requests
                        && cx.approve("Review native operation".into(), detail).await?;
                    let decision = if allowed {
                        json!({"behavior":"allow","updatedInput":request["input"],"interrupt":false})
                    } else {
                        json!({"behavior":"deny","decisionReason":"Native operation was not approved by the host","interrupt":true})
                    };
                    peer.send(json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":decision}})).await?;
                    if !allowed {
                        return Err(DomainError::new(
                            ErrorCode::IntegrationRestricted,
                            "Native operation was not approved",
                        ));
                    }
                }
                Some("system") => {
                    if !self.native_policy
                        && response["subtype"] == "init"
                        && !advertised_tools_allowed(&self.host, &response["tools"])
                    {
                        return Err(DomainError::new(
                            ErrorCode::IntegrationRestricted,
                            "Text-only native client advertised tools",
                        ));
                    }
                }
                Some("stream_event") => {
                    let event = &response["event"];
                    if event["type"] == "content_block_start" {
                        let block = &event["content_block"];
                        if self.native_policy
                            && matches!(block["type"].as_str(), Some("tool_use" | "mcp_tool_use"))
                        {
                            if block["name"]
                                .as_str()
                                .filter(|name| !name.is_empty() && name.len() <= 256)
                                .is_none()
                            {
                                return Err(native_tool_refused());
                            }
                        } else {
                            check_block_start(&self.host, block)?;
                        }
                    }
                    if event["type"] == "content_block_delta" {
                        let delta = &event["delta"];
                        if let Some(text) = delta["text"].as_str() {
                            streamed = true;
                            emit_text(&cx, text)?;
                        } else if let Some(text) = delta["thinking"].as_str() {
                            cx.emit(EventPayload::ReasoningSummary {
                                text: text.chars().take(16000).collect(),
                            })?;
                        }
                    }
                }
                Some("assistant") => {
                    if let Some(parts) = response["message"]["content"].as_array() {
                        for part in parts {
                            match part["type"].as_str() {
                                Some("text") => {
                                    if let Some(text) = part["text"].as_str().filter(|_| !streamed)
                                    {
                                        emit_text(&cx, text)?;
                                    }
                                }
                                Some("thinking" | "redacted_thinking") => {}
                                _ => {
                                    let is_host = part["name"]
                                        .as_str()
                                        .is_some_and(|name| self.host.allows_qualified(name));
                                    let id = if is_host {
                                        host_tool_use(&self.host, part)?
                                    } else if self.native_policy
                                        && matches!(
                                            part["type"].as_str(),
                                            Some("tool_use" | "mcp_tool_use")
                                        )
                                    {
                                        part["id"]
                                            .as_str()
                                            .filter(|id| !id.is_empty() && id.len() <= 256)
                                            .ok_or_else(invalid)?
                                            .to_owned()
                                    } else {
                                        return Err(native_tool_refused());
                                    };
                                    if self.tool_uses.len() >= 1024 {
                                        return Err(invalid());
                                    }
                                    let name = part["name"].as_str().unwrap_or("").to_owned();
                                    self.tool_uses.insert(id, name.clone());
                                    if !is_host && !self.native_policy {
                                        return Err(native_tool_refused());
                                    }
                                    cx.emit(EventPayload::Tool {
                                        name,
                                        status: "started".into(),
                                    })?;
                                }
                            }
                        }
                    }
                }
                Some("user") => {
                    if let Some(parts) = response["message"]["content"].as_array() {
                        for part in parts.iter().filter(|part| part["type"] == "tool_result") {
                            let id = part["tool_use_id"].as_str().unwrap_or("");
                            let name = self.tool_uses.remove(id).ok_or_else(native_tool_refused)?;
                            cx.emit(EventPayload::Tool {
                                name,
                                status: if part["is_error"] == true {
                                    "failed"
                                } else {
                                    "completed"
                                }
                                .into(),
                            })?;
                        }
                    }
                }
                Some("result") => {
                    if response["is_error"] == true || response["subtype"] != "success" {
                        return Err(DomainError::new(
                            ErrorCode::ProviderUnavailable,
                            "Claude turn failed",
                        ));
                    }
                    self.resumable = true;
                    for (key, unit) in [
                        ("input_tokens", "input_tokens"),
                        ("output_tokens", "output_tokens"),
                    ] {
                        if let Some(used) = response["usage"][key].as_u64() {
                            cx.emit(EventPayload::Usage {
                                unit: unit.into(),
                                used: used.to_string(),
                            })?;
                        }
                    }
                    return Ok(());
                }
                Some("rate_limit_event" | "control_cancel_request") => {}
                _ => return Err(invalid()),
            }
        }
    }
}
/// The stream-json user message content: the bare prompt string for a
/// text-only turn, otherwise base64 image blocks followed by the text block
/// (the Messages API content shape Claude Code forwards unchanged).
pub fn user_content(prompt: &str, images: &[ImageInput]) -> Value {
    if images.is_empty() {
        return Value::String(prompt.into());
    }
    let mut blocks: Vec<Value> = images
        .iter()
        .map(|image| {
            json!({"type":"image","source":{"type":"base64","media_type":image.media_type.as_str(),"data":image.base64()}})
        })
        .collect();
    blocks.push(json!({"type":"text","text":prompt}));
    Value::Array(blocks)
}
/// A text-only session advertises no tools, or only the host's MCP tools.
pub fn advertised_tools_allowed(host: &HostTools, tools: &Value) -> bool {
    match tools {
        Value::Null => true,
        Value::Array(tools) => tools.iter().all(|tool| {
            tool.as_str()
                .is_some_and(|name| host.allows_qualified(name))
        }),
        _ => false,
    }
}
/// A streamed content block may be text or thinking, or an allowed host tool.
pub fn check_block_start(host: &HostTools, block: &Value) -> Result<()> {
    match block["type"].as_str() {
        Some("text" | "thinking" | "redacted_thinking") => Ok(()),
        _ if block["name"]
            .as_str()
            .is_some_and(|name| host.allows_qualified(name)) =>
        {
            Ok(())
        }
        _ => Err(native_tool_refused()),
    }
}
/// The id of an allowed host `tool_use` block; any other block fails the turn.
pub fn host_tool_use(host: &HostTools, part: &Value) -> Result<String> {
    if !matches!(part["type"].as_str(), Some("tool_use" | "mcp_tool_use"))
        || !part["name"]
            .as_str()
            .is_some_and(|name| host.allows_qualified(name))
    {
        return Err(native_tool_refused());
    }
    part["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn emit_text(cx: &TurnContext, text: &str) -> Result<()> {
    let mut chunk = String::new();
    for ch in text.chars() {
        chunk.push(ch);
        if chunk.len() >= 16000 {
            cx.emit(EventPayload::TextDelta {
                text: std::mem::take(&mut chunk),
            })?;
        }
    }
    if !chunk.is_empty() {
        cx.emit(EventPayload::TextDelta { text: chunk })?;
    }
    Ok(())
}
impl Agent for ClaudeAgent {
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
            if let Some(mut peer) = self.peer.take() {
                peer.shutdown().await;
            }
            // Deletes the MCP config, and with it the host's bearer token.
            self.mcp_config.take();
            self.spec.take();
            Ok(())
        })
    }
    fn native_resumption(&self) -> bool {
        self.persistent
    }
    fn image_input(&self) -> bool {
        true
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            let session = match (&self.session, self.persistent && self.resumable) {
                (Some(session), true) => session.clone(),
                _ => {
                    return Err(DomainError::new(
                        ErrorCode::Conflict,
                        "Nothing to resume until a turn completes",
                    ));
                }
            };
            if let Some(mut peer) = self.peer.take() {
                peer.shutdown().await;
            }
            self.mcp_config.take();
            self.spec.take();
            Ok(session)
        })
    }
}
