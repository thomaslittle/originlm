//! Local control center. It consumes the same authenticated JSON-RPC surface as
//! every other application; it never opens the SQLite store or calls a provider.
use originlm_contracts::*;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub const MAX_PAGE_LINES: usize = 500;

/// Default deadline for one broker request when the caller does not supply one.
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);

const CANCEL_POLL: Duration = Duration::from_millis(10);

pub struct BrokerClient {
    runtime: tokio::runtime::Runtime,
    stream: Option<BufReader<originlm_transports::local_ipc::IpcStream>>,
    sequence: u64,
}

impl BrokerClient {
    pub fn connect(address: &str, token: &str) -> std::result::Result<Self, String> {
        Self::connect_with_timeout(address, token, DEFAULT_CALL_TIMEOUT)
    }

    /// Connect and complete authentication/handshake within `timeout`.
    pub fn connect_with_timeout(
        address: &str,
        token: &str,
        timeout: Duration,
    ) -> std::result::Result<Self, String> {
        if address.is_empty() || token.len() != 64 {
            return Err("A broker address and owner token are required".into());
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|_| "Runtime unavailable".to_owned())?;
        let stream = runtime
            .block_on(async {
                tokio::time::timeout(timeout, originlm_transports::local_ipc::connect(address))
                    .await
            })
            .map_err(|_| "Broker connection timed out".to_owned())?
            .map_err(|error| error.to_string())?;
        let mut client = Self {
            runtime,
            stream: Some(BufReader::new(stream)),
            sequence: 0,
        };
        let authentication = client
            .call_typed_with(
                "broker.authenticate",
                json!({"token": token}),
                timeout,
                None,
            )
            .map_err(|error| error.to_string())?;
        if authentication["authenticated"] != true {
            return Err("PERMISSION_DENIED: Broker did not authenticate this client".into());
        }
        let handshake = client
            .call_typed_with(
                "runtime.handshake",
                json!({"protocolVersion":"1.0","clientName":"originlm-control"}),
                timeout,
                None,
            )
            .map_err(|error| error.to_string())?;
        validate_handshake(&handshake)?;
        Ok(client)
    }

    pub fn from_token_file(
        path: &Path,
        expected_address: Option<&str>,
    ) -> std::result::Result<Self, String> {
        let bytes = originlm_platform::read_private_file(path, 16 * 1024).map_err(|_| {
            "Token file must be a readable, bounded owner-only regular file".to_owned()
        })?;
        let descriptor: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Token file is malformed".to_owned())?;
        let address = descriptor["address"]
            .as_str()
            .ok_or_else(|| "Token file has no address".to_owned())?;
        if expected_address.is_some_and(|expected| expected != address) {
            return Err("Token file belongs to a different broker".into());
        }
        let token = descriptor["token"]
            .as_str()
            .ok_or_else(|| "Token file has no token".to_owned())?;
        Self::connect(address, token)
    }

    pub fn call(&mut self, method: &str, params: Value) -> std::result::Result<Value, String> {
        self.call_typed(method, params).map_err(|error| {
            let code = serde_json::to_value(&error.code).ok();
            format!(
                "{}: {}",
                code.as_ref()
                    .and_then(Value::as_str)
                    .unwrap_or("PROVIDER_UNAVAILABLE"),
                error.message
            )
        })
    }

    /// Canonical errors retain code and retry hints without exposing remote error bodies.
    pub fn call_typed(&mut self, method: &str, params: Value) -> originlm_contracts::Result<Value> {
        self.call_typed_with(method, params, DEFAULT_CALL_TIMEOUT, None)
    }

