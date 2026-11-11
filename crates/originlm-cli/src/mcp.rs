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
/// Upper bound for one usage read across every provider.
const PROBE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

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
            "description": "Every AI provider and profile OriginLM detects on this machine, with plan usage wherever a usage source exists (Claude Code, Codex, Cursor, OpenCode, Muse, Grok, Antigravity, Gemini CLI, GitHub Copilot, Pi): percent used and remaining per window (5-hour, weekly, model-specific, monthly credits), reset times, account and plan. Reads the providers' own usage endpoints through their installed clients; never starts a model turn. Each provider says whether its numbers are live or cached and how old they are. A detected provider without a usage source is still listed and marked so. A missing window is unknown, not zero.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "providers": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Provider ids to include, e.g. [\"claude_code\",\"codex\"]. Default: every detected provider."
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
        let filter = provider_filter(arguments.get("providers"))?;
        let report = collect_usage(
            &self.service,
            CLIENT,
            &self.providers,
            filter.as_ref(),
            refresh,
            !self.discovered || refresh,
            self.identity,
        )
        .await?;
        self.discovered = true;
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
        let text = history_text(&value);
        Ok((text, value))
    }
}

/// Readable local token totals (`usage_history` tool, `usage --history`).
pub fn history_text(value: &Value) -> String {
    let mut text =
        String::from("Local session-history token totals (this machine only; not plan quota):\n");
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
    text
}

/// Discover (when asked), probe every granted connection of `wanted`, and
/// shape the snapshot into the usage report. Shared by the MCP `usage` tool
/// and the `originlm usage` / `olm usage` command so both show the same numbers.
pub async fn collect_usage(
    service: &Arc<NativeService>,
    client: &str,
    granted: &BTreeSet<String>,
    filter: Option<&BTreeSet<String>>,
    refresh: bool,
    discover: bool,
    identity: bool,
) -> Result<Value> {
    let started = chrono::Utc::now();
    if discover {
        service
            .handle(client, "discovery.refresh", json!({"level":"passive"}))
            .await?;
    }
    let listed = service
        .handle(client, "connections.list", json!({}))
        .await?;
    // (provider, connection id, is the provider's default profile)
    let mut targets: Vec<(String, String, bool)> = vec![];
    for connection in listed.as_array().into_iter().flatten() {
        if let (Some(provider), Some(id)) =
            (connection["providerId"].as_str(), connection["id"].as_str())
            && granted.contains(provider)
            && filter.is_none_or(|filter| filter.contains(provider))
        {
            let default = connection["displayName"]
                .as_str()
                .is_some_and(|name| name.contains("Default profile"));
            targets.push((provider.to_owned(), id.to_owned(), default));
        }
    }
    // Each probe starts that provider's own client, and starting them all at
    // once starves the slow ones (Claude Code allows 10 s for its usage
    // answer) into timeouts. So: Claude Code and Codex default profiles
    // first, alone; then every other provider's first profile, three at a
    // time; then additional profiles. One provider's profiles never run at
    // the same time, because a probe may refresh that provider's credentials.
    targets.sort_by_key(|(provider, _, default)| (!*default, provider.clone()));
    let mut seen = BTreeSet::new();
    let mut waves: [Vec<(String, String)>; 3] = Default::default();
    for (provider, id, _) in targets {
        let wave = if !seen.insert(provider.clone()) {
            2
        } else if matches!(provider.as_str(), "claude_code" | "codex") {
            0
        } else {
            1
        };
        waves[wave].push((provider, id));
    }
    // One bound for the whole read, so a client that hangs costs its own row,
    // not the answer: what has been read by then is reported, the rest is a
    // timeout with any earlier reading shown as cached.
    let deadline = tokio::time::Instant::now() + PROBE_DEADLINE;
    let mut probes = std::collections::BTreeMap::new();
    for wave in waves {
        let mut by_provider: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (provider, id) in wave {
            by_provider.entry(provider).or_default().push(id);
        }
        let slots = Arc::new(tokio::sync::Semaphore::new(3));
        let mut checks = tokio::task::JoinSet::new();
        for ids in by_provider.into_values() {
            let service = service.clone();
            let client = client.to_owned();
            let slots = slots.clone();
            checks.spawn(async move {
                let _slot = tokio::time::timeout_at(deadline, slots.acquire_owned()).await;
                let mut results = vec![];
                for id in ids {
                    let outcome = tokio::time::timeout_at(
                        deadline,
                        service.handle(
                            &client,
                            "connections.probe",
                            json!({"connectionId": id, "force": refresh}),
                        ),
                    )
                    .await
                    .unwrap_or_else(|_| {
                        Err(DomainError::new(ErrorCode::Timeout, "Usage read timed out"))
                    });
                    results.push((id, outcome));
                }
                results
            });
        }
        while let Some(joined) = checks.join_next().await {
            for (id, outcome) in joined.unwrap_or_default() {
                probes.insert(
                    id,
                    match outcome {
                        Ok(value) => ProbeOutcome::Ok(value["status"].as_str() == Some("cached")),
                        Err(error) => {
                            ProbeOutcome::Failed(code(&error), detail_codes(&error.message))
                        }
                    },
                );
            }
        }
    }
    let snapshot = service.handle(client, "snapshot.get", json!({})).await?;
    Ok(usage_report(
        &snapshot,
        granted,
        filter,
        &probes,
        started,
        chrono::Utc::now(),
        identity,
    ))
}

