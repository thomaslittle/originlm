use super::{CodexExecution, ManagedCredentialSource, PathBuf, failure, native_id};
use crate::{
    codex::ManagedIdentity,
    host_mcp::{HostTools, native_tool_refused},
    process::{LaunchSpec, Peer, exchange_with_identity},
};
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, ImageInput, TurnContext};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Duration,
};
pub(super) struct CodexAgent {
    pub peer: Peer,
    pub host: HostTools,
    pub config: CodexExecution,
    pub workspace: PathBuf,
    pub thread: Option<String>,
    /// A turn completed (or the thread was reattached), so history exists.
    pub resumable: bool,
    pub managed: Option<ManagedRuntime>,
}
pub(super) struct ManagedRuntime {
    pub identity: ManagedIdentity,
    pub source: ManagedCredentialSource,
    pub template: LaunchSpec,
    pub current_token: zeroize::Zeroizing<String>,
}
/// Thread config. Read-only sessions are text-only: besides the always-off
/// integrations, Codex's command, exec and generation tools are disabled so
/// the model cannot run anything on the host (the read-only sandbox alone
/// still permits reading commands).
///
/// Host MCP servers are added as per-thread `mcp_servers.<name>` overrides: the
/// bearer token travels as an HTTP header over the app-server's stdio pipe
/// (never argv, env or a config file), only the allowed tools are enabled, and
/// their calls are pre-approved because the host owns them.
///
/// `other_mcp_servers` are the MCP servers already configured for this Codex
/// home; a host-MCP thread disables each of them so the host's tools are the
/// only MCP tools the agent sees.
pub(crate) fn thread_config(
    writable: bool,
    native_policy: bool,
    host_mcp_servers: &[HostMcpServer],
    other_mcp_servers: &[String],
) -> Result<Value> {
    let mut config = json!({"features.apps":false,"features.plugins":false,"features.hooks":false,"features.multi_agent":false,"features.computer_use":false});
    if !native_policy {
        config["features.browser_use"] = Value::Bool(false);
        config["web_search"] = Value::String("disabled".into());
    }
    if !host_mcp_servers.is_empty() {
        // These are host-owned MCP calls, so expose them directly even for
        // models that otherwise defer tools to the disabled code-mode host.
        // Keep MCP namespaces distinct from built-ins such as `functions`.
        config["features.non_prefixed_mcp_tool_names"] = Value::Bool(false);
        let namespaces: BTreeSet<_> = host_mcp_servers
            .iter()
            .map(|server| {
                if server.name.starts_with("mcp__") {
                    server.name.clone()
                } else {
                    format!("mcp__{}", server.name)
                }
            })
            .collect();
        if namespaces.len() != host_mcp_servers.len() {
            return Err(failure(ErrorCode::IntegrationRestricted));
        }
        config["features.code_mode.direct_only_tool_namespaces"] = json!(namespaces);
    }
    for server in host_mcp_servers {
        config[format!("mcp_servers.{}", server.name)] = json!({
            "url": server.url,
            "http_headers": {"Authorization": format!("Bearer {}", server.bearer_token)},
            "enabled_tools": server.allowed_tools,
            "default_tools_approval_mode": "approve",
            "required": true,
        });
    }
    for name in other_mcp_servers {
        config[format!("mcp_servers.{name}.enabled")] = Value::Bool(false);
    }
    if !writable {
        for feature in [
            "features.shell_tool",
            "features.unified_exec",
            "features.image_generation",
            "features.tool_suggest",
            "features.goals",
            "features.sleep_tool",
        ] {
            config[feature] = Value::Bool(false);
        }
    }
    Ok(config)
}

/// `turn/start` input items: the text item, then one app-server `image`
/// item per staged image carried inline as a data URL (never a file path).
pub(crate) fn turn_input(prompt: &str, images: &[ImageInput]) -> Value {
    let mut items = vec![json!({"type":"text","text":prompt})];
    items.extend(
        images
            .iter()
            .map(|image| json!({"type":"image","url":image.data_url()})),
    );
    Value::Array(items)
}