    /// Bounded, optionally cancellable request. Cancellation closes this
    /// transport because a partial reply cannot be reused.
    pub fn call_typed_with(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: Option<&AtomicBool>,
    ) -> originlm_contracts::Result<Value> {
        if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
            self.stream = None;
            return Err(cancelled());
        }
        self.sequence = self.sequence.saturating_add(1);
        let id = self.sequence;
        let frame = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        let stream = self.stream.as_mut().ok_or_else(|| {
            DomainError::new(ErrorCode::ProviderUnavailable, "Control center is closed")
        })?;
        let mut complete_reply = false;
        let result = self.runtime.block_on(async {
            tokio::time::timeout(timeout, async {
                tokio::select! {
                    biased;
                    () = wait_for_cancel(cancel) => Err(cancelled()),
                    result = async {
                        let bytes =
                            serde_json::to_vec(&frame).map_err(|_| DomainError::new(ErrorCode::InvalidRequest, "Request is invalid"))?;
                        if bytes.len() > 1024 * 1024 {
                            return Err(DomainError::new(ErrorCode::FrameTooLarge, "Request exceeds the frame bound"));
                        }
                        stream
                            .get_mut()
                            .write_all(&bytes)
                            .await
                            .map_err(|_| DomainError::new(ErrorCode::ProviderUnavailable, "Broker connection is closed"))?;
                        stream
                            .get_mut()
                            .write_all(b"\n")
                            .await
                            .map_err(|_| DomainError::new(ErrorCode::ProviderUnavailable, "Broker connection is closed"))?;
                        stream
                            .get_mut()
                            .flush()
                            .await
                            .map_err(|_| DomainError::new(ErrorCode::ProviderUnavailable, "Broker connection is closed"))?;
                        loop {
                            let mut line = Vec::new();
                            // Bound allocation before parsing, including a peer that never sends a newline.
                            let read = (&mut *stream)
                                .take(1024 * 1024 + 1)
                                .read_until(b'\n', &mut line)
                                .await
                                .map_err(|_| DomainError::new(ErrorCode::ProviderUnavailable, "Broker connection is closed"))?;
                            if line.len() > 1024 * 1024 {
                                return Err(DomainError::new(ErrorCode::FrameTooLarge, "Broker response exceeds the frame bound"));
                            }
                            if read == 0 {
                                return Err(DomainError::new(ErrorCode::ProviderUnavailable, "Broker connection is closed"));
                            }
                            let value: Value = serde_json::from_slice(&line)
                                .map_err(|_| DomainError::new(ErrorCode::ProtocolIncompatible, "Broker returned invalid JSON"))?;
                            if value.get("id").and_then(Value::as_i64) != Some(id as i64) {
                                continue;
                            }
                            if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                                return Err(DomainError::new(ErrorCode::ProtocolIncompatible, "Broker returned an invalid response envelope"));
                            }
                            if let Some(error) = value.get("error") {
                                let data = &error["data"];
                                let code = serde_json::from_value::<ErrorCode>(data["code"].clone())
                                    .map_err(|_| DomainError::new(ErrorCode::ProtocolIncompatible, "Broker returned an invalid error code"))?;
                                let retryable = data["retryable"] == true;
                                let retry_after_ms = match data.get("retryAfterMs") {
                                    None | Some(Value::Null) => None,
                                    Some(value) => Some(value.as_u64().filter(|value| retryable && *value <= 86_400_000)
                                        .ok_or_else(|| DomainError::new(ErrorCode::ProtocolIncompatible, "Broker returned an invalid retry hint"))?),
                                };
                                complete_reply = true;
                                return Err(DomainError { code, message: "Broker rejected this request; check its error code and the application's grants".into(), retryable, retry_after_ms });
                            }
                            if value.get("result").is_none() {
                                return Err(DomainError::new(ErrorCode::ProtocolIncompatible, "Broker returned an invalid response envelope"));
                            }
                            complete_reply = true;
                            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                        }
                    } => result,
                }
            })
            .await
            .map_err(|_| DomainError::new(ErrorCode::Timeout, "Broker request timed out"))?
        });
        // A partial write/read, timeout or late reply cannot be reused as a new request's stream.
        if result.is_err() && !complete_reply {
            self.stream = None;
        }
        result
    }

    pub fn snapshot(&mut self) -> std::result::Result<Snapshot, String> {
        let value = self.call("snapshot.get", json!({}))?;
        serde_json::from_value(value).map_err(|_| "Broker returned an incompatible snapshot".into())
    }

    pub fn apps(&mut self) -> std::result::Result<Vec<Value>, String> {
        Ok(self
            .call("apps.list", json!({}))?
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    pub fn close(self) {
        self.close_with(DEFAULT_CALL_TIMEOUT, None);
    }

    /// Close within `timeout`; a cancelled or timed-out close still drops the
    /// transport locally.
    pub fn close_with(mut self, timeout: Duration, cancel: Option<&AtomicBool>) {
        let _ = self.call_typed_with("runtime.closeClient", json!({}), timeout, cancel);
        self.stream = None;
    }
}

fn cancelled() -> DomainError {
    DomainError::new(ErrorCode::Cancelled, "Broker request was cancelled")
}

async fn wait_for_cancel(cancel: Option<&AtomicBool>) {
    match cancel {
        Some(cancel) => {
            while !cancel.load(Ordering::Acquire) {
                tokio::time::sleep(CANCEL_POLL).await;
            }
        }
        None => std::future::pending::<()>().await,
    }
}

fn validate_handshake(value: &Value) -> std::result::Result<(), String> {
    let compatible = value["protocolVersion"].as_str().is_some_and(|version| {
        version
            .strip_prefix("1.")
            .is_some_and(|minor| !minor.is_empty() && minor.bytes().all(|b| b.is_ascii_digit()))
    }) && value["methods"]
        .as_array()
        .is_some_and(|methods| methods.iter().all(Value::is_string));
    if compatible {
        Ok(())
    } else {
        Err("PROTOCOL_INCOMPATIBLE: Update the runtime or use a compatible private helper".into())
    }
}

pub fn read_export(path: &Path) -> std::result::Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|_| "Export is unreadable".to_owned())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Export exceeds the bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Export is malformed".to_owned())
}