/// Optional `providers` narrowing: ids OriginLM discovery knows, else an error.
pub fn provider_filter(value: Option<&Value>) -> Result<Option<BTreeSet<String>>> {
    let items = match value {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(invalid("`providers` must be an array")),
    };
    let mut set = BTreeSet::new();
    for item in items {
        let id = item
            .as_str()
            .ok_or_else(|| invalid("`providers` must be strings"))?;
        if !originlm_discovery::definitions()
            .iter()
            .any(|definition| definition.id == id)
        {
            return Err(invalid(&format!("Unknown provider `{id}`")));
        }
        set.insert(id.to_owned());
    }
    Ok(Some(set))
}

pub enum ProbeOutcome {
    /// `true` when the service answered from its own short-lived probe cache.
    Ok(bool),
    /// Error code, plus the adapter's own diagnostic codes when the error
    /// carried them (`message [CODE,...]`).
    Failed(String, Vec<String>),
}

/// Diagnostic codes an adapter appended to an error message as `[A,B]`.
pub fn detail_codes(message: &str) -> Vec<String> {
    let Some(inner) = message
        .trim_end()
        .strip_suffix(']')
        .and_then(|rest| rest.rsplit_once('[').map(|(_, inner)| inner))
    else {
        return vec![];
    };
    inner
        .split(',')
        .map(str::trim)
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 96
                && code
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '.')
        })
        .map(str::to_owned)
        .take(16)
        .collect()
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

/// Providers whose adapters can report plan quota, and the grant that turns
/// the read on. Claude Code and Codex are granted by default; the others are
/// listed with the flags that enable them. Every other detected provider has
/// no usage source in OriginLM today and is listed as such.
pub const USAGE_SOURCES: [&str; 10] = crate::args::DEFAULT_USAGE_PROVIDERS;

fn usage_source(provider: &str) -> Option<&'static str> {
    USAGE_SOURCES.iter().find(|id| **id == provider).copied()
}

/// Why a detected provider has no usage source in OriginLM, checked per
/// provider (see docs/integration/mcp-server.md).
fn no_source_reason(provider: &str) -> &'static str {
    match provider {
        "ollama" => "Ollama's local API reports sign-in and plan, not cloud usage limits",
        "lmstudio" => "local runtime; LM Studio has no plan quota",
        "kiro" => "Kiro CLI does not report usage",
        _ => "OriginLM has no usage source for it",
    }
}