impl CodexAgent {
    async fn refresh_managed(&mut self) -> Result<()> {
        let Some(managed) = &self.managed else {
            return Ok(());
        };
        let credential = (managed.source)().await?;
        if credential.subject != managed.identity.subject
            || credential.client_id != managed.identity.client_id
        {
            return Err(failure(ErrorCode::SelectionStale));
        }
        if credential.access_token == *managed.current_token {
            return Ok(());
        }
        let mut spec = managed.template.clone();
        spec.env.insert(
            "ACCESS_TOKEN".into(),
            credential.access_token.clone().into(),
        );
        let mut next = Peer::spawn(spec).await?;
        let report = tokio::time::timeout(
            Duration::from_secs(20),
            exchange_with_identity(
                &mut next,
                &self.config.connection,
                &self.config.route,
                Some(&managed.identity),
            ),
        )
        .await
        .map_err(|_| failure(ErrorCode::Timeout))??;
        if report.auth.fresh_value() != Some(&AuthState::Authenticated)
            || !report.models.iter().any(|model| {
                model.id == self.config.model.id && model.revision == self.config.model.revision
            })
        {
            next.shutdown().await;
            return Err(failure(ErrorCode::SelectionStale));
        }
        let mut previous = std::mem::replace(&mut self.peer, next);
        self.config.resume_thread = self.thread.take().or(self.config.resume_thread.clone());
        let resumed = self.ensure_thread().await;
        previous.shutdown().await;
        resumed?;
        if let Some(managed) = &mut self.managed {
            managed.current_token = zeroize::Zeroizing::new(credential.access_token);
        }
        Ok(())
    }
    /// Read-only, unreviewed sessions allow no client tool but the host's.
    fn text_only(&self) -> bool {
        !self.config.writable && !self.config.review_requests && !self.config.native_policy
    }
    /// Names of MCP servers configured for this Codex home (none started).
    /// Only read for host-MCP sessions; a configured server with a host
    /// server's name is refused rather than merged with the host's settings.
    async fn configured_mcp_servers(&mut self) -> Result<Vec<String>> {
        if self.host.is_empty() {
            return Ok(vec![]);
        }
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.peer.request(
                "mcpServerStatus/list",
                json!({"detail":"toolsAndAuthOnly","limit":100}),
            ),
        )
        .await
        .map_err(|_| failure(ErrorCode::Timeout))??;
        let data = response["data"]
            .as_array()
            .filter(|data| data.len() <= 100)
            .ok_or_else(|| failure(ErrorCode::ProtocolIncompatible))?;
        if response["nextCursor"].as_str().is_some() {
            return Err(failure(ErrorCode::IntegrationRestricted));
        }
        let mut names = Vec::new();
        for server in data {
            let name = server["name"]
                .as_str()
                .filter(|name| {
                    !name.is_empty()
                        && name.len() <= 128
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                })
                .ok_or_else(|| failure(ErrorCode::IntegrationRestricted))?;
            if self.host.server_names().any(|host| host == name) {
                return Err(DomainError::new(
                    ErrorCode::IntegrationRestricted,
                    "Host MCP server name is already configured in Codex",
                ));
            }
            names.push(name.to_owned());
        }
        Ok(names)
    }
    pub(super) async fn ensure_thread(&mut self) -> Result<String> {
        if let Some(thread) = &self.thread {
            return Ok(thread.clone());
        }
        let others = self.configured_mcp_servers().await?;
        // Codex 0.157+ takes the thread sandbox mode in kebab case and reports
        // it back in camel case.
        let (mode, reported) = if self.config.native_policy {
            ("danger-full-access", "dangerFullAccess")
        } else if self.config.writable {
            ("workspace-write", "workspaceWrite")
        } else {
            ("read-only", "readOnly")
        };
        let approval = if self.config.review_requests || self.config.native_policy {
            "on-request"
        } else {
            "never"
        };
        let config = thread_config(
            self.config.writable || self.config.native_policy,
            self.config.native_policy,
            &self.config.host_mcp_servers,
            &others,
        )?;
        let (method, params) = match &self.config.resume_thread {
            // Reattach a persisted thread with the same policy it was started with.
            Some(thread) => (
                "thread/resume",
                json!({
                    "threadId":thread,"model":self.config.model.native_id,"cwd":self.workspace,
                    "sandbox":mode,"approvalPolicy":approval,"approvalsReviewer":"user",
                    "serviceTier":"default","excludeTurns":true,"config":config
                }),
            ),
            None => (
                "thread/start",
                json!({
                    "model":self.config.model.native_id,"cwd":self.workspace,
                    "ephemeral":!self.config.persistent,"sandbox":mode,"approvalPolicy":approval,
                    "approvalsReviewer":"user","serviceTier":"default",
                    "serviceName":"originlm","config":config
                }),
            ),
        };
        let response =
            tokio::time::timeout(Duration::from_secs(20), self.peer.request(method, params))
                .await
                .map_err(|_| failure(ErrorCode::Timeout))??;
        if response["model"] != self.config.model.native_id
            || response["sandbox"]["type"] != reported
        {
            return Err(failure(ErrorCode::IntegrationRestricted));
        }
        let thread = native_id(&response["thread"]["id"])?;
        if self
            .config
            .resume_thread
            .as_ref()
            .is_some_and(|expected| expected != &thread)
        {
            return Err(failure(ErrorCode::SessionRouteMismatch));
        }
        self.thread = Some(thread.clone());
        Ok(thread)
    }
    async fn execute(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        if cx.is_cancelled() {
            return Err(failure(ErrorCode::Cancelled));
        }
        for (id, value) in &options {
            let d = self
                .config
                .model
                .options
                .iter()
                .find(|d| &d.id == id)
                .ok_or_else(|| failure(ErrorCode::OptionUnsupported))?;
            d.validate(value)
                .map_err(|_| failure(ErrorCode::OptionUnsupported))?;
        }
        self.refresh_managed().await?;
        tokio::time::timeout(Duration::from_secs(10), self.peer.verify_account())
            .await
            .map_err(|_| failure(ErrorCode::Timeout))??;
        let thread = self.ensure_thread().await?;
        // Codex 0.157+ rejects per-turn read access (`readOnly.access`,
        // `readOnlyAccess`); read scope now comes from permission profiles.
        let sandbox = if self.config.native_policy {
            json!({"type":"dangerFullAccess"})
        } else if self.config.writable {
            json!({"type":"workspaceWrite","writableRoots":[self.workspace],"networkAccess":false})
        } else {
            json!({"type":"readOnly"})
        };
        let params = json!({"threadId":thread,"model":self.config.model.native_id,"input":turn_input(&prompt, cx.images()),"effort":options.get("reasoningEffort"),"serviceTier":options.get("serviceTier").cloned().unwrap_or(json!("default")),"sandboxPolicy":sandbox,"approvalPolicy":if self.config.review_requests || self.config.native_policy {"on-request"}else{"never"}});
        self.peer.sequence += 1;
        let request = self.peer.sequence;
        self.peer
            .send(json!({"id":request,"method":"turn/start","params":params}))
            .await?;
        let mut turn: Option<String> = None;
        let mut early = VecDeque::new();
        let mut delivered = BTreeSet::new();
        loop {
            let message = if turn.is_some() && !early.is_empty() {
                early.pop_front().unwrap()
            } else {
                tokio::select! {
                 value=self.peer.read()=>value?,
                 _=cx.cancelled()=>{
                  if let Some(turn)=&turn{self.peer.sequence+=1;let _=self.peer.send(json!({"id":self.peer.sequence,"method":"turn/interrupt","params":{"threadId":thread,"turnId":turn}})).await;}
                  return Err(failure(ErrorCode::Cancelled));
                 }
                }
            };
            if !message.is_object() {
                return Err(failure(ErrorCode::ProtocolIncompatible));
            }
            if message.get("method").is_none() {
                if message["id"] != request || message.get("error").is_some() {
                    return Err(failure(ErrorCode::ProviderUnavailable));
                }
                if turn.is_some() {
                    return Err(failure(ErrorCode::ProtocolIncompatible));
                }
                turn = Some(native_id(&message["result"]["turn"]["id"])?);
                continue;
            }
            let method = message["method"]
                .as_str()
                .ok_or_else(|| failure(ErrorCode::ProtocolIncompatible))?;
            // A token refresh or plan-label update keeps the turn; a sign-out or
            // model reroute does not. Account switches are caught by the
            // identity re-read before every turn.
            if method == "model/rerouted"
                || (method == "account/updated"
                    && self.managed.is_none()
                    && crate::process::account_update_signs_out(&message["params"]))
            {
                return Err(failure(ErrorCode::SelectionStale));
            }
            if method == "account/updated" {
                self.peer.account_update_seen = true;
                continue;
            }
            if turn.is_none() {
                if early.len() >= 64 {
                    return Err(failure(ErrorCode::FrameTooLarge));
                }
                early.push_back(message);
                continue;
            }
            let p = &message["params"];
            if p.get("threadId").is_some_and(|id| id != &thread)
                || p.get("turnId")
                    .is_some_and(|id| Some(id.as_str().unwrap_or("")) != turn.as_deref())
            {
                return Err(failure(ErrorCode::SessionRouteMismatch));
            }
            if message.get("id").is_some() {
                if ![
                    "item/commandExecution/requestApproval",
                    "item/fileChange/requestApproval",
                ]
                .contains(&method)
                {
                    self.peer.send(json!({"id":message["id"],"error":{"code":-32601,"message":"Unsupported client request"}})).await?;
                    return Err(failure(ErrorCode::IntegrationRestricted));
                }
                if p["threadId"] != thread || p["turnId"] != turn.as_deref().unwrap_or("") {
                    return Err(failure(ErrorCode::SessionRouteMismatch));
                }
                let detail = if method.contains("commandExecution") {
                    p["command"].as_str().unwrap_or("Native command request")
                } else {
                    "Native file change request"
                };
                let escalation = [
                    "additionalPermissions",
                    "networkApprovalContext",
                    "proposedExecpolicyAmendment",
                    "grantRoot",
                ]
                .iter()
                .any(|k| p.get(k).is_some_and(|v| !v.is_null()));
                let allowed = if self.config.review_requests && !escalation {
                    cx.approve(
                        "Review native operation".into(),
                        detail.chars().take(8192).collect(),
                    )
                    .await?
                        && (self.config.writable || self.config.native_policy)
                } else {
                    false
                };
                self.peer.send(json!({"id":message["id"],"result":{"decision":if allowed{"accept"}else{"decline"}}})).await?;
                continue;
            }
            match method {
                "item/agentMessage/delta" => {
                    let text = p["delta"]
                        .as_str()
                        .ok_or_else(|| failure(ErrorCode::ProtocolIncompatible))?;
                    delivered.insert(p["itemId"].as_str().unwrap_or("").to_owned());
                    emit_text(&cx, text, false)?;
                }
                "item/reasoning/summaryTextDelta" => {
                    if let Some(text) = p["delta"].as_str() {
                        emit_text(&cx, text, true)?;
                    }
                }
                "item/started" | "item/completed" => {
                    let item = &p["item"];
                    match item["type"].as_str().unwrap_or("") {
                        "agentMessage"
                            if method == "item/completed"
                                && !delivered.contains(item["id"].as_str().unwrap_or("")) =>
                        {
                            if let Some(text) = item["text"].as_str() {
                                emit_text(&cx, text, false)?;
                            }
                        }
                        "mcpToolCall"
                            if self.host.allows(
                                item["server"].as_str().unwrap_or(""),
                                item["tool"].as_str().unwrap_or(""),
                            ) =>
                        {
                            cx.emit(EventPayload::Tool {
                                name: format!(
                                    "mcp__{}__{}",
                                    item["server"].as_str().unwrap_or(""),
                                    item["tool"].as_str().unwrap_or("")
                                ),
                                status: codex_tool_status(method, item).into(),
                            })?;
                        }
                        kind if self.text_only() && native_tool_item(kind) => {
                            if method == "item/started" {
                                self.peer.sequence += 1;
                                let _ = self.peer.send(json!({"id":self.peer.sequence,"method":"turn/interrupt","params":{"threadId":thread,"turnId":turn}})).await;
                            }
                            return Err(native_tool_refused());
                        }
                        "commandExecution" | "fileChange" | "mcpToolCall" => {
                            cx.emit(EventPayload::Tool {
                                name: item["type"].as_str().unwrap().into(),
                                status: if method == "item/started" {
                                    "started"
                                } else {
                                    "completed"
                                }
                                .into(),
                            })?;
                        }
                        _ => {}
                    }
                }
                "thread/tokenUsage/updated" => {
                    if let Some(total) = p["tokenUsage"]["total"]["totalTokens"].as_u64() {
                        cx.emit(EventPayload::Usage {
                            unit: "session_tokens".into(),
                            used: total.to_string(),
                        })?;
                    }
                }
                "turn/completed" => {
                    if p["turn"]["id"].as_str() != turn.as_deref() {
                        return Err(failure(ErrorCode::SessionRouteMismatch));
                    }
                    return match p["turn"]["status"].as_str() {
                        Some("completed") => {
                            self.resumable = true;
                            Ok(())
                        }
                        Some("interrupted") => Err(failure(ErrorCode::Cancelled)),
                        _ => {
                            let detail = p["turn"]["error"]["message"]
                                .as_str()
                                .or_else(|| p["turn"]["error"].as_str())
                                .filter(|message| !message.trim().is_empty())
                                .map(redact_diagnostic_detail)
                                .filter(|message| !message.is_empty())
                                .unwrap_or_else(|| "Codex app-server turn did not complete".into());
                            Err(DomainError::new(
                                ErrorCode::ProviderUnavailable,
                                format!("Codex app-server turn: {detail}"),
                            ))
                        }
                    };
                }
                "error" => {
                    let detail = p["error"]["message"]
                        .as_str()
                        .or_else(|| p["message"].as_str())
                        .filter(|message| !message.trim().is_empty())
                        .map(redact_diagnostic_detail)
                        .filter(|message| !message.is_empty())
                        .unwrap_or_else(|| "Codex app-server reported a turn error".into());
                    return Err(DomainError::new(
                        ErrorCode::ProviderUnavailable,
                        format!("Codex app-server turn: {detail}"),
                    ));
                }
                _ => {}
            }
        }
    }
}
/// Thread items that are the client's own tools, never allowed in a text-only
/// session (an `mcpToolCall` reaches here only when it is not a host tool).
pub(crate) fn native_tool_item(kind: &str) -> bool {
    matches!(
        kind,
        "commandExecution"
            | "fileChange"
            | "mcpToolCall"
            | "dynamicToolCall"
            | "webSearch"
            | "imageGeneration"
            | "collabAgentToolCall"
    )
}
fn codex_tool_status(method: &str, item: &Value) -> &'static str {
    match (method, item["status"].as_str()) {
        ("item/started", _) => "started",
        (_, Some("failed")) => "failed",
        _ => "completed",
    }
}
fn emit_text(cx: &TurnContext, text: &str, reasoning: bool) -> Result<()> {
    let mut chunk = String::new();
    for ch in text.chars() {
        chunk.push(ch);
        if chunk.len() >= 16_000 {
            cx.emit(if reasoning {
                EventPayload::ReasoningSummary {
                    text: std::mem::take(&mut chunk),
                }
            } else {
                EventPayload::TextDelta {
                    text: std::mem::take(&mut chunk),
                }
            })?;
        }
    }
    if !chunk.is_empty() {
        cx.emit(if reasoning {
            EventPayload::ReasoningSummary { text: chunk }
        } else {
            EventPayload::TextDelta { text: chunk }
        })?;
    }
    Ok(())
}
impl Agent for CodexAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        context: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(self.execute(prompt, options, context))
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.peer.shutdown().await;
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
            let thread = match (&self.thread, self.config.persistent && self.resumable) {
                (Some(thread), true) => thread.clone(),
                _ => {
                    return Err(DomainError::new(
                        ErrorCode::Conflict,
                        "Nothing to resume until a turn completes",
                    ));
                }
            };
            self.peer.shutdown().await;
            Ok(thread)
        })
    }
}

