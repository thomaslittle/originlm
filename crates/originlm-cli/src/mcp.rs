//! `originlm mcp`: the built-in Model Context Protocol server.
//!
//! Newline-delimited JSON-RPC 2.0 over stdio (the MCP stdio transport). It
//! serves read-only tools over the same native service the broker uses, so
//! usage numbers come from the provider adapters' own quota probes (Claude
//! Code `get_usage`, Codex `account/rateLimits/read`), never from estimates.
//! The handshake mirrors the SDK's `createCachedUsageMcp`: `initialize`, then
//! `notifications/initialized`, then `tools/list` / `tools/call`.
use originlm_contracts::{DomainError, ErrorCode, Result};
use originlm_service::NativeService;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub const CLIENT: &str = "mcp-owner";
const PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_LINE_BYTES: usize = 1 << 20;
/// A reading this young that was (re)confirmed by this call is reported as live.
const LIVE_AGE_SECONDS: i64 = 90;

pub struct McpServer {
    service: Arc<NativeService>,
    /// Providers this server may probe (the `--allow-metadata` grants).
    providers: BTreeSet<String>,
    home: Option<PathBuf>,
    identity: bool,
    phase: Phase,
    discovered: bool,
}

#[derive(PartialEq)]
enum Phase {
    New,
    Offered,
    Ready,
}

pub fn tool_definitions() -> Value {
    let read_only = json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":true});
    json!([
        {
            "name": "usage",
            "title": "Provider plan usage",
            "description": "Current plan usage for the connected Claude Code and Codex accounts (and any other granted provider that reports quota): percent used and remaining per window (5-hour, weekly, model-specific, monthly credits), reset times, account and plan. Reads the providers' own usage endpoints through their installed clients; never starts a model turn. Each provider says whether its numbers are live or cached and how old they are. A missing window is unknown, not zero.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "providers": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Provider ids to include, e.g. [\"claude_code\",\"codex\"]. Default: every granted provider."
                    },
                    "refresh": {
                        "type": "boolean",
                        "description": "Bypass OriginLM's 60-second probe cache and re-read the provider now."
                    }
                },
                "additionalProperties": false
            },
            "annotations": read_only,
        },
        {
            "name": "providers",
            "title": "Provider connections",
            "description": "Locally installed AI clients OriginLM found (passive discovery), with sign-in state, account, plan and whether usage has been read. Does not contact providers; call `usage` for quota numbers.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false},
        },
        {
            "name": "usage_history",
            "title": "Local token history",
            "description": "Token totals per model from the supported clients' local session history files (Claude Code, Codex and others) (input, cached input, cache writes, output, reasoning). These are token counts from this machine only, not plan quota; prompts and outputs are never read.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false},
        }
    ])
}

impl McpServer {
    pub fn new(
        service: Arc<NativeService>,
        providers: BTreeSet<String>,
        home: Option<PathBuf>,
        identity: bool,
    ) -> Self {
        Self {
            service,
            providers,
            home,
            identity,
            phase: Phase::New,
            discovered: false,
        }
    }