/// Plain reason for a probed connection that returned no reading, from the
/// adapter's own diagnostics. Unknown codes fall back to a neutral line.
fn reading_missing_reason(row: &Value) -> Option<String> {
    let codes: Vec<&str> = row["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let has = |code: &str| codes.iter().any(|c| c.starts_with(code));
    let reason = if has("USAGE_NOT_YET_OBSERVED") {
        "Muse reports usage only after this login has run a turn".to_owned()
    } else if has("METADATA_RUNTIME_MISSING") {
        "this installation's runtime is missing".to_owned()
    } else if has("GEMINI_API_KEY_AUTH_NO_QUOTA") {
        "Gemini CLI signs in with a Gemini API key here; that is metered and Google publishes no usage endpoint for a key".to_owned()
    } else if has("GEMINI_VERTEX_AUTH_NO_QUOTA") {
        "Gemini CLI uses Vertex AI here; usage is billed in Google Cloud, not as a plan quota"
            .to_owned()
    } else if has("GEMINI_LOGIN_EXPIRED") {
        "Gemini CLI's saved login has expired; run gemini once to refresh it".to_owned()
    } else if has("GEMINI_LOGIN_NOT_FOUND") {
        "no saved Gemini CLI login".to_owned()
    } else if has("METADATA_READ_TIMEOUT") || has("USAGE_READ_FAILED_Timeout") {
        "the client did not answer in time; try olm usage --refresh".to_owned()
    } else if has("COPILOT_GH_LOGIN_UNAVAILABLE") {
        "sign in with `gh auth login` to read Copilot usage".to_owned()
    } else if has("COPILOT_USAGE_REQUEST_FAILED") {
        "GitHub did not return Copilot usage for the gh login".to_owned()
    } else if has("PI_NO_SAVED_LOGINS") {
        "Pi has no saved logins".to_owned()
    } else if let Some(name) = codes.iter().find_map(|c| {
        c.strip_prefix("PI_")
            .and_then(|c| c.strip_suffix("_NO_USAGE_SOURCE"))
    }) {
        format!(
            "Pi login `{}` has no usage source",
            name.to_ascii_lowercase()
        )
    } else if has("ANTIGRAVITY_LOGIN_REFRESH_FAILED") {
        "Antigravity's saved login could not be refreshed".to_owned()
    } else if has("ANTIGRAVITY_CREDENTIAL") {
        "no saved Antigravity login for this profile".to_owned()
    } else if has("GROK_BILLING_UNAVAILABLE") {
        "Grok did not answer its billing request".to_owned()
    } else if has("CURSOR_SESSION_TOKEN_UNAVAILABLE") || has("CURSOR_CLI_NOT_SIGNED_IN") {
        "Cursor is not signed in".to_owned()
    } else if row["auth"] == "unauthenticated" {
        "not signed in".to_owned()
    } else if row["auth"] != "authenticated" && has("PLAN_NOT_REPORTED_BY_PROVIDER") {
        "the client reported no sign-in or plan for this profile".to_owned()
    } else {
        return None;
    };
    Some(reason)
}

/// Shape the projected snapshot into the `usage` result. Pure, for tests.
///
/// Every detected connection is listed (narrowed only by `filter`); only the
/// `granted` providers were probed. A connection without a reading is still
/// listed, with the reason: not granted, or no usage source at all.
pub fn usage_report(
    snapshot: &Value,
    granted: &BTreeSet<String>,
    filter: Option<&BTreeSet<String>>,
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
        if filter.is_some_and(|filter| !filter.contains(provider)) {
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
                    row["unit"] = value["unit"].clone();
                    row["used"] = value["used"].clone();
                    row["limit"] = value["limit"].clone();
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
        let source = if granted.contains(provider) {
            "granted"
        } else if usage_source(provider).is_some() {
            "not_granted"
        } else {
            "none"
        };
        let state = if windows.is_empty() {
            match source {
                "granted" => "unavailable",
                "not_granted" => "not_read",
                _ => "no_source",
            }
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
            Some(ProbeOutcome::Failed(code, _)) => data["refreshError"] = json!(code),
            Some(ProbeOutcome::Ok(true)) => data["servedFromProbeCache"] = json!(true),
            _ => {}
        }
        // The connection's own diagnostics, plus any the failed read carried.
        let mut diagnostics: Vec<Value> = connection["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if let Some(ProbeOutcome::Failed(_, codes)) = probe {
            diagnostics.extend(codes.iter().map(|code| json!(code)));
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
            "usageSource": source,
            "diagnostics": diagnostics,
            "noSourceReason": if source == "none" { json!(no_source_reason(provider)) } else { Value::Null },
            "data": data,
            "windows": windows,
        }));
    }
    // Readings first, then granted connections without one, then providers
    // whose usage was not read, then providers with no usage source.
    rows.sort_by_key(|row| {
        let rank = if row["windows"].as_array().is_some_and(|w| !w.is_empty()) {
            0
        } else {
            match row["usageSource"].as_str() {
                Some("granted") => 1,
                Some("not_granted") => 2,
                _ => 3,
            }
        };
        // Claude Code and Codex lead their group; the rest are alphabetical.
        let lead = match row["providerId"].as_str() {
            Some("claude_code") => 0,
            Some("codex") => 1,
            _ => 2,
        };
        (
            rank,
            lead,
            row["providerName"].as_str().unwrap_or_default().to_owned(),
            row["connectionName"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
    });
    let found: BTreeSet<&str> = rows
        .iter()
        .filter_map(|row| row["providerId"].as_str())
        .collect();
    let missing: Vec<&String> = filter
        .unwrap_or(granted)
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
            "Every detected provider is listed. usageSource `none` means OriginLM has no usage source for it; `not_granted` means one exists but this run excluded it (--no-usage or an explicit --allow-metadata list).",
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
            _ => status_without_reading(row),
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
                _ => text.push_str(&format!(
                    "  {label}: {}{reset}\n",
                    amount_text(window).unwrap_or_else(|| "unknown".into())
                )),
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
        text.push_str("No AI provider was detected on this machine.\n");
    }
    text
}

