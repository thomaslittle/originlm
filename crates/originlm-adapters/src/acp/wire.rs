use super::{AcpCatalog, invalid, parse_catalog, text};
use crate::process::{LaunchSpec, Peer};
use originlm_contracts::*;
use serde_json::{Value, json};
use std::time::Duration;
pub(super) struct Wire {
    pub peer: Peer,
    pub init: Value,
    pub session: Value,
    pub notices: Vec<Value>,
    pub provider: String,
    pub route: String,
}
impl Wire {
    pub async fn connect(spec: LaunchSpec, provider: String, route: String) -> Result<Self> {
        Self::connect_with_auth(spec, provider, route, None, &[]).await
    }
    /// ACP `session/new`/`session/load` MCP server list. Host servers are sent
    /// only to peers that advertise `mcpCapabilities.http`; others refuse typed.
    fn mcp_servers(&self, servers: &[HostMcpServer]) -> Result<Value> {
        if servers.is_empty() {
            return Ok(json!([]));
        }
        if self.init["agentCapabilities"]["mcpCapabilities"]["http"].as_bool() != Some(true) {
            return Err(crate::host_mcp::unsupported());
        }
        Ok(acp_mcp_servers(servers))
    }
    async fn initialized(
        spec: LaunchSpec,
        provider: String,
        route: String,
        auth: Option<&str>,
        timeout: Duration,
    ) -> Result<Self> {
        let mut this = Self {
            peer: Peer::spawn(spec).await?,
            init: Value::Null,
            session: Value::Null,
            notices: vec![],
            provider,
            route,
        };
        let outcome = tokio::time::timeout(Duration::from_secs(20), async {
            this.init = this
                .call(
                    "initialize",
                    json!({"protocolVersion":1,"clientInfo":{"name":"originlm","version":env!("CARGO_PKG_VERSION")},"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false,"session":{"configOptions":{"boolean":{}},"notices":{}}}}),
                )
                .await?;
            if this.init["protocolVersion"].as_u64() != Some(1) {
                return Err(invalid());
            }
            if let Some(method) = auth {
                let offered = this.init["authMethods"].as_array().filter(|a| a.len() <= 64);
                if method.len() > 128
                    || method.chars().any(char::is_control)
                    || !offered.is_some_and(|a| a.iter().any(|v| v["id"] == method))
                {
                    return Err(DomainError::new(
                        ErrorCode::OptionUnsupported,
                        "Native authentication method is not advertised",
                    ));
                }
                this.call("authenticate", json!({"methodId":method})).await?;
            }
            Ok(())
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorCode::Timeout,
                "ACP initialization timed out",
            ))
        });
        if let Err(error) = outcome {
            this.peer.shutdown().await;
            return Err(error);
        }
        Ok(this)
    }
    /// Interactive sign-in: initialize and authenticate, but do not create a
    /// session. The caller keeps the peer alive until the browser callback.
    pub async fn connect_auth(
        spec: LaunchSpec,
        provider: String,
        route: String,
        method: &str,
        host_mcp_servers: &[HostMcpServer],
    ) -> Result<Self> {
        let _ = host_mcp_servers;
        Self::initialized(spec, provider, route, Some(method), Duration::from_secs(300)).await
    }

