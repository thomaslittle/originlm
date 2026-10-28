//! MSP v1 transport: newline-delimited JSON-RPC 2.0 over the owned process.
use super::{invalid, launch::uuid_v7};
use crate::process::{LaunchSpec, Peer};
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::VecDeque;

/// Inbound frames buffered while a request waits for its response.
const MAX_QUEUED: usize = 8192;
/// `turn/start` frame cap: the kit's 512 KiB prompt plus four 4 MiB images
/// as base64.
const TURN_FRAME: usize = 24 * 1024 * 1024;

/// A JSON-RPC error object returned by Muse.
#[derive(Debug, Clone)]
pub(crate) struct RpcError {
    pub code: i64,
    /// MSP `data.kind` (for example `sessionNotFound`), when present.
    pub kind: String,
    pub message: String,
}
impl RpcError {
    fn from_value(error: &Value) -> Self {
        Self {
            code: error["code"].as_i64().unwrap_or(0),
            kind: error["data"]["kind"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(64)
                .collect(),
            message: error["message"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(4096)
                .collect(),
        }
    }
    /// A typed, redacted kit error. Muse's text is kept (redacted) because it
    /// carries the actionable reason, for example an inaccessible model.
    pub fn domain(&self, method: &str) -> DomainError {
        let kind = match (self.kind.as_str(), self.code) {
            ("", -32601) => "methodNotFound",
            ("", -32001) => "overloaded",
            (kind, _) => kind,
        };
        let code = match kind {
            "capabilityRequired" | "methodNotFound" | "experimentalRequired" => {
                ErrorCode::CapabilityUnsupported
            }
            "overloaded" | "backpressured" => ErrorCode::RateLimited,
            "sessionNotFound" | "notFound" => ErrorCode::NotFound,
            "inputTooLarge" => ErrorCode::FrameTooLarge,
            "cancelled" | "interrupted" => ErrorCode::Cancelled,
            _ => ErrorCode::ProviderUnavailable,
        };
        let detail = redact_diagnostic_detail(&self.message);
        let retryable = code == ErrorCode::RateLimited;
        let mut error = DomainError::new(
            code,
            if detail.is_empty() {
                format!("Muse rejected {method}")
            } else {
                format!("Muse {method}: {detail}")
            },
        );
        error.retryable = retryable;
        error
    }
}

pub(crate) struct Wire {
    pub peer: Peer,
    next_id: u64,
    /// The `initialize` result.
    pub init: Value,
    queue: VecDeque<Value>,
    pending: Vec<u8>,
}

impl Wire {
    /// Spawn `muse serve` and complete the MSP handshake. `experimental`
    /// opts into Muse's experimental surface (used only by metadata probes for
    /// `account/read`); `session_mcp` requests the `sessionMcp` capability.
    pub async fn connect(spec: LaunchSpec, experimental: bool, session_mcp: bool) -> Result<Self> {
        let mut wire = Self {
            peer: Peer::spawn(spec).await?,
            next_id: 0,
            init: Value::Null,
            queue: VecDeque::new(),
            pending: Vec::new(),
        };
        let requested: Vec<&str> = if session_mcp {
            vec!["sessionMcp"]
        } else {
            vec![]
        };
        let init = wire
            .call(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "originlm",
                        "title": "OriginLM",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "capabilities": {
                        "experimentalApi": experimental,
                        "requestedCapabilities": requested,
                    },
                }),
            )
            .await;
        let init = match init {
            Ok(init) => init,
            Err(error) => {
                wire.peer.shutdown().await;
                return Err(error);
            }
        };
        if init["schema"]["version"].as_u64() != Some(1) {
            wire.peer.shutdown().await;
            return Err(DomainError::new(
                ErrorCode::ProtocolIncompatible,
                "Muse returned an unsupported MSP envelope version",
            ));
        }
        wire.init = init;
        wire.send(json!({"jsonrpc":"2.0","method":"initialized"}))
            .await?;
        Ok(wire)
    }
    pub fn granted(&self, capability: &str) -> bool {
        self.init["grantedCapabilities"]
            .as_array()
            .is_some_and(|granted| granted.iter().any(|value| value == capability))
    }
    pub async fn send(&mut self, value: Value) -> Result<()> {
        self.peer.send(value).await
    }
    /// One raw inbound frame. Cancel-safe: a partially read line is kept and
    /// completed by the next call. The caller must [`Self::acknowledge`] it.
    pub async fn read_frame(&mut self) -> Result<Value> {
        let frame = self.peer.read_buffered(&mut self.pending).await?;
        if frame["jsonrpc"] != "2.0" || !frame.is_object() {
            return Err(invalid());
        }
        Ok(frame)
    }
    /// A queued frame (already acknowledged), if any.
    pub fn pop_queued(&mut self) -> Option<Value> {
        self.queue.pop_front()
    }
    /// One inbound frame, acknowledged.
    async fn receive(&mut self) -> Result<Value> {
        let frame = self.read_frame().await?;
        self.acknowledge(&frame).await?;
        Ok(frame)
    }
    /// Server requests are answered at once with the MSP presentation receipt
    /// (`{}`) and then handled like notifications; decisions travel
    /// separately as commands. Unknown server requests are refused.
    pub async fn acknowledge(&mut self, frame: &Value) -> Result<()> {
        if let (Some(method), Some(id)) = (frame.get("method"), frame.get("id")) {
            let id = id.clone();
            if !(id.is_string() || id.is_u64()) {
                return Err(invalid());
            }
            match method.as_str() {
                Some("approval/request" | "userInput/request") => {
                    self.send(json!({"jsonrpc":"2.0","id":id,"result":{}}))
                        .await?;
                }
                _ => {
                    self.send(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not supported by this client"}}))
                        .await?;
                    return Err(DomainError::new(
                        ErrorCode::IntegrationRestricted,
                        "Muse sent an unsupported server request",
                    ));
                }
            }
        }
        Ok(())
    }
    /// Send a request and wait for its response; frames that arrive first are
    /// queued for [`Self::next`].
    pub async fn call_raw(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<std::result::Result<Value, RpcError>> {
        self.next_id += 1;
        let id = self.next_id;
        let mut request = json!({"jsonrpc":"2.0","id":id,"method":method});
        if !params.is_null() {
            request["params"] = params;
        }
        // Only a turn carries inline images (kit-bounded to 4 x 4 MiB).
        let cap = if method == "turn/start" {
            TURN_FRAME
        } else {
            1_048_576
        };
        self.peer.send_bounded(request, cap).await?;
        loop {
            let frame = self.receive().await?;
            if frame.get("method").is_some() {
                if self.queue.len() >= MAX_QUEUED {
                    return Err(DomainError::new(
                        ErrorCode::FrameTooLarge,
                        "Too many Muse notifications while waiting for a response",
                    ));
                }
                self.queue.push_back(frame);
                continue;
            }
            if frame["id"] != id {
                return Err(invalid());
            }
            return match (frame.get("result"), frame.get("error")) {
                (Some(result), None) => Ok(Ok(result.clone())),
                (None, Some(error)) => Ok(Err(RpcError::from_value(error))),
                _ => Err(invalid()),
            };
        }
    }
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.call_raw(method, params)
            .await?
            .map_err(|error| error.domain(method))
    }
    /// An MSP command: `params` plus a fresh UUIDv7 `commandId`. Returns the
    /// command id with the result.
    pub async fn command(&mut self, method: &str, mut params: Value) -> Result<(String, Value)> {
        let command = uuid_v7()?;
        params["commandId"] = Value::String(command.clone());
        let result = self.call(method, params).await?;
        Ok((command, result))
    }
    pub async fn command_raw(
        &mut self,
        method: &str,
        mut params: Value,
    ) -> Result<std::result::Result<Value, RpcError>> {
        params["commandId"] = Value::String(uuid_v7()?);
        self.call_raw(method, params).await
    }
    pub async fn shutdown(&mut self) {
        self.peer.shutdown().await;
    }
}