/// Diagnostic exports are public-contract data only. Secret-shaped keys are
/// removed defensively even though the protocol never returns provider tokens.
pub fn redacted(value: &Value) -> Value {
    fn key_is_secret(key: &str) -> bool {
        let key = key.to_ascii_lowercase();
        [
            "token",
            "secret",
            "authorization",
            "cookie",
            "password",
            "apikey",
            "api_key",
        ]
        .iter()
        .any(|marker| key.contains(marker))
    }
    fn walk(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .filter(|(key, _)| !key_is_secret(key))
                    .map(|(key, value)| (key.clone(), walk(value)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(values.iter().map(walk).collect()),
            _ => value.clone(),
        }
    }
    walk(value)
}

pub fn diagnostic_document(
    snapshot: &Snapshot,
    runtime: &Value,
    apps: &[Value],
    adapters: &[Value],
) -> Value {
    redacted(&json!({
        "protocolVersion": snapshot.protocol_version,
        "exportedAt": Timestamp::now().as_str(),
        "synthetic": snapshot.mode == DataMode::Synthetic,
        "runtime": runtime,
        "snapshot": snapshot,
        "apps": apps,
        "adapters": adapters,
    }))
}

pub fn write_export(path: &Path, document: &Value) -> std::result::Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Export path is empty".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| "Export directory is unavailable".to_owned())?;
    }
    let bytes = serde_json::to_vec_pretty(document).map_err(|_| "Export is invalid".to_owned())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Export exceeds the bound".into());
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|_| "Export could not be written".to_owned())?;
    std::fs::rename(&temporary, path).map_err(|_| "Export could not be published".to_owned())
}

