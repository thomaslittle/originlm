//! MSP turn view → kit turn events. Pure: no I/O, so recorded transcripts can
//! drive it directly.
//!
//! One kit turn covers the prompt's own Muse turn plus any turn Muse starts by
//! itself while that turn's workflows or subagents are still running (Muse
//! reports a finished workflow in a turn of its own, as T3 Code shows it).
//! Text, provider reasoning summaries, tool lifecycle, usage and warnings map
//! to the kit's neutral events. Muse-specific detail with no neutral event
//! (web search hits, workflow progress, subagents, todo lists, compaction) is
//! carried as bounded, sanitized `extension` events in the `muse.*`
//! namespaces documented in `docs/adapters/muse.md`.
use super::{bounded, launch::Posture};
use crate::host_mcp::HostTools;
use originlm_contracts::*;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Muse's native tools that only read, plan or delegate. A read-only session
/// (no shell, no writes, no web tools, no workflow launches at host level)
/// lets these run: Muse cannot remove them. Anything else fails the turn.
pub const READ_ONLY_TOOLS: &[&str] = &[
    "read_file",
    "search",
    "tool_search",
    "read_skill",
    "write_todos",
    "request_user_input",
    "subagent_spawn",
    "subagent_status",
    "subagent_send_message",
    "subagent_wait",
    "subagent_read_result",
    "subagent_cancel",
];

const MAX_TEXT_CHUNK: usize = 16_000;
const MAX_ITEMS: usize = 4096;
const MAX_HITS: usize = 20;
const MAX_CHILDREN: usize = 64;

/// How a session treats native tools and approvals.
#[derive(Debug, Clone)]
pub struct TurnPolicy {
    pub posture: Posture,
    /// Host MCP tools the session may call.
    pub host: HostTools,
    /// The host reviews each approval (`reviewRequests`).
    pub review: bool,
    /// Full access: Muse runs without asking (`allowAll`); a workflow child's
    /// approval, which does not inherit that mode, is approved too.
    pub full_access: bool,
}

/// A native approval the client must decide.
#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalAsk {
    pub approval_id: String,
    /// The `currentRequirementId` the decision must carry.
    pub requirement: Value,
    pub action: String,
    pub detail: String,
    /// The native choice for "allow once", when offered.
    pub allow_choice: Option<String>,
    /// The native choice for a denial (`denied`, else `abort`).
    pub deny_choice: Option<String>,
    /// Raised by this kit turn's own work (or one of its subagents).
    pub owned: bool,
    /// Raised by a native subagent or workflow child.
    pub child: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Emit(EventPayload),
    Approval(ApprovalAsk),
    /// Decline a structured question: the kit has no answer contract.
    DeclineQuestion {
        user_input_id: String,
    },
    /// Stop the turn with this error (the client interrupts Muse).
    Fail(DomainError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Completed,
    Cancelled,
    Failed(DomainError),
}

#[derive(Debug, Default)]
struct Tracked {
    kind: String,
    revision: u64,
    status: String,
    /// Bytes of `text` already emitted (agent messages).
    emitted: usize,
    /// A reasoning summary was streamed.
    streamed_summary: bool,
    tool: Option<String>,
    started_reported: bool,
    /// Last extension payload, to coalesce identical re-emissions.
    last_extension: Option<Value>,
    /// Workflow child labels by `childId:attempt`; later revisions drop them.
    labels: BTreeMap<String, String>,
}

pub struct TurnState {
    session: String,
    own: String,
    policy: TurnPolicy,
    owned: BTreeSet<String>,
    active: BTreeSet<String>,
    items: BTreeMap<String, Tracked>,
    background: BTreeSet<String>,
    workflows: BTreeMap<String, String>,
    own_outcome: Option<Outcome>,
    /// Muse started the own turn (it may first queue behind another turn).
    own_started: bool,
    /// Background work outlived the own turn, so Muse may start a report turn.
    follow_up: bool,
    /// Muse unloaded the session after this turn settled.
    closed: bool,
    decided: BTreeSet<(String, i64)>,
    context_tokens: Option<u64>,
    input_tokens: u64,
    output_tokens: u64,
    reasoning_tokens: u64,
    usage_seen: bool,
}

fn status_word(status: &str) -> &'static str {
    match status {
        "inProgress" => "started",
        "completed" => "completed",
        "cancelled" => "cancelled",
        _ => "failed",
    }
}