    pub async fn connect_with_auth(
        spec: LaunchSpec,
        provider: String,
        route: String,
        auth: Option<&str>,
        host_mcp_servers: &[HostMcpServer],
    ) -> Result<Self> {
        let cwd = spec.cwd.clone();
        let mut this =
            Self::initialized(spec, provider, route, auth, Duration::from_secs(20)).await?;
        let outcome = tokio::time::timeout(Duration::from_secs(20), async {
            let mcp_servers = this.mcp_servers(host_mcp_servers)?;
            this.session = this
                .call("session/new", json!({"cwd":cwd,"mcpServers":mcp_servers}))
                .await?;
            this.catalog()?;
            this.apply_notices()?;
            Ok(())
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorCode::Timeout,
                "ACP session creation timed out",
            ))
        });
        if let Err(error) = outcome {
            this.peer.shutdown().await;
            return Err(error);
        }
        Ok(this)
    }
    pub async fn connect_resume_with_auth(
        spec: LaunchSpec,
        provider: String,
        route: String,
        auth: Option<&str>,
        native_session_id: &str,
        host_mcp_servers: &[HostMcpServer],
    ) -> Result<Self> {
        if native_session_id.is_empty()
            || native_session_id.len() > 512
            || native_session_id.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let cwd = spec.cwd.clone();
        let mut this =
            Self::initialized(spec, provider, route, auth, Duration::from_secs(20)).await?;
        if this.init["agentCapabilities"]["loadSession"].as_bool() != Some(true) {
            this.peer.shutdown().await;
            return Err(DomainError::new(
                ErrorCode::CapabilityUnsupported,
                "Native client does not support session resumption",
            ));
        }
        let outcome = tokio::time::timeout(Duration::from_secs(20), async {
            let mcp_servers = this.mcp_servers(host_mcp_servers)?;
            let mut session = this
                .call(
                    "session/load",
                    json!({"sessionId":native_session_id,"cwd":cwd,"mcpServers":mcp_servers}),
                )
                .await?;
            if let Some(id) = session.get("sessionId") {
                if id.as_str() != Some(native_session_id) {
                    return Err(DomainError::new(
                        ErrorCode::SessionRouteMismatch,
                        "Native client loaded a different session",
                    ));
                }
            } else {
                session["sessionId"] = json!(native_session_id);
            }
            this.session = session;
            this.catalog()?;
            this.apply_notices()?;
            Ok(())
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorCode::Timeout,
                "ACP session resume timed out",
            ))
        });
        if let Err(error) = outcome {
            this.peer.shutdown().await;
            return Err(error);
        }
        Ok(this)
    }
    pub fn catalog(&self) -> Result<AcpCatalog> {
        parse_catalog(&self.provider, &self.route, &self.init, &self.session)
    }
    pub async fn send_request(&mut self, method: &str, params: Value) -> Result<u64> {
        self.peer.sequence += 1;
        let id = self.peer.sequence;
        self.peer
            .send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        Ok(id)
    }
    pub async fn read(&mut self) -> Result<Value> {
        let v = self.peer.read().await?;
        if v["jsonrpc"] != "2.0" || !v.is_object() {
            return Err(invalid());
        }
        Ok(v)
    }
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.send_request(method, params).await?;
        for _ in 0..512 {
            let response = self.read().await?;
            if response.get("method").is_some() {
                if response.get("id").is_some() {
                    self.deny_request(&response).await?;
                    return Err(DomainError::new(
                        ErrorCode::IntegrationRestricted,
                        "ACP requested an unadvertised setup operation",
                    ));
                }
                if response["method"] == "session/update" {
                    if self.notices.len() >= 128 {
                        return Err(DomainError::new(
                            ErrorCode::FrameTooLarge,
                            "Too many ACP updates",
                        ));
                    }
                    self.notices.push(response);
                }
                continue;
            }
            if response["id"] != id {
                return Err(invalid());
            }
            if method == "authenticate"
                && response.get("result") == Some(&Value::Null)
                && response.get("error").is_none()
            {
                return Ok(json!({}));
            }
            return response_result(&response);
        }
        Err(invalid())
    }
    pub fn apply_notices(&mut self) -> Result<()> {
        let session = text(&self.session["sessionId"])?;
        for notice in std::mem::take(&mut self.notices) {
            if notice["params"]["sessionId"] != session {
                return Err(DomainError::new(
                    ErrorCode::SessionRouteMismatch,
                    "ACP session changed",
                ));
            }
            let update = &notice["params"]["update"];
            if update["sessionUpdate"] == "config_option_update" {
                self.session["configOptions"] = update["configOptions"].clone();
            }
        }
        self.catalog()?;
        Ok(())
    }
    pub async fn deny_request(&mut self, request: &Value) -> Result<()> {
        let id = request
            .get("id")
            .filter(|id| id.is_string() || id.is_u64())
            .ok_or_else(invalid)?
            .clone();
        if request["method"] == "session/request_permission" {
            self.peer
                .send(json!({"jsonrpc":"2.0","id":id,"result":{"outcome":{"outcome":"cancelled"}}}))
                .await
        } else {
            self.peer.send(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Client capability not available"}})).await
        }
    }
    pub async fn close(&mut self) {
        if self.init["agentCapabilities"]["sessionCapabilities"]
            .get("close")
            .is_some()
        {
            let id = self.session["sessionId"].clone();
            let _ = tokio::time::timeout(
                Duration::from_millis(500),
                self.call("session/close", json!({"sessionId":id})),
            )
            .await;
        }
        self.peer.shutdown().await;
    }
}
/// ACP `McpServer` entries for streamable-HTTP host servers.
pub fn acp_mcp_servers(servers: &[HostMcpServer]) -> Value {
    Value::Array(
        servers
            .iter()
            .map(|server| {
                json!({
                    "type": "http",
                    "name": server.name,
                    "url": server.url,
                    "headers": [{"name": "Authorization", "value": format!("Bearer {}", server.bearer_token)}],
                })
            })
            .collect(),
    )
}
pub(super) fn response_result(value: &Value) -> Result<Value> {
    match (value.get("result"), value.get("error")) {
        (Some(result), None) if result.is_object() => Ok(result.clone()),
        // Authentication failures often carry login URLs or device codes that
        // generic redaction cannot recognize; never forward their text.
        (None, Some(error)) if error["code"].as_i64() == Some(-32000) => Err(DomainError::new(
            ErrorCode::AuthRequired,
            "ACP provider requires sign-in in the installed client",
        )),
        (None, Some(error)) => {
            let detail = error["message"]
                .as_str()
                .filter(|message| !message.trim().is_empty())
                .map(originlm_contracts::redact_diagnostic_detail)
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| "ACP request was rejected".into());
            Err(DomainError::new(
                if error["code"].as_i64() == Some(-32601) {
                    ErrorCode::CapabilityUnsupported
                } else {
                    ErrorCode::ProviderUnavailable
                },
                format!("ACP provider: {detail}"),
            ))
        }
        _ => Err(invalid()),
    }
}
