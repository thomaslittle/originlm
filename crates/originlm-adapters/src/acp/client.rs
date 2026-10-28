use super::{
    AcpCatalog, invalid,
    wire::{Wire, response_result},
};
use crate::{
    host_mcp::{HostTools, native_tool_refused},
    process::LaunchSpec,
};
use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
pub async fn inspect(spec: LaunchSpec, provider: &str, route: &str) -> Result<AcpCatalog> {
    let mut wire = Wire::connect(spec, provider.into(), route.into()).await?;
    let result = wire.catalog();
    wire.close().await;
    result
}
/// The host must select a trusted native client and explicitly own its permission policy.
/// ACP callbacks alone are not an OS sandbox for the native agent's built-in tools.
pub async fn open(
    spec: LaunchSpec,
    provider: String,
    route: String,
    model: Model,
    review: bool,
) -> Result<Box<dyn Agent>> {
    open_with_auth(spec, provider, route, model, review, None).await
}
pub async fn open_with_auth(
    spec: LaunchSpec,
    provider: String,
    route: String,
    model: Model,
    review: bool,
    auth: Option<&str>,
) -> Result<Box<dyn Agent>> {
    open_with_host_mcp(spec, provider, route, model, review, auth, &[]).await
}
/// Like [`open_with_auth`], with host MCP servers attached to the ACP session.
/// Refused with `CAPABILITY_UNSUPPORTED` unless the peer advertises
/// `mcpCapabilities.http`.
pub async fn open_with_host_mcp(
    spec: LaunchSpec,
    provider: String,
    route: String,
    model: Model,
    review: bool,
    auth: Option<&str>,
    host_mcp_servers: &[HostMcpServer],
) -> Result<Box<dyn Agent>> {
    let wire = Wire::connect_with_auth(spec, provider, route, auth, host_mcp_servers).await?;
    finish_open(wire, model, review, HostTools::new(host_mcp_servers)).await
}
pub async fn open_resume(
    spec: LaunchSpec,
    provider: String,
    route: String,
    model: Model,
    review: bool,
    auth: Option<&str>,
    native_session_id: &str,
) -> Result<Box<dyn Agent>> {
    open_resume_with_host_mcp(
        spec,
        provider,
        route,
        model,
        review,
        auth,
        native_session_id,
        &[],
    )
    .await
}
#[allow(clippy::too_many_arguments)]
pub async fn open_resume_with_host_mcp(
    spec: LaunchSpec,
    provider: String,
    route: String,
    model: Model,
    review: bool,
    auth: Option<&str>,
    native_session_id: &str,
    host_mcp_servers: &[HostMcpServer],
) -> Result<Box<dyn Agent>> {
    let wire = Wire::connect_resume_with_auth(
        spec,
        provider,
        route,
        auth,
        native_session_id,
        host_mcp_servers,
    )
    .await?;
    finish_open(wire, model, review, HostTools::new(host_mcp_servers)).await
}
async fn finish_open(
    mut wire: Wire,
    model: Model,
    review: bool,
    host: HostTools,
) -> Result<Box<dyn Agent>> {
    if !wire
        .catalog()?
        .models
        .iter()
        .any(|m| m.id == model.id && m.revision == model.revision)
    {
        wire.close().await;
        return Err(DomainError::new(
            ErrorCode::SelectionStale,
            "ACP model configuration changed",
        ));
    }
    Ok(Box::new(AcpAgent {
        wire,
        model,
        review,
        host,
        host_calls: BTreeMap::new(),
    }))
}
struct AcpAgent {
    wire: Wire,
    model: Model,
    review: bool,
    host: HostTools,
    /// `toolCallId`s the peer opened for allowed host tools.
    host_calls: BTreeMap<String, String>,
}
/// The qualified host tool an ACP tool call names, if it is one. ACP does not
/// say which MCP server a call came from, so the call must carry no native
/// tool kind and its title must resolve to exactly one allowed host tool.
pub fn acp_host_tool(host: &HostTools, call: &Value) -> Option<String> {
    if !matches!(call["kind"].as_str(), None | Some("other")) {
        return None;
    }
    call["title"].as_str().and_then(|title| host.resolve(title))
}
impl AcpAgent {
    /// Handles a tool update for an allowed host tool; `false` leaves every
    /// other update (including non-host tools) to the normal path.
    fn host_update(&mut self, update: &Value, cx: &TurnContext) -> Result<bool> {
        let id = update["toolCallId"].as_str().unwrap_or("");
        let name = match update["sessionUpdate"].as_str() {
            Some("tool_call") => match acp_host_tool(&self.host, update) {
                Some(name) if !id.is_empty() && id.len() <= 256 => {
                    if self.host_calls.len() >= 1024 {
                        return Err(invalid());
                    }
                    self.host_calls.insert(id.to_owned(), name.clone());
                    name
                }
                _ => return Ok(false),
            },
            Some("tool_call_update") => match self.host_calls.get(id) {
                Some(name) => name.clone(),
                None => return Ok(false),
            },
            _ => return Ok(false),
        };
        cx.emit(EventPayload::Tool {
            name,
            status: update["status"]
                .as_str()
                .unwrap_or("pending")
                .chars()
                .take(40)
                .collect(),
        })?;
        Ok(true)
    }
    /// Allows a permission request for an allowed host tool once; `false`
    /// leaves every other request to the normal (deny or review) path.
    async fn host_permission(&mut self, request: &Value, sid: &Value) -> Result<bool> {
        let p = &request["params"];
        if request["method"] != "session/request_permission" || &p["sessionId"] != sid {
            return Ok(false);
        }
        let call = &p["toolCall"];
        let known = call["toolCallId"]
            .as_str()
            .is_some_and(|id| self.host_calls.contains_key(id));
        if !known && acp_host_tool(&self.host, call).is_none() {
            return Ok(false);
        }
        let Some(option) = p["options"]
            .as_array()
            .filter(|a| a.len() <= 32)
            .and_then(|a| a.iter().find(|c| c["kind"] == "allow_once"))
            .and_then(|c| c["optionId"].as_str())
            .map(str::to_owned)
        else {
            return Ok(false);
        };
        self.wire
            .peer
            .send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"outcome":{"outcome":"selected","optionId":option}}}))
            .await?;
        Ok(true)
    }
    async fn configure(&mut self, options: &BTreeMap<String, Value>) -> Result<()> {
        let catalog = self.wire.catalog()?;
        let sid = catalog.session_id;
        if !catalog
            .models
            .iter()
            .any(|m| m.native_id == self.model.native_id && m.is_default)
        {
            if let Some(model) = catalog
                .config_options
                .iter()
                .find(|d| d.category.as_deref() == Some("model") || d.id == "model")
            {
                let response = self
                    .wire
                    .call(
                        "session/set_config_option",
                        json!({"sessionId":sid,"configId":model.id,"value":self.model.native_id}),
                    )
                    .await?;
                self.wire.session["configOptions"] = response["configOptions"].clone();
            } else {
                self.wire
                    .call(
                        "session/set_model",
                        json!({"sessionId":sid,"modelId":self.model.native_id}),
                    )
                    .await?;
                self.wire.session["models"]["currentModelId"] = json!(self.model.native_id);
            }
            self.wire.apply_notices()?;
        }
        for (id, value) in options {
            let config = self.wire.catalog()?;
            let descriptor = config
                .config_options
                .iter()
                .find(|d| d.id == *id)
                .ok_or_else(invalid)?;
            descriptor.validate(value).map_err(|_| {
                DomainError::new(ErrorCode::OptionUnsupported, "Native option changed")
            })?;
            if descriptor.permission_sensitive {
                return Err(DomainError::new(
                    ErrorCode::ApprovalRequired,
                    "Permission changes require a dedicated host policy",
                ));
            }
            if descriptor.category.as_deref() == Some("model") {
                return Err(invalid());
            }
            if descriptor.current_value.as_ref() == Some(value) {
                continue;
            }
            let mut params = json!({"sessionId":sid,"configId":id,"value":value});
            if matches!(descriptor.control, OptionControl::Boolean) {
                params["type"] = json!("boolean");
            }
            let response = self.wire.call("session/set_config_option", params).await?;
            self.wire.session["configOptions"] = response["configOptions"].clone();
            self.wire.apply_notices()?;
        }
        let current = self.wire.catalog()?;
        if !current
            .models
            .iter()
            .any(|m| m.native_id == self.model.native_id && m.is_default)
        {
            return Err(DomainError::new(
                ErrorCode::SelectionStale,
                "ACP model switch was not accepted",
            ));
        }
        for (id, value) in options {
            if !current
                .config_options
                .iter()
                .any(|d| d.id == *id && d.current_value.as_ref() == Some(value))
            {
                return Err(DomainError::new(
                    ErrorCode::SelectionStale,
                    "ACP configuration was not accepted",
                ));
            }
        }
        Ok(())
    }
    async fn run(
        &mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> Result<()> {
        tokio::select! {
         result=tokio::time::timeout(Duration::from_secs(20),self.configure(&options))=>result.map_err(|_|DomainError::new(ErrorCode::Timeout,"ACP configuration timed out"))??,
         _=cx.cancelled()=>return Err(DomainError::new(ErrorCode::Cancelled,"ACP turn cancelled"))
        }
        let sid = self.wire.session["sessionId"].clone();
        // ACP image content blocks are only sent to peers that advertised promptCapabilities.image.
        if !cx.images().is_empty() && !self.image_input() {
            return Err(DomainError::new(
                ErrorCode::CapabilityUnsupported,
                "Native ACP client does not accept image prompts",
            ));
        }
        let mut blocks = vec![json!({"type":"text","text":prompt})];
        blocks.extend(cx.images().iter().map(|image| {
            json!({"type":"image","mimeType":image.media_type.as_str(),"data":image.base64()})
        }));
        let id = self
            .wire
            .send_request("session/prompt", json!({"sessionId":sid,"prompt":blocks}))
            .await?;
        loop {
            let response = tokio::select! {value=self.wire.read()=>value?,_=cx.cancelled()=>{
             let _=tokio::time::timeout(Duration::from_millis(300),self.wire.peer.send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":sid}}))).await;
             return Err(DomainError::new(ErrorCode::Cancelled,"ACP turn cancelled"));
            }};
            if response.get("method").is_some() {
                if response.get("id").is_some() {
                    if self.host_permission(&response, &sid).await? {
                        continue;
                    }
                    self.wire.permission(&response, &cx, self.review).await?;
                } else if response["method"] == "session/update" {
                    if response["params"]["sessionId"] == sid
                        && self.host_update(&response["params"]["update"], &cx)?
                    {
                        continue;
                    }
                    // Text-only sessions launch clients with their tools removed.
                    // A tool call anyway means the client ignored that contract:
                    // stop the turn instead of silently letting it run.
                    if !self.review
                        && response["params"]["sessionId"] == sid
                        && matches!(
                            response["params"]["update"]["sessionUpdate"].as_str(),
                            Some("tool_call" | "tool_call_update")
                        )
                    {
                        let _ = tokio::time::timeout(
                            Duration::from_millis(300),
                            self.wire.peer.send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":sid}})),
                        )
                        .await;
                        return Err(if self.host.is_empty() {
                            DomainError::new(
                                ErrorCode::IntegrationRestricted,
                                "Native client attempted tool use in a text-only session",
                            )
                        } else {
                            native_tool_refused()
                        });
                    }
                    self.wire.update(&response, &cx, &self.model)?;
                }
                continue;
            }
            if response["id"] != id {
                return Err(invalid());
            }
            let result = response_result(&response)?;
            return match result["stopReason"].as_str() {
                Some("end_turn") => Ok(()),
                Some("max_tokens" | "max_turn_requests") => {
                    cx.emit(EventPayload::Warning {
                        code: "NATIVE_TURN_LIMIT".into(),
                        message: "Native client stopped at its turn limit".into(),
                    })?;
                    Ok(())
                }
                Some("cancelled") => Err(DomainError::new(
                    ErrorCode::Cancelled,
                    "Native turn cancelled",
                )),
                Some("refusal") => Err(DomainError::new(
                    ErrorCode::IntegrationRestricted,
                    "Native client declined the request",
                )),
                _ => Err(invalid()),
            };
        }
    }
}
impl Agent for AcpAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        options: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(self.run(prompt, options, cx))
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.wire.close().await;
            Ok(())
        })
    }
    fn native_resumption(&self) -> bool {
        self.wire
            .catalog()
            .is_ok_and(|catalog| catalog.load_session)
    }
    fn image_input(&self) -> bool {
        self.wire
            .catalog()
            .is_ok_and(|catalog| catalog.image_prompt)
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            let catalog = self.wire.catalog()?;
            if !catalog.load_session {
                return Err(DomainError::new(
                    ErrorCode::CapabilityUnsupported,
                    "Native client does not support session resumption",
                ));
            }
            let native_session_id = catalog.session_id;
            self.wire.peer.shutdown().await;
            Ok(native_session_id)
        })
    }
}

/// Explicit native auth negotiation; no token is sent by the host.
pub async fn inspect_authenticated(
    spec: LaunchSpec,
    provider: &str,
    route: &str,
    auth: Option<&str>,
) -> Result<AcpCatalog> {
    let mut wire = Wire::connect_with_auth(spec, provider.into(), route.into(), auth, &[]).await?;
    let result = wire.catalog();
    wire.close().await;
    result
}