    /// Handle one decoded JSON-RPC message. Notifications return `None`.
    pub async fn handle(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let valid_id = matches!(&id, Some(Value::String(s)) if s.len() <= 128)
            || matches!(&id, Some(Value::Number(n)) if n.is_i64() || n.is_u64());
        let reply_id = if valid_id {
            id.clone().unwrap()
        } else {
            Value::Null
        };
        let error = |code: i64, text: &str| {
            Some(
                json!({"jsonrpc":"2.0","id":reply_id.clone(),"error":{"code":code,"message":text}}),
            )
        };
        let Some(method) = message["method"].as_str().map(str::to_owned) else {
            return error(-32600, "Invalid MCP request");
        };
        if message["jsonrpc"] != "2.0"
            || message
                .get("params")
                .is_some_and(|p| !p.is_object() && !p.is_null())
        {
            return error(-32600, "Invalid MCP request");
        }
        let params = message.get("params").cloned().unwrap_or(json!({}));
        if id.is_none() {
            if method == "notifications/initialized" && self.phase == Phase::Offered {
                self.phase = Phase::Ready;
            }
            return None;
        }
        if !valid_id {
            return error(-32600, "Invalid MCP request ID");
        }
        let success =
            |result: Value| Some(json!({"jsonrpc":"2.0","id":reply_id.clone(),"result":result}));
        match method.as_str() {
            "ping" => success(json!({})),
            "initialize" => {
                if self.phase != Phase::New {
                    return error(-32600, "Already initialized");
                }
                if !params["protocolVersion"].is_string() {
                    return error(-32602, "Invalid initialization");
                }
                self.phase = Phase::Offered;
                success(json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "originlm", "title": "OriginLM", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Use `usage` to answer how much Claude / Codex plan quota is left and when it resets. `providers` lists connected clients; `usage_history` gives local token totals."
                }))
            }
            _ if self.phase == Phase::New => error(-32002, "Initialize MCP first"),
            "tools/list" => success(json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or_default().to_owned();
                let arguments = match params.get("arguments") {
                    None | Some(Value::Null) => json!({}),
                    Some(value) if value.is_object() => value.clone(),
                    Some(_) => return error(-32602, "Tool arguments must be an object"),
                };
                let outcome = match name.as_str() {
                    "usage" => self.usage(&arguments).await,
                    "providers" => self.providers_tool(&arguments).await,
                    "usage_history" => self.history(&arguments),
                    _ => return error(-32602, "Unknown tool"),
                };
                Some(match outcome {
                    Ok((text, data)) => json!({"jsonrpc":"2.0","id":reply_id,"result":{
                        "content":[{"type":"text","text":text}],
                        "structuredContent":data,
                        "isError":false
                    }}),
                    Err(failure) => json!({"jsonrpc":"2.0","id":reply_id,"result":{
                        "content":[{"type":"text","text":format!("{}: {}", code(&failure), failure.message)}],
                        "isError":true
                    }}),
                })
            }
            _ => error(-32601, "MCP method unavailable"),
        }
    }

    async fn discover(&mut self, force: bool) -> Result<()> {
        if self.discovered && !force {
            return Ok(());
        }
        self.service
            .handle(CLIENT, "discovery.refresh", json!({"level":"passive"}))
            .await?;
        self.discovered = true;
        Ok(())
    }

    async fn snapshot(&self) -> Result<Value> {
        self.service.handle(CLIENT, "snapshot.get", json!({})).await
    }

    async fn usage(&mut self, arguments: &Value) -> Result<(String, Value)> {
        allow(arguments, &["providers", "refresh"])?;
        let refresh = match arguments.get("refresh") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(invalid("`refresh` must be a boolean")),
        };
        let wanted: BTreeSet<String> = match arguments.get("providers") {
            None | Some(Value::Null) => self.providers.clone(),
            Some(Value::Array(items)) => {
                let mut set = BTreeSet::new();
                for item in items {
                    let id = item
                        .as_str()
                        .ok_or_else(|| invalid("`providers` must be strings"))?;
                    if !self.providers.contains(id) {
                        return Err(invalid(&format!(
                            "Provider `{id}` is not granted to this server (granted: {})",
                            self.providers
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ")
                        )));
                    }
                    set.insert(id.to_owned());
                }
                set
            }
            Some(_) => return Err(invalid("`providers` must be an array")),
        };
        let started = chrono::Utc::now();
        self.discover(refresh).await?;
        let listed = self
            .service
            .handle(CLIENT, "connections.list", json!({}))
            .await?;
        let mut by_provider: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for connection in listed.as_array().into_iter().flatten() {
            if let (Some(provider), Some(id)) =
                (connection["providerId"].as_str(), connection["id"].as_str())
                && wanted.contains(provider)
            {
                by_provider
                    .entry(provider.to_owned())
                    .or_default()
                    .push(id.to_owned());
            }
        }
        // Providers are probed concurrently; one provider's profiles in order,
        // because a probe may refresh that provider's own credentials.
        let mut checks = tokio::task::JoinSet::new();
        for ids in by_provider.into_values() {
            let service = self.service.clone();
            checks.spawn(async move {
                let mut results = vec![];
                for id in ids {
                    let outcome = service
                        .handle(
                            CLIENT,
                            "connections.probe",
                            json!({"connectionId": id, "force": refresh}),
                        )
                        .await;
                    results.push((id, outcome));
                }
                results
            });
        }
        let mut probes = std::collections::BTreeMap::new();
        while let Some(joined) = checks.join_next().await {
            for (id, outcome) in joined.unwrap_or_default() {
                probes.insert(
                    id,
                    match outcome {
                        Ok(value) => ProbeOutcome::Ok(value["status"].as_str() == Some("cached")),
                        Err(error) => ProbeOutcome::Failed(code(&error)),
                    },
                );
            }
        }
        let snapshot = self.snapshot().await?;
        let report = usage_report(
            &snapshot,
            &wanted,
            &probes,
            started,
            chrono::Utc::now(),
            self.identity,
        );
        let text = usage_text(&report);
        let mut report = report;
        // Some clients (Claude Code) show only structuredContent when present,
        // so the readable summary travels inside it as well.
        report["summary"] = json!(text);
        Ok((text, report))
    }

    async fn providers_tool(&mut self, arguments: &Value) -> Result<(String, Value)> {
        allow(arguments, &[])?;
        self.discover(false).await?;
        let snapshot = self.snapshot().await?;
        let report = providers_report(&snapshot, &self.providers, self.identity);
        let mut text = String::new();
        for row in report["providers"].as_array().into_iter().flatten() {
            text.push_str(&format!(
                "{} ({}) — {}; auth {}; account {}; plan {}; {} usage window(s){}\n",
                row["providerName"].as_str().unwrap_or("?"),
                row["providerId"].as_str().unwrap_or("?"),
                row["status"].as_str().unwrap_or("?"),
                row["auth"].as_str().unwrap_or("unknown"),
                row["account"].as_str().unwrap_or(if self.identity {
                    "unknown"
                } else {
                    "hidden (start with --allow-identity)"
                }),
                row["plan"].as_str().unwrap_or("unknown"),
                row["usageWindows"],
                if row["usageGranted"] == true {
                    ""
                } else {
                    " (usage not granted)"
                },
            ));
        }
        if text.is_empty() {
            text.push_str("No provider clients were found on this machine.\n");
        }
        Ok((text, report))
    }

    fn history(&self, arguments: &Value) -> Result<(String, Value)> {
        allow(arguments, &[])?;
        let limits = originlm_discovery::history::Limits::default();
        let history = match &self.home {
            Some(home) => originlm_discovery::history::read_usage(home, limits),
            None => {
                let environment = originlm_discovery::ScanEnvironment::from_process()?;
                originlm_discovery::history::read_usage_for_environment(&environment, limits)
            }
        };
        let value = json!(history);
        let mut text = String::from(
            "Local session-history token totals (this machine only; not plan quota):\n",
        );
        for provider in value["providers"].as_array().into_iter().flatten() {
            let t = &provider["totals"];
            text.push_str(&format!(
                "{}: input {} · cached input {} · cache writes {} · output {} (reasoning {}) across {} file(s){}\n",
                provider["providerId"].as_str().unwrap_or("?"),
                t["inputTokens"], t["cachedInputTokens"], t["cacheCreationTokens"],
                t["outputTokens"], t["reasoningTokens"], provider["filesRead"],
                if provider["truncated"] == true { " (truncated)" } else { "" },
            ));
        }
        Ok((text, value))
    }
}