fn clean(value: &Value, limit: usize) -> Option<String> {
    value
        .as_str()
        .map(|s| {
            s.chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .collect::<String>()
        })
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .map(|s| bounded(&s, limit))
}

fn web_url(value: &Value) -> Option<String> {
    clean(value, 2048).filter(|u| {
        (u.starts_with("https://") || u.starts_with("http://")) && !u.contains(char::is_whitespace)
    })
}

/// The kit error for a failed Muse turn. Muse's own message is kept
/// (redacted): it carries actionable reasons such as "does not exist or you
/// lack access" for a listed model the account cannot use.
pub fn turn_failure(params: &Value) -> DomainError {
    let error = &params["error"];
    let kind = error["kind"].as_str().unwrap_or("");
    let message = redact_diagnostic_detail(
        error["message"]
            .as_str()
            .or_else(|| params["reason"].as_str())
            .unwrap_or(""),
    );
    if kind == "authRequired" {
        return DomainError::new(
            ErrorCode::AuthRequired,
            "Muse Code needs sign-in: run `muse login` on this computer",
        );
    }
    if error.get("quota").is_some_and(Value::is_object) {
        let mut failure = DomainError::new(
            ErrorCode::RateLimited,
            if message.is_empty() {
                "Muse usage limit reached".to_owned()
            } else {
                format!("Muse usage limit reached: {message}")
            },
        );
        let resets = error["quota"]["resetsAtMs"].as_u64();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        failure.retry_after_ms = resets.filter(|r| *r > now).map(|r| r - now);
        return failure;
    }
    let mut failure = DomainError::new(
        ErrorCode::ProviderUnavailable,
        match (kind.is_empty(), message.is_empty()) {
            (true, false) => format!("Muse turn failed: {message}"),
            (false, false) => format!("Muse turn failed ({kind}): {message}"),
            (false, true) => format!("Muse turn failed ({kind})"),
            (true, true) => "Muse turn failed without diagnostic text".to_owned(),
        },
    );
    failure.retryable = error["retryable"].as_bool().unwrap_or(false);
    failure
}

impl TurnState {
    /// `own` is the turn id Muse acknowledged for this kit turn
    /// (`turnId == commandId` for fresh turns).
    pub fn new(session: &str, own: &str, policy: TurnPolicy) -> Self {
        Self {
            session: session.into(),
            own: own.into(),
            policy,
            owned: BTreeSet::from([own.to_owned()]),
            active: BTreeSet::new(),
            items: BTreeMap::new(),
            background: BTreeSet::new(),
            workflows: BTreeMap::new(),
            own_outcome: None,
            own_started: false,
            follow_up: false,
            closed: false,
            decided: BTreeSet::new(),
            context_tokens: None,
            input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            usage_seen: false,
        }
    }
    pub fn own_turn(&self) -> &str {
        &self.own
    }
    /// Muse turns owned by this kit turn that have not completed.
    pub fn active_turns(&self) -> Vec<String> {
        self.active.iter().cloned().collect()
    }
    /// Running workflow run ids launched by this kit turn.
    pub fn running_workflows(&self) -> Vec<String> {
        self.workflows
            .iter()
            .filter(|(item, _)| self.background.contains(*item))
            .map(|(_, run)| run.clone())
            .collect()
    }
    /// Background work outlived the own turn, so Muse may start a report
    /// turn for it: the client briefly waits for one after settling.
    pub fn expects_follow_up(&self) -> bool {
        self.follow_up && !self.closed
    }
    /// The own turn's terminal, once Muse reported it.
    pub fn own_outcome(&self) -> Option<&Outcome> {
        self.own_outcome.as_ref()
    }
    /// The kit turn may end: its own turn settled and, after a successful
    /// turn, no owned Muse turn or background work is still running.
    pub fn settled(&self) -> bool {
        match &self.own_outcome {
            None => false,
            Some(Outcome::Completed) => self.active.is_empty() && self.background.is_empty(),
            Some(_) => true,
        }
    }
    /// Usage and context totals for the end of the turn.
    pub fn summary(&self) -> Vec<EventPayload> {
        let mut events = Vec::new();
        if self.usage_seen {
            for (unit, used) in [
                ("input_tokens", self.input_tokens),
                ("output_tokens", self.output_tokens),
                ("reasoning_tokens", self.reasoning_tokens),
            ] {
                events.push(EventPayload::Usage {
                    unit: unit.into(),
                    used: used.to_string(),
                });
            }
        }
        if let Some(used) = self.context_tokens {
            events.push(EventPayload::Usage {
                unit: "context_tokens".into(),
                used: used.to_string(),
            });
        }
        events
    }
    fn joins(&self) -> bool {
        // A turn Muse starts by itself belongs to this kit turn while the own
        // turn runs, or while its background work runs or has just settled.
        self.own_started
            && (self.own_outcome.is_none() || !self.background.is_empty() || self.follow_up)
    }