pub fn overview_lines(snapshot: &Snapshot, runtime: &Value, apps: &[Value]) -> Vec<String> {
    let authenticated = snapshot
        .connections
        .iter()
        .filter(|connection| connection.auth.value() == Some(&AuthState::Authenticated))
        .count();
    let mut lines = vec![
        format!("Revision       {}", snapshot.revision),
        format!("Journal cursor {}", snapshot.cursor),
        format!("Observed       {}", snapshot.observed_at.as_str()),
        format!("Mode           {:?}", snapshot.mode),
        format!(
            "Runtime        {} ({})",
            runtime["status"].as_str().unwrap_or("unknown"),
            runtime["runtimeVersion"].as_str().unwrap_or("-")
        ),
        format!(
            "Storage        {}",
            if runtime["storageAvailable"].as_bool().unwrap_or(false) {
                "available"
            } else {
                "degraded"
            }
        ),
        format!(
            "Hydrated       {}",
            if runtime["hydrated"].as_bool().unwrap_or(false) {
                "yes (reverification pending)"
            } else {
                "no"
            }
        ),
        format!(
            "Health         {:?} / {} open",
            snapshot.health.status, snapshot.health.open_issues
        ),
        format!(
            "Inventory      {} providers, {} connections, {} authenticated, {} routes, {} models, {} pools",
            snapshot.providers.len(),
            snapshot.connections.len(),
            authenticated,
            snapshot.routes.len(),
            snapshot.models.len(),
            snapshot.quota_pools.len()
        ),
        format!("Connected apps {}", apps.len()),
        String::new(),
        "Providers".into(),
    ];
    for provider in &snapshot.providers {
        let total = snapshot
            .connections
            .iter()
            .filter(|connection| connection.provider_id == provider.id)
            .count();
        let ready = snapshot
            .connections
            .iter()
            .filter(|connection| {
                connection.provider_id == provider.id
                    && connection.auth.value() == Some(&AuthState::Authenticated)
            })
            .count();
        lines.push(format!(
            "  {:<20} {} connection(s), {} authenticated",
            provider.name, total, ready
        ));
    }
    if snapshot.providers.is_empty() {
        lines.push("  No supported AI installation or explicit route was found.".into());
    }
    lines
}

pub fn account_lines(snapshot: &Snapshot) -> Vec<String> {
    let mut lines = Vec::new();
    for connection in &snapshot.connections {
        let provider = snapshot
            .providers
            .iter()
            .find(|provider| provider.id == connection.provider_id)
            .map(|provider| provider.name.as_str())
            .unwrap_or(&connection.provider_id);
        let auth = connection
            .auth
            .value()
            .map(|state| format!("{state:?}"))
            .unwrap_or_else(|| "unknown".into());
        let freshness = match &connection.auth {
            Observation::Known { freshness, .. } => format!("{freshness:?}"),
            _ => "-".into(),
        };
        let account = snapshot
            .accounts
            .iter()
            .find(|account| Some(&account.id) == connection.account_id.as_ref())
            .and_then(|account| account.display_label.value().cloned())
            .unwrap_or_else(|| "identity hidden/unknown".into());
        let plan = connection
            .plan
            .value()
            .map(|plan| plan.label.clone())
            .unwrap_or_else(|| "plan unknown".into());
        lines.push(format!(
            "{provider} / {} [{} {}] {account} · {plan} · {}{}",
            connection.display_name,
            auth,
            freshness,
            connection.status,
            if connection.diagnostics.is_empty() {
                String::new()
            } else {
                format!(" · {}", connection.diagnostics.join("; "))
            }
        ));
    }
    if lines.is_empty() {
        lines.push("No connections are visible to this application.".into());
    }
    lines
}

pub fn model_lines(snapshot: &Snapshot) -> Vec<String> {
    let mut lines: Vec<String> = snapshot
        .models
        .iter()
        .map(|model| {
            let route = snapshot
                .routes
                .iter()
                .find(|route| route.id == model.route_id);
            let provider = route.map(|route| route.provider_id.as_str()).unwrap_or("-");
            let context = model
                .context_window
                .value()
                .cloned()
                .unwrap_or_else(|| "context unknown".into());
            format!(
                "{} · {provider}/{}({}) · context {context} · {} option(s) · {} capability(ies)",
                model.name,
                model.native_id,
                route
                    .map(|route| route.display_name.as_str())
                    .unwrap_or("-"),
                model.options.len(),
                model.capabilities.len()
            )
        })
        .collect();
    if lines.is_empty() {
        lines.push(
            "No model catalog is available. Run Refresh after granting metadata access.".into(),
        );
    }
    lines.truncate(MAX_PAGE_LINES);
    lines
}