pub enum ProbeOutcome {
    /// `true` when the service answered from its own short-lived probe cache.
    Ok(bool),
    Failed(String),
}

fn allow(arguments: &Value, keys: &[&str]) -> Result<()> {
    match arguments.as_object() {
        Some(map) if map.keys().all(|k| keys.contains(&k.as_str())) => Ok(()),
        _ => Err(invalid("Unexpected tool argument")),
    }
}

fn invalid(message: &str) -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, message)
}

fn provider_name(id: &str) -> String {
    originlm_discovery::definitions()
        .iter()
        .find(|definition| definition.id == id)
        .map(|definition| definition.name.to_string())
        .unwrap_or_else(|| id.to_owned())
}

fn known(observation: &Value) -> Option<&Value> {
    (observation["state"] == "known").then(|| &observation["value"])
}

fn parse_time(value: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    value
        .as_str()
        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(|time| time.with_timezone(&chrono::Utc))
}

fn decimal(value: &Value) -> Option<f64> {
    value.as_str().and_then(|text| text.parse::<f64>().ok())
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn account_label(snapshot: &Value, account_id: &Value, identity: bool) -> Value {
    if !identity {
        return Value::Null;
    }
    snapshot["accounts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|account| account["id"] == *account_id && !account_id.is_null())
        .and_then(|account| known(&account["displayLabel"]).cloned())
        .unwrap_or(Value::Null)
}

/// Shape the projected snapshot into the `usage` result. Pure, for tests.
pub fn usage_report(
    snapshot: &Value,
    wanted: &BTreeSet<String>,
    probes: &std::collections::BTreeMap<String, ProbeOutcome>,
    started: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
    identity: bool,
) -> Value {
    let empty = vec![];
    let pools = snapshot["quotaPools"].as_array().unwrap_or(&empty);
    let mut rows = vec![];
    for connection in snapshot["connections"].as_array().unwrap_or(&empty) {
        let provider = connection["providerId"].as_str().unwrap_or_default();
        if !wanted.contains(provider) {
            continue;
        }
        let connection_id = connection["id"].as_str().unwrap_or_default();
        let mut windows = vec![];
        let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;
        for pool in pools.iter().filter(|pool| {
            pool["connectionIds"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == connection_id))
        }) {
            for window in pool["windows"].as_array().unwrap_or(&empty) {
                let reading = &window["reading"];
                let mut row = json!({
                    "id": window["id"],
                    "label": friendly_label(window["label"].as_str().unwrap_or("window")),
                    "providerLabel": window["label"],
                    "resetsAt": window["resetsAt"],
                    "windowSeconds": window["durationSeconds"],
                });
                if let Some(value) = known(reading) {
                    let observed = parse_time(&reading["observedAt"]);
                    if let Some(observed) = observed {
                        oldest = Some(oldest.map_or(observed, |o| o.min(observed)));
                    }
                    let used = decimal(&value["used"]);
                    let limit = decimal(&value["limit"]);
                    let percent = match (used, limit, value["unit"].as_str()) {
                        (Some(used), Some(limit), _) if limit > 0.0 => Some(used / limit * 100.0),
                        (Some(used), None, Some("percent")) => Some(used),
                        _ => None,
                    };
                    row["usedPercent"] = percent.map(round1).map_or(Value::Null, |p| json!(p));
                    row["remainingPercent"] = percent
                        .map(|p| round1((100.0 - p).max(0.0)))
                        .map_or(Value::Null, |p| json!(p));
                    row["observedAt"] = reading["observedAt"].clone();
                    row["freshness"] = reading["freshness"].clone();
                } else {
                    row["usedPercent"] = Value::Null;
                    row["remainingPercent"] = Value::Null;
                    row["unknownReason"] = reading["reason"].clone();
                }
                row["resetsInSeconds"] = parse_time(&window["resetsAt"])
                    .map(|reset| json!((reset - now).num_seconds().max(0)))
                    .unwrap_or(Value::Null);
                windows.push(row);
            }
        }
        let probe = probes.get(connection_id);
        let age = oldest.map(|observed| (now - observed).num_seconds().max(0));
        let probe_ok = matches!(probe, Some(ProbeOutcome::Ok(_)));
        let state = if windows.is_empty() {
            "unavailable"
        } else if probe_ok
            && age.is_some_and(|age| age <= LIVE_AGE_SECONDS)
            && oldest.is_some_and(|o| o >= started - chrono::Duration::seconds(LIVE_AGE_SECONDS))
        {
            "live"
        } else {
            "cached"
        };
        let mut data = json!({
            "state": state,
            "observedAt": oldest.map(|o| o.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            "ageSeconds": age,
        });
        match probe {
            Some(ProbeOutcome::Failed(code)) => data["refreshError"] = json!(code),
            Some(ProbeOutcome::Ok(true)) => data["servedFromProbeCache"] = json!(true),
            _ => {}
        }
        rows.push(json!({
            "providerId": provider,
            "providerName": provider_name(provider),
            "connectionId": connection_id,
            "connectionName": connection["displayName"],
            "auth": known(&connection["auth"]).cloned().unwrap_or(json!("unknown")),
            "account": account_label(snapshot, &connection["accountId"], identity),
            "identityHidden": !identity,
            "plan": known(&connection["plan"]).map(|plan| plan["label"].clone()).unwrap_or(Value::Null),
            "data": data,
            "windows": windows,
        }));
    }
    // Connections with readings first; signed-out extra profiles last.
    rows.sort_by_key(|row| {
        (
            row["windows"].as_array().is_none_or(Vec::is_empty),
            row["providerId"].as_str().unwrap_or_default().to_owned(),
        )
    });
    let found: BTreeSet<&str> = rows
        .iter()
        .filter_map(|row| row["providerId"].as_str())
        .collect();
    let missing: Vec<&String> = wanted
        .iter()
        .filter(|p| !found.contains(p.as_str()))
        .collect();
    json!({
        "generatedAt": now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "providers": rows,
        "notFound": missing,
        "notes": [
            "Percentages are the provider's own plan-quota readings, not token estimates.",
            "A missing window or null percent is unknown, not zero.",
            "data.state is `live` when this call read the provider within the last 90 s, otherwise `cached` with its age."
        ]
    })
}

/// Claude Code's `limits[]` rows carry a raw `kind` (`session`, `weekly_all`,
/// `weekly_scoped · <model>`). Name the ones whose meaning is established —
/// the session bucket is the same one the adapter's keyed path labels
/// "5-hour session" — and keep any other provider label unchanged.
pub fn friendly_label(label: &str) -> String {
    let (kind, scope) = match label.split_once(" · ") {
        Some((kind, scope)) => (kind, Some(scope)),
        None => (label, None),
    };
    match (kind, scope) {
        ("session", None) => "5-hour session".into(),
        ("weekly_all", None) => "Weekly (all models)".into(),
        ("weekly_scoped", Some(model)) => format!("Weekly ({model})"),
        _ => label.to_owned(),
    }
}

fn duration_text(seconds: i64) -> String {
    let (d, h, m) = (
        seconds / 86_400,
        (seconds % 86_400) / 3_600,
        (seconds % 3_600) / 60,
    );
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h {m}m"),
        _ => format!("{d}d {h}h"),
    }
}

/// The human-readable side of the `usage` result.
pub fn usage_text(report: &Value) -> String {
    let mut text = String::new();
    for row in report["providers"].as_array().into_iter().flatten() {
        let data = &row["data"];
        let mut who = vec![];
        if let Some(account) = row["account"].as_str() {
            who.push(account.to_owned());
        }
        if let Some(plan) = row["plan"].as_str() {
            who.push(format!("plan {plan}"));
        }
        let freshness = match (data["state"].as_str(), data["ageSeconds"].as_i64()) {
            (Some("live"), Some(age)) => format!("live, read {age}s ago"),
            (Some("cached"), Some(age)) => format!("CACHED, {} old", duration_text(age)),
            _ if row["auth"] == "unauthenticated" => "not signed in; no usage".to_owned(),
            _ => "no usage reported".to_owned(),
        };
        text.push_str(&format!(
            "{}{} — {}{}\n",
            row["connectionName"]
                .as_str()
                .or(row["providerName"].as_str())
                .unwrap_or("?"),
            if who.is_empty() {
                String::new()
            } else {
                format!(" ({})", who.join(", "))
            },
            freshness,
            data["refreshError"]
                .as_str()
                .map(|e| format!(" — refresh failed: {e}"))
                .unwrap_or_default(),
        ));
        for window in row["windows"].as_array().into_iter().flatten() {
            let label = window["label"].as_str().unwrap_or("window");
            let reset = match (
                window["resetsAt"].as_str(),
                window["resetsInSeconds"].as_i64(),
            ) {
                (Some(at), Some(secs)) => format!(
                    ", resets {} (in {})",
                    parse_time(&json!(at))
                        .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                        .unwrap_or_else(|| at.to_owned()),
                    duration_text(secs)
                ),
                _ => String::new(),
            };
            match (
                window["usedPercent"].as_f64(),
                window["remainingPercent"].as_f64(),
            ) {
                (Some(used), Some(left)) => {
                    text.push_str(&format!("  {label}: {used}% used, {left}% left{reset}\n"))
                }
                _ => text.push_str(&format!("  {label}: unknown{reset}\n")),
            }
        }
    }
    for missing in report["notFound"].as_array().into_iter().flatten() {
        text.push_str(&format!(
            "{}: not found on this machine.\n",
            provider_name(missing.as_str().unwrap_or("?"))
        ));
    }
    if text.is_empty() {
        text.push_str("No granted provider reported usage.\n");
    }
    text
}

/// Shape the snapshot into the `providers` result. Pure, for tests.
pub fn providers_report(snapshot: &Value, granted: &BTreeSet<String>, identity: bool) -> Value {
    let empty = vec![];
    let pools = snapshot["quotaPools"].as_array().unwrap_or(&empty);
    let rows: Vec<Value> = snapshot["connections"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|connection| {
            let id = &connection["id"];
            let windows: usize = pools
                .iter()
                .filter(|pool| pool["connectionIds"].as_array().is_some_and(|ids| ids.contains(id)))
                .map(|pool| pool["windows"].as_array().map_or(0, Vec::len))
                .sum();
            let provider = connection["providerId"].as_str().unwrap_or_default();
            json!({
                "providerId": provider,
                "providerName": provider_name(provider),
                "connectionId": id,
                "connectionName": connection["displayName"],
                "status": connection["status"],
                "auth": known(&connection["auth"]).cloned().unwrap_or(json!("unchecked")),
                "account": account_label(snapshot, &connection["accountId"], identity),
                "plan": known(&connection["plan"]).map(|plan| plan["label"].clone()).unwrap_or(Value::Null),
                "usageGranted": granted.contains(provider),
                "usageWindows": windows,
            })
        })
        .collect();
    json!({"providers": rows})
}

/// Serve MCP over a byte stream until EOF.
pub async fn serve<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: R,
    mut writer: W,
    mut server: McpServer,
) -> Result<()> {
    let mut lines = BufReader::new(reader);
    let mut line = String::new();
    let io = |_| DomainError::new(ErrorCode::ProviderUnavailable, "MCP stdio closed");
    loop {
        line.clear();
        let read = (&mut lines)
            .take(MAX_LINE_BYTES as u64 + 1)
            .read_line(&mut line)
            .await
            .map_err(io)?;
        if read == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let reply = if line.len() > MAX_LINE_BYTES {
            // Drain the rest of the oversized line before answering.
            let mut rest = Vec::new();
            lines.read_until(b'\n', &mut rest).await.map_err(io)?;
            Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"MCP message too large"}}),
            )
        } else {
            match serde_json::from_str::<Value>(trimmed) {
                Ok(Value::Array(_)) => Some(
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Batches are not supported"}}),
                ),
                Ok(message) => server.handle(message).await,
                Err(_) => Some(
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
                ),
            }
        };
        if let Some(reply) = reply {
            let mut bytes = serde_json::to_vec(&reply).unwrap_or_default();
            bytes.push(b'\n');
            writer.write_all(&bytes).await.map_err(io)?;
            writer.flush().await.map_err(io)?;
        }
    }
}