    /// Apply one inbound MSP frame (notification or server request).
    pub fn handle(&mut self, frame: &Value) -> Vec<Action> {
        let mut out = Vec::new();
        let Some(method) = frame["method"].as_str() else {
            return out;
        };
        let params = &frame["params"];
        if let Some(session) = params["sessionId"].as_str()
            && session != self.session
        {
            return out;
        }
        match method {
            "turn/started" => {
                let Some(turn) = params["turnId"].as_str() else {
                    return out;
                };
                if turn == self.own {
                    self.own_started = true;
                }
                if turn == self.own || (!self.owned.contains(turn) && self.joins()) {
                    self.owned.insert(turn.to_owned());
                    if turn != self.own || self.own_outcome.is_none() {
                        self.active.insert(turn.to_owned());
                    }
                }
            }
            "turn/completed" => self.turn_completed(params, &mut out),
            "item/started" | "item/updated" | "item/completed" => {
                self.item(&params["item"], &mut out)
            }
            "item/delta" => self.delta(params, &mut out),
            "approval/request" | "approval/requested" | "approval/updated" => {
                self.approval(params, &mut out)
            }
            "approval/resolved" => {
                if self.policy.review && params["resolvedBy"] == "llmJudge" {
                    out.push(Action::Emit(EventPayload::Warning {
                        code: "NATIVE_APPROVAL_JUDGE".into(),
                        message: "Muse's approval judge decided a request without the host".into(),
                    }));
                }
            }
            "userInput/request" | "userInput/requested" => {
                if let Some(id) = params["userInputId"].as_str()
                    && self.decided.insert((format!("question:{id}"), 0))
                {
                    out.push(Action::DeclineQuestion {
                        user_input_id: id.to_owned(),
                    });
                    out.push(Action::Emit(EventPayload::Warning {
                            code: "NATIVE_QUESTION_DECLINED".into(),
                            message: "Muse asked a structured question; this integration cannot answer it, so Muse continues without an answer".into(),
                        }));
                }
            }
            "session/tokenUsage" => {
                if params["turnId"]
                    .as_str()
                    .is_some_and(|t| self.owned.contains(t))
                {
                    let usage = &params["usage"];
                    self.usage_seen = true;
                    self.input_tokens = self
                        .input_tokens
                        .saturating_add(params["promptTokens"].as_u64().unwrap_or(0));
                    self.output_tokens = self
                        .output_tokens
                        .saturating_add(usage["outputTokens"].as_u64().unwrap_or(0));
                    self.reasoning_tokens = self
                        .reasoning_tokens
                        .saturating_add(usage["reasoningTokens"].as_u64().unwrap_or(0));
                }
            }
            "session/contextUsage" => {
                if let Some(used) = params["usedTokens"].as_u64() {
                    self.context_tokens = Some(used);
                }
            }
            "session/todoListChanged" => {
                let items: Vec<Value> = params["items"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .take(64)
                            .filter_map(|item| {
                                Some(json!({
                                    "text": clean(&item["text"], 512)?,
                                    "status": clean(&item["status"], 32).unwrap_or_else(|| "pending".into()),
                                }))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(Action::Emit(EventPayload::Extension {
                    namespace: "muse.todo".into(),
                    data: json!({"items": items}),
                }));
            }
            "turn/retryScheduled" => {
                if params["turnId"]
                    .as_str()
                    .is_some_and(|t| self.owned.contains(t))
                {
                    out.push(Action::Emit(EventPayload::Warning {
                        code: "PROVIDER_RETRY".into(),
                        message: format!(
                            "Muse is retrying this turn (attempt {} of {}): {}",
                            params["nextAttempt"].as_u64().unwrap_or(0),
                            params["maxAttempts"].as_u64().unwrap_or(0),
                            redact_diagnostic_detail(params["reason"].as_str().unwrap_or("")),
                        ),
                    }));
                }
            }
            "session/modelRouteUnserved" => out.push(Action::Emit(EventPayload::Warning {
                code: "MODEL_ROUTE_UNSERVED".into(),
                message: "Muse cannot route the selected model with the installed provider".into(),
            })),
            "view/gap" => out.push(Action::Fail(DomainError::new(
                ErrorCode::EventGap,
                "Muse dropped session events; reopen the session",
            ))),
            "session/closed" => {
                if self.settled() {
                    self.closed = true;
                } else {
                    out.push(Action::Fail(DomainError::new(
                        ErrorCode::ProviderUnavailable,
                        "Muse closed the session",
                    )));
                }
            }
            _ => {}
        }
        out
    }

    fn turn_completed(&mut self, params: &Value, out: &mut Vec<Action>) {
        let Some(turn) = params["turnId"].as_str() else {
            return;
        };
        if !self.owned.contains(turn) {
            return;
        }
        self.active.remove(turn);
        let terminal = params["terminal"].as_str().unwrap_or("");
        if turn == self.own {
            if self.own_outcome.is_some() {
                return;
            }
            self.own_outcome = Some(match terminal {
                "completed" => Outcome::Completed,
                "cancelled" => Outcome::Cancelled,
                _ => Outcome::Failed(turn_failure(params)),
            });
            // Workflows and subagents may outlive a successful turn; a
            // stopped or failed turn ends them for the kit.
            if terminal == "completed" {
                self.follow_up = !self.background.is_empty();
            } else {
                self.background.clear();
            }
        } else if terminal != "completed" {
            let failure = turn_failure(params);
            out.push(Action::Emit(EventPayload::Warning {
                code: "NATIVE_FOLLOW_UP_TURN_FAILED".into(),
                message: failure.message,
            }));
        }
    }

    fn delta(&mut self, params: &Value, out: &mut Vec<Action>) {
        let (Some(id), Some(delta)) = (params["itemId"].as_str(), params["delta"].as_str()) else {
            return;
        };
        let Some(tracked) = self.items.get_mut(id) else {
            return;
        };
        if tracked.status != "inProgress" {
            return;
        }
        let field = params["field"].as_str().unwrap_or("text");
        match (tracked.kind.as_str(), field) {
            ("agentMessage", "text") => {
                tracked.emitted += delta.len();
                emit_text(out, delta);
            }
            ("reasoning", field) if field.starts_with("summary.") => {
                tracked.streamed_summary = true;
                if !delta.is_empty() {
                    out.push(Action::Emit(EventPayload::ReasoningSummary {
                        text: bounded(delta, MAX_TEXT_CHUNK),
                    }));
                }
            }
            _ => {}
        }
    }

    fn item(&mut self, item: &Value, out: &mut Vec<Action>) {
        let (Some(id), Some(kind)) = (item["itemId"].as_str(), item["kind"].as_str()) else {
            return;
        };
        let revision = item["revision"].as_u64().unwrap_or(0);
        let status = item["status"].as_str().unwrap_or("").to_owned();
        let known = self.items.contains_key(id);
        if !known {
            let owned = item["turnId"]
                .as_str()
                .is_some_and(|turn| self.owned.contains(turn));
            if !owned || self.items.len() >= MAX_ITEMS {
                return;
            }
        }
        let tracked = self.items.entry(id.to_owned()).or_default();
        if known && revision <= tracked.revision {
            return;
        }
        tracked.revision = revision;
        tracked.kind = kind.to_owned();
        let previous = std::mem::replace(&mut tracked.status, status.clone());
        let terminal = status != "inProgress";
        match kind {
            "agentMessage" => {
                if terminal
                    && let Some(text) = item["text"].as_str()
                    && tracked.emitted < text.len()
                    && text.is_char_boundary(tracked.emitted)
                {
                    emit_text(out, &text[tracked.emitted..]);
                    tracked.emitted = text.len();
                }
            }
            "reasoning" => {
                if terminal && !tracked.streamed_summary {
                    tracked.streamed_summary = true;
                    for part in item["summary"].as_array().into_iter().flatten().take(64) {
                        if let Some(text) = part.as_str().filter(|t| !t.is_empty()) {
                            out.push(Action::Emit(EventPayload::ReasoningSummary {
                                text: bounded(text, MAX_TEXT_CHUNK),
                            }));
                        }
                    }
                }
            }
            "toolCall" => {
                let tool = item["tool"]
                    .as_str()
                    .filter(|t| !t.is_empty() && t.len() <= 256 && !t.chars().any(char::is_control))
                    .unwrap_or("tool")
                    .to_owned();
                tracked.tool = Some(tool.clone());
                if !tracked.started_reported {
                    tracked.started_reported = true;
                    if let Err(error) = self.policy_allows(&tool) {
                        out.push(Action::Fail(error));
                        return;
                    }
                    out.push(Action::Emit(EventPayload::Tool {
                        name: tool.clone(),
                        status: "started".into(),
                    }));
                }
                if terminal && previous != status {
                    out.push(Action::Emit(EventPayload::Tool {
                        name: tool.clone(),
                        status: status_word(&status).into(),
                    }));
                    if matches!(tool.as_str(), "web_search" | "web_fetch")
                        && let Some(data) = web_extension(&tool, item, &status)
                    {
                        out.push(Action::Emit(EventPayload::Extension {
                            namespace: "muse.web_search".into(),
                            data,
                        }));
                    }
                }
            }
            "webSearch" => {
                if !tracked.started_reported {
                    tracked.started_reported = true;
                    if let Err(error) = self.policy_allows("web_search") {
                        out.push(Action::Fail(error));
                        return;
                    }
                    out.push(Action::Emit(EventPayload::Tool {
                        name: "web_search".into(),
                        status: "started".into(),
                    }));
                }
                if terminal && previous != status {
                    out.push(Action::Emit(EventPayload::Tool {
                        name: "web_search".into(),
                        status: status_word(&status).into(),
                    }));
                    if let Some(data) = web_extension("web_search", item, &status) {
                        out.push(Action::Emit(EventPayload::Extension {
                            namespace: "muse.web_search".into(),
                            data,
                        }));
                    }
                }
            }
            "workflow" => {
                if let Some(run) = clean(&item["workflowRunId"], 256) {
                    self.workflows.insert(id.to_owned(), run);
                }
                for child in item["children"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(MAX_CHILDREN)
                {
                    if let (Some(child_id), Some(label)) =
                        (child["childId"].as_str(), clean(&child["label"], 256))
                    {
                        let key = format!("{child_id}:{}", child["attempt"].as_u64().unwrap_or(0));
                        tracked.labels.insert(key, label);
                    }
                }
                let data = workflow_extension(id, item, &status, &tracked.labels);
                push_extension(tracked, out, "muse.workflow", data);
                self.track_background(id, !terminal);
            }
            "subagent" => {
                let ready = item["controlStatus"] == "resultReady";
                let data = subagent_extension(id, item, &status);
                push_extension(tracked, out, "muse.subagent", data);
                self.track_background(id, !terminal && !ready);
            }
            "compaction" if terminal => {
                let data = json!({
                    "itemId": id,
                    "status": status,
                    "outcome": clean(&item["outcome"], 64),
                    "trigger": clean(&item["trigger"], 64),
                });
                push_extension(tracked, out, "muse.compaction", data);
            }
            // Housekeeping (skill reminders), echoes of the prompt, hooks and
            // side chats are not turn output.
            _ => {}
        }
    }

    fn track_background(&mut self, id: &str, running: bool) {
        if running {
            self.background.insert(id.to_owned());
        } else {
            self.background.remove(id);
        }
    }

    /// Read-only sessions accept host tools and Muse's read-only tools only.
    fn policy_allows(&self, tool: &str) -> Result<()> {
        if self.policy.host.allows_qualified(tool) {
            return Ok(());
        }
        if tool.starts_with("mcp__") && !self.policy.host.is_empty() {
            return Err(crate::host_mcp::native_tool_refused());
        }
        if self.policy.posture == Posture::ReadOnly && !self.policy.full_access {
            if READ_ONLY_TOOLS.contains(&tool) {
                return Ok(());
            }
            return Err(crate::host_mcp::native_tool_refused());
        }
        Ok(())
    }

    fn approval(&mut self, params: &Value, out: &mut Vec<Action>) {
        let Some(approval) = params["approvalId"].as_str() else {
            return;
        };
        let requirement = params["currentRequirementId"].clone();
        let stage = requirement["sourceIndex"].as_i64().unwrap_or(-1);
        if requirement["approvalId"] != approval || stage < 0 {
            return;
        }
        if self.decided.len() >= 1024 {
            out.push(Action::Fail(DomainError::new(
                ErrorCode::FrameTooLarge,
                "Too many Muse approvals in one turn",
            )));
            return;
        }
        if !self.decided.insert((approval.to_owned(), stage)) {
            return;
        }
        let child = params.get("subagentOrigin").is_some_and(Value::is_object);
        let owned = child
            || params["turnId"]
                .as_str()
                .is_some_and(|turn| self.owned.contains(turn));
        let choices = params["availableChoices"].as_array();
        let pick = |decision: &str| {
            choices
                .into_iter()
                .flatten()
                .find(|c| c["decision"] == decision && c["scope"] == "once")
                .and_then(|c| c["choiceId"].as_str())
                .map(str::to_owned)
        };
        let subject = &params["subject"];
        let kind = subject["kind"].as_str().unwrap_or("");
        let verb = match kind {
            "shell" => "run a command",
            "fileAccess" if subject["access"] == "read" => "read a file",
            "fileAccess" => "change a file",
            "network" => "use the network",
            "tool" => "use a tool",
            "process" => "control a process",
            _ => "perform a restricted action",
        };
        let target = [
            &subject["command"],
            &subject["path"],
            &subject["host"],
            &subject["target"],
            &subject["toolName"],
        ]
        .into_iter()
        .find_map(|v| clean(v, 4000))
        .or_else(|| clean(&params["toolName"], 256))
        .unwrap_or_else(|| "Muse permission request".into());
        let mut detail = target;
        if let Some(tool) = clean(&params["toolName"], 256) {
            detail = format!("{detail}\nTool: {tool}");
        }
        out.push(Action::Approval(ApprovalAsk {
            approval_id: approval.to_owned(),
            requirement,
            action: format!(
                "{} wants to {verb}",
                if child { "A Muse subagent" } else { "Muse" }
            ),
            detail: bounded(&detail, 8000),
            allow_choice: pick("approved"),
            deny_choice: pick("denied").or_else(|| pick("abort")),
            owned,
            child,
        }));
    }
}

fn emit_text(out: &mut Vec<Action>, text: &str) {
    let mut chunk = String::new();
    for ch in text.chars() {
        chunk.push(ch);
        if chunk.len() >= MAX_TEXT_CHUNK {
            out.push(Action::Emit(EventPayload::TextDelta {
                text: std::mem::take(&mut chunk),
            }));
        }
    }
    if !chunk.is_empty() {
        out.push(Action::Emit(EventPayload::TextDelta { text: chunk }));
    }
}

fn push_extension(tracked: &mut Tracked, out: &mut Vec<Action>, namespace: &str, data: Value) {
    if tracked.last_extension.as_ref() == Some(&data) {
        return;
    }
    tracked.last_extension = Some(data.clone());
    out.push(Action::Emit(EventPayload::Extension {
        namespace: namespace.into(),
        data,
    }));
}

fn strip_nulls(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        map.retain(|_, v| !v.is_null());
    }
    value
}

/// `muse.web_search`: the query and linked hits of a search, or the page and
/// an excerpt of a fetch.
fn web_extension(tool: &str, item: &Value, status: &str) -> Option<Value> {
    let args: Value = item["args"]
        .as_str()
        .and_then(|a| serde_json::from_str(a).ok())
        .unwrap_or(Value::Null);
    let output = item["visibleOutput"].as_str().unwrap_or("");
    let mut data = Map::new();
    data.insert("itemId".into(), item["itemId"].clone());
    data.insert("tool".into(), json!(tool));
    data.insert("status".into(), json!(status));
    if let Some(query) = clean(&args["query"], 512).or_else(|| clean(&item["query"], 512)) {
        data.insert("query".into(), json!(query));
    }
    let mut hits = Vec::new();
    if tool == "web_search" && status == "completed" {
        let parsed: Value = serde_json::from_str(output).unwrap_or(Value::Null);
        let results = parsed["results"]
            .as_array()
            .or_else(|| item["results"].as_array());
        for hit in results.into_iter().flatten().take(MAX_HITS) {
            let entry = strip_nulls(json!({
                "url": web_url(&hit["url"]),
                "title": clean(&hit["title"], 256),
                "snippet": clean(&hit["snippet"], 500),
            }));
            if entry.as_object().is_some_and(|m| !m.is_empty()) {
                hits.push(entry);
            }
        }
    } else if tool == "web_fetch" {
        let entry = strip_nulls(json!({
            "url": web_url(&args["url"]),
            "snippet": clean(&json!(output), 500),
        }));
        if entry.as_object().is_some_and(|m| !m.is_empty()) {
            hits.push(entry);
        }
    }
    data.insert("results".into(), Value::Array(hits));
    if status != "completed"
        && let Some(reason) = clean(&item["failureReason"], 500)
    {
        data.insert("failure".into(), json!(redact_diagnostic_detail(&reason)));
    }
    Some(Value::Object(data))
}

/// `muse.workflow`: a launched workflow, its child agents and its result.
fn workflow_extension(
    id: &str,
    item: &Value,
    status: &str,
    labels: &BTreeMap<String, String>,
) -> Value {
    let children: Vec<Value> = item["children"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_CHILDREN)
        .map(|child| {
            let key = format!(
                "{}:{}",
                child["childId"].as_str().unwrap_or(""),
                child["attempt"].as_u64().unwrap_or(0)
            );
            strip_nulls(json!({
                "childId": clean(&child["childId"], 128),
                "attempt": child["attempt"].as_u64(),
                "label": labels.get(&key),
                "status": clean(&child["status"], 64),
                "terminal": clean(&child["terminal"], 64),
                "failure": clean(&child["failureReason"], 500)
                    .or_else(|| clean(&child["failureKind"], 128))
                    .map(|f| redact_diagnostic_detail(&f)),
            }))
        })
        .collect();
    strip_nulls(json!({
        "itemId": id,
        "status": status,
        "entryId": clean(&item["entryId"], 256),
        "label": clean(&item["fallbackText"], 256),
        "children": children,
        "finalSummary": clean(&item["finalSummary"], 4000),
        "durationMs": item["durationMs"].as_u64(),
    }))
}

/// `muse.subagent`: a native subagent's objective, state and result.
fn subagent_extension(id: &str, item: &Value, status: &str) -> Value {
    strip_nulls(json!({
        "itemId": id,
        "status": status,
        "controlStatus": clean(&item["controlStatus"], 64),
        "subagentId": clean(&item["subagentId"], 128),
        "role": clean(&item["role"], 256),
        "objective": clean(&item["objective"], 1000),
        "result": clean(&item["result"]["summary"], 4000)
            .or_else(|| clean(&item["result"]["text"], 4000)),
        "failure": clean(&item["failureReason"], 500).map(|f| redact_diagnostic_detail(&f)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const S: &str = "01a13400-0000-7000-8000-00000000000a";
    const OWN: &str = "01a13400-0000-7000-8000-00000000000b";
    const OTHER: &str = "01a13400-0000-7000-8000-00000000000c";
    fn state(review: bool) -> TurnState {
        TurnState::new(
            S,
            OWN,
            TurnPolicy {
                posture: Posture::Workspace,
                host: HostTools::default(),
                review,
                full_access: false,
            },
        )
    }
    fn frame(method: &str, params: Value) -> Value {
        let mut params = params;
        params["sessionId"] = json!(S);
        json!({"jsonrpc":"2.0","method":method,"params":params})
    }
    fn message(turn: &str, id: &str, text: &str) -> Vec<Value> {
        vec![
            frame(
                "item/started",
                json!({"item":{"itemId":id,"kind":"agentMessage","turnId":turn,"revision":1,"status":"inProgress","text":""}}),
            ),
            frame(
                "item/delta",
                json!({"itemId":id,"field":"text","delta":text}),
            ),
        ]
    }
    fn texts(actions: &[Action]) -> String {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Emit(EventPayload::TextDelta { text }) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
    fn run(state: &mut TurnState, frames: &[Value]) -> Vec<Action> {
        frames.iter().flat_map(|f| state.handle(f)).collect()
    }

    #[test]
    fn a_queued_turn_ignores_the_turn_muse_is_still_running() {
        let mut s = state(false);
        let mut frames = vec![frame("turn/started", json!({"turnId":OTHER}))];
        frames.extend(message(OTHER, "i1", "not ours"));
        frames.push(frame(
            "turn/completed",
            json!({"turnId":OTHER,"terminal":"completed"}),
        ));
        frames.push(frame("turn/started", json!({"turnId":OWN})));
        frames.extend(message(OWN, "i2", "ours"));
        frames.push(frame(
            "turn/completed",
            json!({"turnId":OWN,"terminal":"completed"}),
        ));
        let actions = run(&mut s, &frames);
        assert_eq!(texts(&actions), "ours");
        assert!(s.settled());
        assert!(!s.expects_follow_up());
    }

    #[test]
    fn a_report_turn_for_outliving_work_joins_the_kit_turn() {
        let mut s = state(false);
        let workflow = |rev: u64, status: &str| {
            frame(
                "item/updated",
                json!({"item":{"itemId":"w","kind":"workflow","turnId":OWN,"revision":rev,"status":status,"workflowRunId":"run-1","children":[]}}),
            )
        };
        run(
            &mut s,
            &[
                frame("turn/started", json!({"turnId":OWN})),
                workflow(1, "inProgress"),
            ],
        );
        run(
            &mut s,
            &[frame(
                "turn/completed",
                json!({"turnId":OWN,"terminal":"completed"}),
            )],
        );
        assert!(!s.settled());
        assert_eq!(s.running_workflows(), ["run-1"]);
        run(&mut s, &[workflow(2, "completed")]);
        assert!(s.settled() && s.expects_follow_up());
        let mut frames = vec![frame("turn/started", json!({"turnId":OTHER}))];
        frames.extend(message(OTHER, "r", "report"));
        let actions = run(&mut s, &frames);
        assert_eq!(texts(&actions), "report");
        assert!(!s.settled());
        run(
            &mut s,
            &[frame(
                "turn/completed",
                json!({"turnId":OTHER,"terminal":"completed"}),
            )],
        );
        assert!(s.settled());
    }

    #[test]
    fn approval_twins_ask_once_and_each_stage_asks_again() {
        let mut s = state(true);
        let approval = |stage: i64| {
            json!({"approvalId":"a","turnId":OWN,"currentRequirementId":{"approvalId":"a","sourceIndex":stage},
                "subject":{"kind":"fileAccess","access":"write","path":"src/x.rs"},"toolName":"edit_file",
                "availableChoices":[{"choiceId":"ok","decision":"approved","scope":"once"},
                    {"choiceId":"always","decision":"approvedPolicyAmendment","scope":"localPersistent"},
                    {"choiceId":"no","decision":"denied","scope":"once"},
                    {"choiceId":"stop","decision":"abort","scope":"once"}]})
        };
        let mut request = frame("approval/request", approval(0));
        request["id"] = json!(1);
        let actions = run(
            &mut s,
            &[
                frame("turn/started", json!({"turnId":OWN})),
                request,
                frame("approval/requested", approval(0)),
                frame("approval/updated", approval(1)),
            ],
        );
        let asks: Vec<_> = actions
            .iter()
            .filter_map(|a| match a {
                Action::Approval(ask) => Some(ask),
                _ => None,
            })
            .collect();
        assert_eq!(asks.len(), 2);
        assert_eq!(asks[0].action, "Muse wants to change a file");
        assert_eq!(
            asks[0].detail,
            "src/x.rs
Tool: edit_file"
        );
        assert_eq!(asks[0].allow_choice.as_deref(), Some("ok"));
        // A denial is the once-only `denied`, never a persistent amendment.
        assert_eq!(asks[0].deny_choice.as_deref(), Some("no"));
        assert!(asks[0].owned && !asks[0].child);
        assert_eq!(asks[1].requirement["sourceIndex"], 1);
    }

    #[test]
    fn frames_for_other_sessions_and_raw_reasoning_are_ignored() {
        let mut s = state(false);
        let mut foreign = frame("turn/started", json!({"turnId":OWN}));
        foreign["params"]["sessionId"] = json!("someone-else");
        let reasoning = |field: &str, delta: &str| {
            frame(
                "item/delta",
                json!({"itemId":"r","field":field,"delta":delta}),
            )
        };
        let actions = run(
            &mut s,
            &[
                foreign,
                frame("turn/started", json!({"turnId":OWN})),
                frame(
                    "item/started",
                    json!({"item":{"itemId":"r","kind":"reasoning","turnId":OWN,"revision":1,"status":"inProgress"}}),
                ),
                reasoning("text", "private chain of thought"),
                reasoning("summary.0", "Public summary"),
            ],
        );
        assert_eq!(
            actions,
            [Action::Emit(EventPayload::ReasoningSummary {
                text: "Public summary".into()
            })]
        );
    }
}