pub fn usage_lines(snapshot: &Snapshot) -> Vec<String> {
    let mut lines = Vec::new();
    for pool in &snapshot.quota_pools {
        lines.push(format!(
            "{} · sharing {} · {} route(s)",
            pool.id,
            pool.sharing,
            pool.route_ids.len()
        ));
        for window in &pool.windows {
            match &window.reading {
                Observation::Known {
                    value,
                    freshness,
                    source,
                    observed_at,
                    ..
                } => {
                    let limit = value
                        .limit
                        .as_ref()
                        .map(|limit| format!("{} {}", limit.as_str(), value.unit))
                        .unwrap_or_else(|| "unknown".into());
                    lines.push(format!(
                        "  {} · used {} {} of {limit} · {:?} · {} · {}",
                        window.label,
                        value.used.as_str(),
                        value.unit,
                        freshness,
                        source.adapter_id,
                        observed_at.as_str()
                    ));
                }
                Observation::Unknown { reason, .. } | Observation::Unavailable { reason, .. } => {
                    lines.push(format!("  {} · unavailable · {reason}", window.label));
                }
            }
            if let Some(reset) = &window.resets_at {
                lines.push(format!("    reset reported at {}", reset.as_str()));
            }
        }
    }
    if lines.is_empty() {
        lines.push("No usage or reset window has been verified.".into());
    }
    lines
}

pub fn issue_lines(snapshot: &Snapshot) -> Vec<String> {
    let mut lines = Vec::new();
    for issue in &snapshot.issues {
        lines.push(format!(
            "{:?} · {:?} · {:?} · {}{}",
            issue.severity,
            issue.kind,
            issue.state,
            issue.title,
            issue
                .connection_id
                .as_ref()
                .map(|id| format!(" ({id})"))
                .unwrap_or_default()
        ));
        lines.push(format!("  {}", issue.summary));
        if let Some(last) = issue.evidence.last() {
            lines.push(format!("  evidence: {} — {}", last.code, last.message));
        }
        for step in &issue.remediation {
            lines.push(format!(
                "  action: {} — {}{}",
                step.label,
                step.description,
                if step.requires_user_action {
                    " [user]"
                } else {
                    ""
                }
            ));
        }
    }
    if lines.is_empty() {
        lines.push("No actionable issues.".into());
    }
    lines
}

pub fn app_lines(apps: &[Value]) -> Vec<String> {
    let mut lines: Vec<String> = apps
        .iter()
        .map(|app| {
            format!(
                "{} · {} · passive={} identity={} admin={}",
                app["app"].as_str().unwrap_or("-"),
                app["id"].as_str().unwrap_or("-"),
                app["allowPassiveDiscovery"].as_bool().unwrap_or(false),
                app["revealIdentity"].as_bool().unwrap_or(false),
                app["admin"].as_bool().unwrap_or(false)
            )
        })
        .collect();
    if lines.is_empty() {
        lines.push("No application is connected.".into());
    }
    lines
}

pub fn adapter_lines(adapters: &[Value]) -> Vec<String> {
    let mut lines: Vec<String> = adapters
        .iter()
        .map(|adapter| {
            format!(
                "{} · {} · metadata={} text={} qualification={}",
                adapter["name"].as_str().unwrap_or("-"),
                adapter["integration"].as_str().unwrap_or("-"),
                adapter["metadata"].as_bool().unwrap_or(false),
                adapter["textSessions"].as_bool().unwrap_or(false),
                adapter["qualification"].as_str().unwrap_or("-")
            )
        })
        .collect();
    if lines.is_empty() {
        lines.push("No adapter descriptors were reported.".into());
    }
    lines
}