#[cfg(test)]
mod direct_namespace_tests {
    use super::*;

    #[test]
    fn host_namespace_cannot_make_builtin_functions_direct() {
        let server = HostMcpServer {
            name: "functions".into(),
            url: "http://127.0.0.1:48123/mcp".into(),
            bearer_token: "k".repeat(40),
            allowed_tools: vec!["echo".into()],
        };
        assert!(server.validate());
        let config = thread_config(false, false, std::slice::from_ref(&server), &[]).unwrap();
        assert_eq!(config["features.non_prefixed_mcp_tool_names"], false);
        assert_eq!(
            config["features.code_mode.direct_only_tool_namespaces"],
            json!(["mcp__functions"])
        );
        assert_eq!(config["features.shell_tool"], false);
        assert_eq!(config["features.unified_exec"], false);
        let prefixed = HostMcpServer {
            name: "mcp__functions".into(),
            ..server.clone()
        };
        let config = thread_config(false, false, std::slice::from_ref(&prefixed), &[]).unwrap();
        assert_eq!(
            config["features.code_mode.direct_only_tool_namespaces"],
            json!(["mcp__functions"])
        );
        assert_eq!(
            thread_config(false, false, &[server, prefixed], &[])
                .unwrap_err()
                .code,
            ErrorCode::IntegrationRestricted
        );
        let plain = thread_config(false, false, &[], &[]).unwrap();
        assert!(
            plain
                .get("features.code_mode.direct_only_tool_namespaces")
                .is_none()
        );
    }
}