fn code(error: &DomainError) -> String {
    serde_json::to_value(&error.code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "ERROR".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn time(text: &str) -> chrono::DateTime<chrono::Utc> {
        parse_time(&json!(text)).unwrap()
    }

    fn known_value(value: Value, observed: &str) -> Value {
        json!({"state":"known","value":value,
            "source":{"kind":"official_protocol","adapterId":"x","adapterVersion":"1"},
            "observedAt":observed,"freshness":"fresh","verification":"reported"})
    }

    fn reading(used: &str, observed: &str) -> Value {
        known_value(
            json!({"unit":"percent","used":used,"limit":"100","remaining":null,"usedFraction":null}),
            observed,
        )
    }

    fn connection(
        id: &str,
        provider: &str,
        name: &str,
        account: Value,
        auth: Value,
        plan: Value,
    ) -> Value {
        json!({"id":id,"providerId":provider,"displayName":name,"accountId":account,
            "auth":auth,"plan":plan,"status":"ready"})
    }

    fn snapshot() -> Value {
        let at = "2026-11-11T16:00:00Z";
        let unknown = json!({"state":"unknown","reason":"NOT_REPORTED"});
        json!({
            "accounts":[{"id":"acct-c","providerId":"claude_code",
                "displayLabel":known_value(json!("me@example.com"), at),"identitySource":unknown}],
            "connections":[
                connection("conn-claude","claude_code","Claude Code · Default profile",json!("acct-c"),
                    known_value(json!("authenticated"),at),known_value(json!({"nativeId":"max","label":"Claude Max"}),at)),
                connection("conn-codex","codex","Codex · Default profile",Value::Null,
                    known_value(json!("authenticated"),at),unknown.clone()),
                connection("conn-codex-2","codex","Codex · Additional profile 2",Value::Null,
                    known_value(json!("unauthenticated"),at),unknown.clone()),
                connection("conn-gemini","gemini_cli","Gemini",Value::Null,unknown.clone(),unknown.clone()),
            ],
            "quotaPools":[
                {"id":"pool-c","accountId":"acct-c","connectionIds":["conn-claude"],"routeIds":[],"sharing":"unknown","windows":[
                    {"id":"session-0","label":"session","reading":reading("16",at),
                     "resetsAt":"2026-11-11T20:10:00.080121+00:00","durationSeconds":null},
                    {"id":"weekly_scoped-2","label":"weekly_scoped · Fable","reading":reading("54",at),
                     "resetsAt":"2026-11-14T21:00:00Z","durationSeconds":null},
                    {"id":"spend","label":"Monthly spend","reading":{"state":"unavailable","reason":"USAGE_UNAVAILABLE"},
                     "resetsAt":null,"durationSeconds":null}
                ]},
                {"id":"pool-x","accountId":null,"connectionIds":["conn-codex"],"routeIds":[],"sharing":"unknown","windows":[
                    {"id":"w1","label":"5 hr","reading":reading("61","2026-11-11T15:00:00Z"),
                     "resetsAt":"2026-11-11T18:00:00+00:00","durationSeconds":18000}
                ]}
            ]
        })
    }

    fn wanted() -> BTreeSet<String> {
        ["claude_code", "codex"].map(String::from).into()
    }

    fn row<'a>(report: &'a Value, id: &str) -> &'a Value {
        report["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["connectionId"] == id)
            .unwrap()
    }

    #[test]
    fn live_and_cached_readings_are_labelled_with_their_age() {
        let mut probes = BTreeMap::new();
        probes.insert("conn-claude".to_owned(), ProbeOutcome::Ok(false));
        probes.insert(
            "conn-codex".to_owned(),
            ProbeOutcome::Failed("PROVIDER_UNAVAILABLE".into()),
        );
        let report = usage_report(
            &snapshot(),
            &wanted(),
            &probes,
            time("2026-11-11T16:00:01Z"),
            time("2026-11-11T16:00:30Z"),
            true,
        );
        let rows = report["providers"].as_array().unwrap();
        // Gemini is not granted; the signed-out profile sorts last.
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2]["connectionId"], "conn-codex-2");
        assert_eq!(rows[2]["data"]["state"], "unavailable");

        let claude = row(&report, "conn-claude");
        assert_eq!(claude["data"]["state"], "live");
        assert_eq!(claude["data"]["ageSeconds"], 30);
        assert_eq!(claude["account"], "me@example.com");
        assert_eq!(claude["plan"], "Claude Max");
        let session = &claude["windows"][0];
        assert_eq!(session["label"], "5-hour session");
        assert_eq!(session["providerLabel"], "session");
        assert_eq!(session["usedPercent"], 16.0);
        assert_eq!(session["remainingPercent"], 84.0);
        assert_eq!(session["resetsInSeconds"], 4 * 3600 + 9 * 60 + 30);
        assert_eq!(claude["windows"][1]["label"], "Weekly (Fable)");
        // An unavailable reading is unknown, never zero.
        assert!(claude["windows"][2]["usedPercent"].is_null());
        assert_eq!(claude["windows"][2]["unknownReason"], "USAGE_UNAVAILABLE");

        let codex = row(&report, "conn-codex");
        assert_eq!(codex["data"]["state"], "cached");
        assert_eq!(codex["data"]["ageSeconds"], 3630);
        assert_eq!(codex["data"]["refreshError"], "PROVIDER_UNAVAILABLE");
        assert!(codex["plan"].is_null());

        let text = usage_text(&report);
        for expected in [
            "Claude Code · Default profile (me@example.com, plan Claude Max) — live, read 30s ago",
            "5-hour session: 16% used, 84% left, resets 2026-11-11T20:10:00Z (in 4h 9m)",
            "Codex · Default profile — CACHED, 1h 0m old — refresh failed: PROVIDER_UNAVAILABLE",
            "Monthly spend: unknown",
            "Additional profile 2 — not signed in; no usage",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn a_reading_older_than_this_call_is_cached_even_after_a_successful_probe() {
        let mut probes = BTreeMap::new();
        probes.insert("conn-claude".to_owned(), ProbeOutcome::Ok(true));
        let report = usage_report(
            &snapshot(),
            &wanted(),
            &probes,
            time("2026-11-11T16:05:00Z"),
            time("2026-11-11T16:05:01Z"),
            true,
        );
        let claude = row(&report, "conn-claude");
        assert_eq!(claude["data"]["state"], "cached");
        assert_eq!(claude["data"]["servedFromProbeCache"], true);
    }

    #[test]
    fn identity_is_withheld_without_the_grant_and_missing_providers_are_named() {
        let wanted: BTreeSet<String> = ["claude_code", "cursor"].map(String::from).into();
        let now = time("2026-11-11T16:00:00Z");
        let report = usage_report(&snapshot(), &wanted, &BTreeMap::new(), now, now, false);
        let claude = row(&report, "conn-claude");
        assert!(claude["account"].is_null());
        assert_eq!(claude["identityHidden"], true);
        assert_eq!(report["notFound"], json!(["cursor"]));
        assert!(usage_text(&report).contains("Cursor: not found on this machine."));
    }

    #[test]
    fn providers_report_counts_windows_and_marks_grants() {
        let report = providers_report(&snapshot(), &wanted(), true);
        let rows = report["providers"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0]["usageWindows"], 3);
        assert_eq!(rows[0]["auth"], "authenticated");
        assert_eq!(rows[3]["usageGranted"], false);
        assert_eq!(rows[3]["auth"], "unchecked");
    }

    #[test]
    fn unknown_claude_kinds_keep_the_provider_label() {
        assert_eq!(friendly_label("weekly_all"), "Weekly (all models)");
        assert_eq!(friendly_label("7-day Opus"), "7-day Opus");
        assert_eq!(friendly_label("burst · X"), "burst · X");
    }

    #[test]
    fn tool_definitions_are_read_only_and_closed() {
        let tools = tool_definitions();
        for tool in tools.as_array().unwrap() {
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
            assert_eq!(tool["annotations"]["destructiveHint"], false);
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        }
    }
}