pub fn default_export_path() -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    PathBuf::from(format!("originlm-diagnostics-{stamp}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Snapshot {
        let mut snapshot = Snapshot::empty(DataMode::Live);
        snapshot.revision = "42".into();
        snapshot.cursor = "epoch:7".into();
        snapshot.health.status = HealthStatus::Degraded;
        snapshot.providers.push(Provider {
            id: "openai".into(),
            name: "OpenAI".into(),
            kind: "native".into(),
        });
        snapshot.connections.push(Connection {
            id: "personal".into(),
            provider_id: "openai".into(),
            installation_id: "i".into(),
            profile_id: "p".into(),
            display_name: "Personal Codex".into(),
            account_id: None,
            auth: Observation::unknown("UNPROBED"),
            plan: Observation::unknown("NOT_REPORTED"),
            status: "metadata_stale".into(),
            diagnostics: vec![],
        });
        snapshot
    }

    #[test]
    fn overview_shows_revision_health_and_explicit_staleness() {
        let snapshot = fixture();
        let lines = overview_lines(
            &snapshot,
            &json!({"status":"ok","storageAvailable":true}),
            &[],
        );
        let text = lines.join("\n");
        assert!(text.contains("42"));
        assert!(text.contains("Degraded"));
        assert!(text.contains("OpenAI"));
        assert!(account_lines(&snapshot).join("\n").contains("unknown"));
    }

    #[test]
    fn destructuring_export_removes_secret_shaped_keys_at_any_depth() {
        let document = redacted(&json!({
            "safe": "visible",
            "nested": {"bearerToken": "secret", "list": [{"authorization": "x", "value": 1}]}
        }));
        let text = document.to_string();
        assert!(text.contains("visible"));
        assert!(!text.contains("secret"));
        assert!(!text.contains("authorization"));
        assert!(text.contains("\"value\":1"));
    }

    #[test]
    fn issue_and_usage_views_keep_unknown_distinct_from_zero() {
        let mut snapshot = fixture();
        snapshot.quota_pools.push(QuotaPool {
            id: "pool".into(),
            account_id: None,
            connection_ids: vec!["personal".into()],
            route_ids: vec![],
            sharing: "unknown".into(),
            windows: vec![UsageWindow {
                id: "w".into(),
                label: "Weekly".into(),
                reading: Observation::unknown("NO_USAGE_INTERFACE"),
                resets_at: None,
                duration_seconds: None,
            }],
        });
        let text = usage_lines(&snapshot).join("\n");
        assert!(text.contains("unavailable"));
        assert!(!text.contains("used 0"));
    }

    #[test]
    fn issue_view_exposes_actionable_remediation_without_connection_secrets() {
        let mut snapshot = fixture();
        let now = Timestamp::now();
        snapshot.issues.push(Issue {
            id: "issue".into(),
            fingerprint: "auth".into(),
            kind: IssueKind::AuthExpired,
            severity: IssueSeverity::Error,
            state: IssueState::Open,
            title: "Sign-in expired".into(),
            summary: "The account needs a fresh sign-in.".into(),
            provider_id: None,
            connection_id: None,
            account_id: None,
            route_id: None,
            pool_id: None,
            window_id: None,
            first_seen_at: now.clone(),
            last_seen_at: now,
            resolved_at: None,
            snoozed_until: None,
            occurrences: 1,
            evidence: vec![],
            remediation: vec![Remediation {
                kind: RemediationKind::SignIn,
                label: "Sign in".into(),
                description: "Use the provider's own sign-in flow.".into(),
                requires_user_action: true,
            }],
            last_observation: None,
        });
        let text = issue_lines(&snapshot).join("\n");
        assert!(text.contains("Sign-in expired"));
        assert!(text.contains("Use the provider's own sign-in flow."));
        assert!(text.contains("[user]"));
        assert!(!text.contains("token"));
    }

    #[test]
    fn diagnostics_document_carries_provenance_and_no_secrets() {
        let snapshot = fixture();
        let app = json!({"id":"app","token":"must-not-leak","revealIdentity":false});
        let document = diagnostic_document(
            &snapshot,
            &json!({"status":"ok"}),
            &[app],
            &[json!({"id":"adapter","qualification":"fixture-tested"})],
        );
        let text = document.to_string();
        assert!(text.contains("fixture-tested"));
        assert!(text.contains("42"));
        assert!(!text.contains("must-not-leak"));
    }
}