/// A known non-percent reading, e.g. dollars spent with no limit.
fn amount_text(window: &Value) -> Option<String> {
    let used: f64 = window["used"].as_str()?.parse().ok()?;
    let unit = window["unit"].as_str()?;
    let money = |v: f64| {
        if unit == "usd" {
            format!("${v:.2}")
        } else {
            format!("{v} {unit}")
        }
    };
    Some(
        match window["limit"].as_str().and_then(|l| l.parse::<f64>().ok()) {
            Some(limit) => format!("{} of {} used", money(used), money(limit)),
            None => format!("{} used, no limit", money(used)),
        },
    )
}

fn percent(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}%")
    } else {
        format!("{value:.1}%")
    }
}

/// Terminal rendering for `originlm usage` / `olm usage`: one block per
/// account with aligned window rows, then every detected provider that has no
/// reading, each with its reason. Reset times are shown in `offset` (the
/// caller's local zone).
pub fn usage_terminal(report: &Value, offset: chrono::FixedOffset) -> String {
    let rows: Vec<&Value> = report["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let with: Vec<&&Value> = rows
        .iter()
        .filter(|row| row["windows"].as_array().is_some_and(|w| !w.is_empty()))
        .collect();
    let without: Vec<&&Value> = rows
        .iter()
        .filter(|row| row["windows"].as_array().is_none_or(Vec::is_empty))
        .collect();
    let label_width = with
        .iter()
        .flat_map(|row| row["windows"].as_array().into_iter().flatten())
        .map(|w| w["label"].as_str().unwrap_or("window").chars().count())
        .max()
        .unwrap_or(0);
    let name = |row: &Value| {
        row["connectionName"]
            .as_str()
            .or(row["providerName"].as_str())
            .unwrap_or("?")
            .to_owned()
    };
    let mut out = String::new();
    for row in &with {
        let data = &row["data"];
        let mut header = name(row);
        if let Some(plan) = row["plan"].as_str() {
            header.push_str(&format!("  ·  {plan}"));
        }
        if let Some(account) = row["account"].as_str() {
            header.push_str(&format!("  ·  {account}"));
        }
        let freshness = match (data["state"].as_str(), data["ageSeconds"].as_i64()) {
            (Some("live"), Some(age)) => format!("live, read {age}s ago"),
            (Some("cached"), Some(age)) => format!("CACHED, {} old", duration_text(age)),
            _ => "age unknown".to_owned(),
        };
        out.push_str(&format!("{header}  ·  {freshness}"));
        if let Some(error) = data["refreshError"].as_str() {
            out.push_str(&format!("  (refresh failed: {error})"));
        }
        out.push('\n');
        for window in row["windows"].as_array().into_iter().flatten() {
            let label = window["label"].as_str().unwrap_or("window");
            let reading = match (
                window["usedPercent"].as_f64(),
                window["remainingPercent"].as_f64(),
            ) {
                (Some(used), Some(left)) => {
                    format!("{:>5} used  {:>5} left", percent(used), percent(left))
                }
                _ => amount_text(window).unwrap_or_else(|| format!("{:>9}", "unknown")),
            };
            let reset = match (
                parse_time(&window["resetsAt"]),
                window["resetsInSeconds"].as_i64(),
            ) {
                (Some(at), Some(secs)) => format!(
                    "  resets {} (in {})",
                    at.with_timezone(&offset).format("%a %b %-d %H:%M"),
                    duration_text(secs)
                ),
                _ => String::new(),
            };
            out.push_str(format!("  {label:<label_width$}  {reading}{reset}").trim_end());
            out.push('\n');
        }
        out.push('\n');
    }
    if !without.is_empty() {
        out.push_str("Detected without a usage reading:\n");
        let width = without
            .iter()
            .map(|row| name(row).chars().count())
            .max()
            .unwrap_or(0);
        for row in &without {
            out.push_str(&format!(
                "  {:<width$}  {}\n",
                name(row),
                status_without_reading(row)
            ));
        }
        out.push('\n');
    }
    for missing in report["notFound"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}: not found on this machine.\n",
            provider_name(missing.as_str().unwrap_or("?"))
        ));
    }
    if rows.is_empty() && report["notFound"].as_array().is_none_or(Vec::is_empty) {
        out.push_str("No AI provider was detected on this machine.\n");
    }
    out.push_str(
        "Percentages are the providers' own plan-quota readings; unknown is not zero.\n\
         Local token totals: olm usage --history\n",
    );
    out
}

/// Why a listed connection has no reading. Plain, and never a number.
fn status_without_reading(row: &Value) -> String {
    match row["data"]["state"].as_str() {
        Some("no_source") => format!(
            "detected; usage unavailable ({})",
            row["noSourceReason"]
                .as_str()
                .unwrap_or("OriginLM has no usage source for it")
        ),
        Some("not_read") => "detected; usage not read (excluded from this run)".to_owned(),
        _ => match (
            reading_missing_reason(row),
            row["data"]["refreshError"].as_str(),
        ) {
            (Some(reason), _) => format!("no usage: {reason}"),
            (None, Some(error)) => format!("no usage: read failed ({error})"),
            (None, None) => "detected; no usage reported".to_owned(),
        },
    }
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
                connection("conn-ollama","ollama","Ollama",Value::Null,unknown.clone(),unknown.clone()),
                connection("conn-cursor","cursor","Cursor",Value::Null,unknown.clone(),unknown.clone()),
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
            ProbeOutcome::Failed("PROVIDER_UNAVAILABLE".into(), vec![]),
        );
        let report = usage_report(
            &snapshot(),
            &wanted(),
            None,
            &probes,
            time("2026-11-11T16:00:01Z"),
            time("2026-11-11T16:00:30Z"),
            true,
        );
        let rows = report["providers"].as_array().unwrap();
        // Every detected connection is listed: readings, then granted without
        // one, then a usage source not granted, then no usage source at all.
        let order: Vec<_> = rows
            .iter()
            .map(|r| r["connectionId"].as_str().unwrap())
            .collect();
        assert_eq!(
            order,
            [
                "conn-claude",
                "conn-codex",
                "conn-codex-2",
                "conn-cursor",
                "conn-ollama"
            ]
        );
        assert_eq!(rows[2]["data"]["state"], "unavailable");
        assert_eq!(rows[3]["data"]["state"], "not_read");
        assert_eq!(rows[3]["usageSource"], "not_granted");
        assert_eq!(rows[4]["data"]["state"], "no_source");
        assert_eq!(rows[4]["usageSource"], "none");
        // Detected without a reading never becomes a number.
        assert_eq!(rows[4]["windows"], json!([]));
        assert!(rows[4]["data"]["ageSeconds"].is_null());

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
            "Additional profile 2 — no usage: not signed in",
            "Cursor — detected; usage not read (excluded from this run)",
            "Ollama — detected; usage unavailable (Ollama's local API reports sign-in and plan, not cloud usage limits)",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }

        let terminal = usage_terminal(&report, chrono::FixedOffset::west_opt(5 * 3600).unwrap());
        for expected in [
            "Claude Code · Default profile  ·  Claude Max  ·  me@example.com  ·  live, read 30s ago\n",
            "  5-hour session    16% used    84% left  resets Wed Nov 11 15:10 (in 4h 9m)\n",
            "  Monthly spend     unknown\n",
            "Codex · Default profile  ·  CACHED, 1h 0m old  (refresh failed: PROVIDER_UNAVAILABLE)\n",
            "Detected without a usage reading:\n",
            "  Codex · Additional profile 2  no usage: not signed in\n",
            "  Ollama                        detected; usage unavailable (Ollama's local API reports sign-in and plan, not cloud usage limits)\n",
            "Local token totals: olm usage --history\n",
        ] {
            assert!(
                terminal.contains(expected),
                "missing {expected:?} in:\n{terminal}"
            );
        }
        assert!(
            !terminal.contains('{'),
            "no JSON in terminal output:\n{terminal}"
        );
    }

    #[test]
    fn a_provider_filter_narrows_the_listing_and_names_what_is_missing() {
        let filter: BTreeSet<String> = ["ollama", "kiro"].map(String::from).into();
        let now = time("2026-11-11T16:00:00Z");
        let report = usage_report(
            &snapshot(),
            &wanted(),
            Some(&filter),
            &BTreeMap::new(),
            now,
            now,
            true,
        );
        let rows = report["providers"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["providerId"], "ollama");
        assert_eq!(report["notFound"], json!(["kiro"]));
    }

    #[test]
    fn an_empty_machine_says_so() {
        let now = time("2026-11-11T16:00:00Z");
        let empty = json!({"connections":[],"quotaPools":[],"accounts":[]});
        let report = usage_report(
            &empty,
            &BTreeSet::new(),
            None,
            &BTreeMap::new(),
            now,
            now,
            false,
        );
        let offset = chrono::FixedOffset::east_opt(0).unwrap();
        assert!(
            usage_terminal(&report, offset)
                .contains("No AI provider was detected on this machine.")
        );
        assert!(usage_text(&report).contains("No AI provider was detected on this machine."));
    }

    #[test]
    fn a_reading_older_than_this_call_is_cached_even_after_a_successful_probe() {
        let mut probes = BTreeMap::new();
        probes.insert("conn-claude".to_owned(), ProbeOutcome::Ok(true));
        let report = usage_report(
            &snapshot(),
            &wanted(),
            None,
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
        // A granted provider that is not installed is named, not dropped.
        let granted: BTreeSet<String> = ["claude_code", "kiro"].map(String::from).into();
        let now = time("2026-11-11T16:00:00Z");
        let report = usage_report(
            &snapshot(),
            &granted,
            None,
            &BTreeMap::new(),
            now,
            now,
            false,
        );
        let claude = row(&report, "conn-claude");
        assert!(claude["account"].is_null());
        assert_eq!(claude["identityHidden"], true);
        assert_eq!(report["notFound"], json!(["kiro"]));
        assert!(usage_text(&report).contains("Kiro: not found on this machine."));
    }

    #[test]
    fn providers_report_counts_windows_and_marks_grants() {
        let report = providers_report(&snapshot(), &wanted(), true);
        let rows = report["providers"].as_array().unwrap();
        assert_eq!(rows.len(), 5);
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
    fn rows_without_a_reading_say_why_from_the_adapter_diagnostics() {
        let row = |diagnostics: &[&str], auth: &str| json!({"data":{"state":"unavailable"},"auth":auth,"diagnostics":diagnostics});
        for (codes, expected) in [
            (&["GEMINI_API_KEY_AUTH_NO_QUOTA"][..], "Gemini API key"),
            (
                &["USAGE_NOT_YET_OBSERVED"],
                "only after this login has run a turn",
            ),
            (
                &[
                    "CONFIGURATION_DIRECTORY_PRESENT",
                    "METADATA_RUNTIME_MISSING",
                ],
                "runtime is missing",
            ),
            (&["COPILOT_GH_LOGIN_UNAVAILABLE"], "gh auth login"),
            (
                &["PI_RADIUS_NO_USAGE_SOURCE"],
                "Pi login `radius` has no usage source",
            ),
            (
                &["USAGE_UNAVAILABLE", "USAGE_READ_FAILED_Timeout"],
                "did not answer in time",
            ),
        ] {
            let text = status_without_reading(&row(codes, "authenticated"));
            assert!(text.contains(expected), "{codes:?} -> {text}");
        }
        // An unrecognised code is never turned into a guess.
        assert_eq!(
            status_without_reading(&row(&["SOMETHING_NEW"], "authenticated")),
            "detected; no usage reported"
        );
        assert_eq!(
            status_without_reading(
                &json!({"data":{"state":"no_source"},"noSourceReason":"local runtime; LM Studio has no plan quota"})
            ),
            "detected; usage unavailable (local runtime; LM Studio has no plan quota)"
        );
    }

    #[test]
    fn a_failed_read_keeps_the_adapter_reason_codes() {
        assert_eq!(
            detail_codes(
                "Native client timed out [NATIVE_SESSION_METADATA_NO_INFERENCE,GEMINI_API_KEY_AUTH_NO_QUOTA]"
            ),
            [
                "NATIVE_SESSION_METADATA_NO_INFERENCE",
                "GEMINI_API_KEY_AUTH_NO_QUOTA"
            ]
        );
        // Only well-formed codes are taken; ordinary text is not.
        assert!(detail_codes("plain message").is_empty());
        assert!(detail_codes("see [this link]").is_empty());
        let now = time("2026-11-11T16:00:00Z");
        let mut probes = BTreeMap::new();
        probes.insert(
            "conn-codex-2".to_owned(),
            ProbeOutcome::Failed(
                "TIMEOUT".into(),
                vec!["GEMINI_API_KEY_AUTH_NO_QUOTA".into()],
            ),
        );
        let report = usage_report(&snapshot(), &wanted(), None, &probes, now, now, false);
        let text = status_without_reading(row(&report, "conn-codex-2"));
        assert!(text.contains("Gemini API key"), "{text}");
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
