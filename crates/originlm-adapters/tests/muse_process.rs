//! Muse adapter against recorded MSP transcripts replayed by a fake
//! `muse serve` (`tests/support/muse_peer.py`). `fixtures/muse/t3/*` are T3
//! Code's recordings (MIT; Muse 1.4.3-1.4.6), `fixtures/muse/echo_*` were
//! recorded from Muse 1.4.7's echo provider, and `fixtures/muse/synthetic/*`
//! are derived from that recording. No account or network is used.
use originlm_adapters::{
    muse::{MuseOpen, Posture, Routing, open_spec, probe_spec},
    process::LaunchSpec,
};
use originlm_contracts::*;
use originlm_runtime::{ImageInput, Runtime};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

fn python() -> PathBuf {
    std::env::var_os("LAI_TEST_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(if cfg!(windows) {
                "C:/Python313/python.exe"
            } else {
                "/usr/bin/python3"
            })
        })
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}
fn spec(transcript: &str, log: &Path, mode: &str) -> LaunchSpec {
    LaunchSpec {
        executable: python(),
        args: vec![
            root().join("support/muse_peer.py").into(),
            root()
                .join(format!("fixtures/muse/{transcript}.ndjson"))
                .into(),
            log.into(),
            mode.into(),
        ],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}

struct Options {
    posture: Posture,
    review: bool,
    full_access: bool,
    host: Vec<HostMcpServer>,
    persistent: bool,
    resume: Option<String>,
    mode: &'static str,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            posture: Posture::ReadOnly,
            review: false,
            full_access: false,
            host: vec![],
            persistent: false,
            resume: None,
            mode: "normal",
        }
    }
}

struct Harness {
    runtime: Runtime,
    session: Session,
    log: PathBuf,
    _dir: tempfile::TempDir,
}

async fn model(transcript: &str, dir: &Path) -> Model {
    probe_spec(
        spec(transcript, &dir.join("probe.jsonl"), "normal"),
        "c",
        "r",
    )
    .await
    .unwrap()
    .models
    .remove(0)
}

fn config(model: Model, dir: &Path, options: &Options) -> MuseOpen {
    MuseOpen {
        model,
        connection: "c".into(),
        route: "r".into(),
        workspace: dir.into(),
        posture: options.posture,
        review: options.review,
        full_access: options.full_access,
        host_mcp_servers: options.host.clone(),
        persistent: options.persistent,
        resume: options.resume.clone(),
        routing: Routing::Meta,
    }
}

async fn try_setup(transcript: &str, options: Options) -> (Result<Harness>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("session.jsonl");
    let model = model(transcript, dir.path()).await;
    let opened = open_spec(
        spec(transcript, &log, options.mode),
        config(model.clone(), dir.path(), &options),
    )
    .await;
    let harness = opened.map(|agent| {
        let runtime = Runtime::new();
        let session = runtime
            .insert(
                "owner",
                "c".into(),
                Selection {
                    route_id: "r".into(),
                    model_id: model.id.clone(),
                    revision: model.revision.clone(),
                    options: BTreeMap::new(),
                },
                agent,
                Arc::new(|| Ok(())),
            )
            .unwrap();
        Harness {
            runtime,
            session,
            log: log.clone(),
            _dir: dir,
        }
    });
    (harness, log)
}

async fn setup(transcript: &str, options: Options) -> Harness {
    try_setup(transcript, options).await.0.unwrap()
}

impl Harness {
    fn events(&self) -> Vec<Value> {
        self.runtime
            .events("owner", Some(&self.session.id), None)
            .unwrap()["events"]
            .as_array()
            .unwrap()
            .clone()
    }
    fn sent(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    fn sent_method(&self, method: &str) -> Vec<Value> {
        self.sent()
            .into_iter()
            .filter(|m| m["method"] == method)
            .collect()
    }
    fn state(&self) -> SessionState {
        self.runtime.get("owner", &self.session.id).unwrap().state
    }
    async fn wait(&self) {
        tokio::time::timeout(Duration::from_secs(15), async {
            while matches!(
                self.state(),
                SessionState::Running | SessionState::AwaitingApproval
            ) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("turn did not settle");
    }
    /// Run one turn, answering host approvals in order with `answers`.
    async fn turn(&self, prompt: &str, request: &str, answers: &[bool]) -> Vec<Value> {
        let before = self.events().len();
        self.runtime
            .start("owner", &self.session.id, prompt.into(), request.into())
            .await
            .unwrap();
        let mut answers = answers.iter();
        let mut answered = std::collections::BTreeSet::new();
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                for event in self.events().split_off(before) {
                    if event["payload"]["type"] == "approval_request" {
                        let id = event["payload"]["approvalId"].as_str().unwrap().to_owned();
                        if answered.insert(id.clone()) {
                            let allow = *answers.next().expect("unexpected approval");
                            self.runtime.approve("owner", &id, allow).unwrap();
                        }
                    }
                }
                if !matches!(
                    self.state(),
                    SessionState::Running | SessionState::AwaitingApproval
                ) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("turn did not settle");
        let events = self.events().split_off(before);
        if std::env::var_os("MUSE_TEST_DEBUG").is_some() {
            for event in &events {
                eprintln!("{}", event["payload"]);
            }
        }
        events
    }
}

fn text(events: &[Value]) -> String {
    events
        .iter()
        .filter(|e| e["payload"]["type"] == "text_delta")
        .map(|e| e["payload"]["text"].as_str().unwrap())
        .collect()
}
fn payloads<'a>(events: &'a [Value], kind: &str) -> Vec<&'a Value> {
    events
        .iter()
        .map(|e| &e["payload"])
        .filter(|p| p["type"] == kind)
        .collect()
}
fn final_status(events: &[Value]) -> String {
    payloads(events, "final")
        .last()
        .map(|p| p["status"].as_str().unwrap().to_owned())
        .unwrap_or_default()
}
fn error_code(events: &[Value]) -> String {
    payloads(events, "error")
        .last()
        .map(|p| p["code"].as_str().unwrap().to_owned())
        .unwrap_or_default()
}

#[tokio::test]
async fn metadata_reports_account_lane_catalog_and_efforts_without_a_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("probe.jsonl");
    let report = probe_spec(spec("t3/simple", &log, "normal"), "c", "r")
        .await
        .unwrap();
    assert_eq!(report.version.value().map(String::as_str), Some("1.4.7"));
    assert_eq!(report.auth.value(), Some(&AuthState::Authenticated));
    assert_eq!(report.billing.value(), Some(&BillingMode::Subscription));
    assert!(report.plan.value().is_none());
    let model = &report.models[0];
    assert_eq!(model.native_id, "muse-spark-1.3-contributor");
    assert_eq!(
        model.context_window.value().map(String::as_str),
        Some("262144")
    );
    assert!(model.is_default);
    let effort = &model.options[0];
    assert_eq!(effort.id, "reasoningEffort");
    assert_eq!(effort.scope, OptionScope::Turn);
    match &effort.control {
        OptionControl::Select {
            choices,
            default_value,
        } => {
            assert_eq!(
                choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
                ["low", "medium", "high", "xhigh"]
            );
            assert_eq!(default_value.as_deref(), Some("high"));
        }
        other => panic!("unexpected control {other:?}"),
    }
    assert!(
        model
            .capabilities
            .iter()
            .any(|c| c.id == "input.image" && c.support == Support::Supported)
    );
    let sent: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let methods: Vec<_> = sent.iter().filter_map(|m| m["method"].as_str()).collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "initialized",
            "account/read",
            "model/list",
            "usage/read"
        ]
    );
    assert_eq!(sent[0]["params"]["capabilities"]["experimentalApi"], true);
    assert_eq!(sent[0]["params"]["clientInfo"]["name"], "originlm");
}

#[tokio::test]
async fn logged_out_muse_is_unauthenticated_and_cannot_open_a_session() {
    let dir = tempfile::tempdir().unwrap();
    let report = probe_spec(
        spec("t3/simple", &dir.path().join("p.jsonl"), "logged_out"),
        "c",
        "r",
    )
    .await
    .unwrap();
    assert_eq!(report.auth.value(), Some(&AuthState::Unauthenticated));
    assert!(report.billing.value().is_none());
    let model = model("t3/simple", dir.path()).await;
    let opened = open_spec(
        spec("t3/simple", &dir.path().join("s.jsonl"), "logged_out"),
        config(model, dir.path(), &Options::default()),
    )
    .await;
    assert!(matches!(opened, Err(e) if e.code == ErrorCode::AuthRequired));
}

#[tokio::test]
async fn an_unknown_envelope_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let report = probe_spec(
        spec("t3/simple", &dir.path().join("p.jsonl"), "bad_schema"),
        "c",
        "r",
    )
    .await;
    assert!(matches!(report, Err(e) if e.code == ErrorCode::ProtocolIncompatible));
}

#[tokio::test]
async fn a_changed_catalog_revision_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let mut model = model("t3/simple", dir.path()).await;
    model.revision = "catalog_old".into();
    let opened = open_spec(
        spec("t3/simple", &dir.path().join("s.jsonl"), "normal"),
        config(model, dir.path(), &Options::default()),
    )
    .await;
    assert!(matches!(opened, Err(e) if e.code == ErrorCode::SelectionStale));
}

#[tokio::test]
async fn a_read_only_turn_streams_text_and_usage_and_starts_a_denying_session() {
    let h = setup("t3/simple", Options::default()).await;
    let events = h.turn("hello", "1", &[]).await;
    assert_eq!(text(&events), "fixture simple ok");
    assert_eq!(final_status(&events), "completed");
    assert!(
        payloads(&events, "usage")
            .iter()
            .any(|u| u["unit"] == "output_tokens")
    );
    // Housekeeping reminder children and the echoed prompt are not output.
    assert!(payloads(&events, "tool").is_empty());
    assert!(payloads(&events, "extension").is_empty());
    let start = &h.sent_method("session/start")[0]["params"];
    assert_eq!(start["approvalMode"], "denyUnmatched");
    assert_eq!(start["providerId"], "meta");
    assert_eq!(start["modelId"], "muse-spark-1.3-contributor");
    assert!(start.get("config").is_none());
    assert_eq!(start["sessionId"].as_str().unwrap().len(), 36);
    let turn = &h.sent_method("turn/start")[0]["params"];
    assert_eq!(turn["ifBusy"], "queue");
    assert_eq!(turn["input"], json!([{"type":"text","text":"hello"}]));
    assert_eq!(turn["sessionId"], start["sessionId"]);
    h.runtime.close("owner", &h.session.id).await.unwrap();
}

#[tokio::test]
async fn consecutive_turns_reuse_the_native_session() {
    let h = setup("t3/multi_turn", Options::default()).await;
    let first = h.turn("one", "1", &[]).await;
    assert_eq!(text(&first), "first fixture turn complete");
    let second = h.turn("two", "2", &[]).await;
    assert_eq!(text(&second), "second fixture turn complete");
    assert_eq!(final_status(&second), "completed");
    assert_eq!(h.sent_method("session/start").len(), 1);
    // Not persistent: there is nothing to resume.
    assert!(matches!(
        h.runtime.suspend("owner", &h.session.id).await,
        Err(e) if e.code == ErrorCode::Conflict || e.code == ErrorCode::CapabilityUnsupported
    ));
}

#[tokio::test]
async fn cancellation_interrupts_the_owned_muse_turn() {
    let h = setup("t3/turn_interrupt", Options::default()).await;
    let turn = h
        .runtime
        .start("owner", &h.session.id, "slow".into(), "1".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.runtime
        .cancel("owner", turn["turnId"].as_str().unwrap())
        .await
        .unwrap();
    h.wait().await;
    let events = h.events();
    assert_eq!(final_status(&events), "cancelled");
    let started = &h.sent_method("turn/start")[0]["params"]["commandId"];
    let interrupt = &h.sent_method("turn/interrupt")[0]["params"];
    assert_eq!(&interrupt["turnId"], started);
    assert_eq!(interrupt["commandId"].as_str().unwrap().len(), 36);
}

#[tokio::test]
async fn supervised_approvals_reach_the_host_and_map_to_once_choices() {
    let h = setup(
        "t3/permission",
        Options {
            posture: Posture::Workspace,
            review: true,
            ..Default::default()
        },
    )
    .await;
    assert_eq!(
        h.sent_method("session/start")[0]["params"]["approvalMode"],
        "promptUnmatched"
    );
    let first = h.turn("touch approved", "1", &[true]).await;
    assert_eq!(text(&first), "DONE");
    let request = payloads(&first, "approval_request")[0];
    assert_eq!(request["action"], "Muse wants to run a command");
    assert!(
        request["detail"]
            .as_str()
            .unwrap()
            .starts_with("touch muse-approved.txt")
    );
    assert!(payloads(&first, "tool").iter().any(|t| t["name"] == "bash"));
    let second = h.turn("touch declined", "2", &[false]).await;
    assert_eq!(text(&second), "REJECTED");
    let decisions: Vec<_> = h
        .sent_method("approval/decide")
        .into_iter()
        .map(|d| d["params"]["choiceId"].as_str().unwrap().to_owned())
        .collect();
    // Muse shell approvals offer no `denied` choice; a denial is `abort`.
    assert_eq!(decisions, ["allow_once", "abort"]);
    let decide = &h.sent_method("approval/decide")[0]["params"];
    assert_eq!(decide["requirementId"]["sourceIndex"], 0);
    // `approval/request` and its `approval/requested` twin are one approval.
    assert_eq!(payloads(&first, "approval_request").len(), 1);
}

#[tokio::test]
async fn unreviewed_read_only_sessions_decline_requests_and_refuse_the_shell() {
    let h = setup("t3/permission", Options::default()).await;
    let events = h.turn("touch approved", "1", &[]).await;
    assert!(payloads(&events, "approval_request").is_empty());
    assert!(
        payloads(&events, "warning")
            .iter()
            .any(|w| w["code"] == "NATIVE_REQUEST_DECLINED")
    );
    assert_eq!(
        h.sent_method("approval/decide")[0]["params"]["choiceId"],
        "abort"
    );
    assert_eq!(error_code(&events), "INTEGRATION_RESTRICTED");
    assert_eq!(final_status(&events), "error");
}

#[tokio::test]
async fn native_subagents_are_reported_with_their_results() {
    let h = setup("t3/subagent_v2", Options::default()).await;
    let events = h.turn("spawn", "1", &[]).await;
    assert_eq!(final_status(&events), "completed");
    let tools: Vec<_> = payloads(&events, "tool")
        .into_iter()
        .map(|t| {
            format!(
                "{}:{}",
                t["name"].as_str().unwrap(),
                t["status"].as_str().unwrap()
            )
        })
        .collect();
    assert!(tools.contains(&"subagent_spawn:started".to_owned()));
    assert!(tools.contains(&"subagent_spawn:completed".to_owned()));
    let subagents: Vec<_> = payloads(&events, "extension")
        .into_iter()
        .filter(|e| e["namespace"] == "muse.subagent")
        .collect();
    assert!(!subagents.is_empty());
    let last = &subagents.last().unwrap()["data"];
    assert_eq!(last["role"], "hello agent");
    assert_eq!(last["objective"], "Reply with exactly: Hello");
    assert_eq!(last["result"], "Hello");
    assert!(text(&events).contains("Hello"));
}

#[tokio::test]
async fn a_subagent_approval_is_reviewed_by_the_host() {
    let h = setup(
        "t3/subagent_approval",
        Options {
            posture: Posture::Workspace,
            review: true,
            ..Default::default()
        },
    )
    .await;
    let events = h.turn("spawn and approve", "1", &[true]).await;
    let request = payloads(&events, "approval_request")[0];
    assert_eq!(request["action"], "A Muse subagent wants to run a command");
    assert_eq!(
        h.sent_method("approval/decide")[0]["params"]["choiceId"],
        "allow_once"
    );
    assert_eq!(final_status(&events), "completed");
}

#[tokio::test]
async fn cancelling_during_a_subagent_interrupts_the_parent_turn() {
    let h = setup(
        "t3/subagent_interrupt",
        Options {
            posture: Posture::Workspace,
            full_access: true,
            ..Default::default()
        },
    )
    .await;
    assert_eq!(
        h.sent_method("session/start")[0]["params"]["approvalMode"],
        "allowAll"
    );
    let turn = h
        .runtime
        .start("owner", &h.session.id, "spawn slow".into(), "1".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    h.runtime
        .cancel("owner", turn["turnId"].as_str().unwrap())
        .await
        .unwrap();
    h.wait().await;
    assert_eq!(final_status(&h.events()), "cancelled");
    assert_eq!(
        h.sent_method("turn/interrupt")[0]["params"]["turnId"],
        h.sent_method("turn/start")[0]["params"]["commandId"]
    );
}

#[tokio::test]
async fn a_workflow_and_the_report_turn_muse_starts_for_it_are_one_kit_turn() {
    let h = setup(
        "t3/workflow",
        Options {
            posture: Posture::Workspace,
            full_access: true,
            ..Default::default()
        },
    )
    .await;
    let events = h.turn("run a workflow", "1", &[]).await;
    assert_eq!(payloads(&events, "final").len(), 1);
    assert_eq!(final_status(&events), "completed");
    assert!(text(&events).contains("The workflow with `ping` and `pong` agents is complete."));
    let workflow: Vec<_> = payloads(&events, "extension")
        .into_iter()
        .filter(|e| e["namespace"] == "muse.workflow")
        .map(|e| &e["data"])
        .collect();
    let last = workflow.last().unwrap();
    assert_eq!(last["status"], "completed");
    assert_eq!(last["entryId"], "ping-pong");
    let labels: Vec<_> = last["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["label"].as_str().unwrap_or(""))
        .collect();
    // Later revisions drop the labels; the kit keeps the first ones seen.
    assert_eq!(labels, ["ping", "pong"]);
    assert!(
        last["children"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["terminal"] == "completed")
    );
    // Identical re-emissions are coalesced.
    assert!(workflow.len() < 14);
}

fn host_server() -> HostMcpServer {
    HostMcpServer {
        name: "design".into(),
        url: "http://127.0.0.1:9/mcp".into(),
        bearer_token: "k".repeat(40),
        allowed_tools: vec!["set_fill".into()],
    }
}

#[tokio::test]
async fn host_mcp_servers_attach_as_optional_session_servers() {
    let h = setup(
        "echo_host_mcp",
        Options {
            host: vec![host_server()],
            ..Default::default()
        },
    )
    .await;
    let events = h
        .turn("Use the design tools if you have them.", "1", &[])
        .await;
    assert_eq!(
        text(&events),
        "echo: Use the design tools if you have them."
    );
    assert_eq!(final_status(&events), "completed");
    let init = &h.sent_method("initialize")[0]["params"]["capabilities"];
    assert_eq!(init["requestedCapabilities"], json!(["sessionMcp"]));
    let start = &h.sent_method("session/start")[0]["params"];
    assert_eq!(
        start["config"],
        json!({"mcpServers":{"design":{
            "transport":"streamableHttp",
            "url":"http://127.0.0.1:9/mcp",
            "headers":{"Authorization":format!("Bearer {}", "k".repeat(40))},
            "mode":"optional"
        }}})
    );
    assert!(
        !serde_json::to_string(&h.events())
            .unwrap()
            .contains(&"k".repeat(40))
    );
}

#[tokio::test]
async fn host_mcp_needs_the_session_mcp_capability() {
    let (opened, _) = try_setup(
        "echo_host_mcp",
        Options {
            host: vec![host_server()],
            mode: "no_session_mcp",
            ..Default::default()
        },
    )
    .await;
    assert!(matches!(opened, Err(e) if e.code == ErrorCode::CapabilityUnsupported));
}

#[tokio::test]
async fn host_tool_calls_are_neutral_tool_events() {
    let h = setup(
        "synthetic/host_tool",
        Options {
            host: vec![host_server()],
            ..Default::default()
        },
    )
    .await;
    let events = h.turn("fill", "1", &[]).await;
    let tools: Vec<_> = payloads(&events, "tool")
        .into_iter()
        .map(|t| (t["name"].as_str().unwrap(), t["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        tools,
        [
            ("mcp__design__set_fill", "started"),
            ("mcp__design__set_fill", "completed")
        ]
    );
    assert_eq!(text(&events), "Filled with the brand red.");
    assert_eq!(final_status(&events), "completed");
}

#[tokio::test]
async fn another_mcp_servers_tool_fails_the_turn() {
    let h = setup(
        "synthetic/foreign_mcp_tool",
        Options {
            host: vec![host_server()],
            ..Default::default()
        },
    )
    .await;
    let events = h.turn("go", "1", &[]).await;
    assert_eq!(error_code(&events), "INTEGRATION_RESTRICTED");
    assert!(!text(&events).contains("must not reach"));
}

#[tokio::test]
async fn read_only_sessions_allow_muse_read_tools_but_not_the_shell() {
    let h = setup("synthetic/read_tools", Options::default()).await;
    let events = h.turn("look", "1", &[]).await;
    assert!(
        payloads(&events, "tool")
            .iter()
            .any(|t| t["name"] == "read_file" && t["status"] == "completed")
    );
    assert!(
        !payloads(&events, "tool")
            .iter()
            .any(|t| t["name"] == "bash")
    );
    assert_eq!(error_code(&events), "INTEGRATION_RESTRICTED");
}

#[tokio::test]
async fn web_hits_reasoning_summaries_todos_and_retries_are_presented() {
    let h = setup(
        "synthetic/web_and_reasoning",
        Options {
            posture: Posture::Workspace,
            full_access: true,
            ..Default::default()
        },
    )
    .await;
    let events = h.turn("node lts?", "1", &[]).await;
    assert_eq!(final_status(&events), "completed");
    let reasoning: Vec<_> = payloads(&events, "reasoning_summary")
        .into_iter()
        .map(|r| r["text"].as_str().unwrap())
        .collect();
    assert_eq!(reasoning, ["Checking the current Node release."]);
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("raw private reasoning")
    );
    let extension = |namespace: &str| -> Vec<Value> {
        payloads(&events, "extension")
            .into_iter()
            .filter(|e| e["namespace"] == namespace)
            .map(|e| e["data"].clone())
            .collect()
    };
    let todo = &extension("muse.todo")[0];
    assert_eq!(
        todo["items"][1],
        json!({"text":"Answer","status":"inProgress"})
    );
    let web = extension("muse.web_search");
    assert_eq!(web[0]["query"], "node lts");
    assert_eq!(
        web[0]["results"],
        json!([
            {"url":"https://nodejs.org/","title":"Node.js","snippet":"Node 24 is LTS"},
            {"snippet":"No link"}
        ])
    );
    assert_eq!(
        web[1]["results"],
        json!([{"url":"https://nodejs.org/en/about","snippet":"About Node.js"}])
    );
    assert!(
        payloads(&events, "warning")
            .iter()
            .any(|w| w["code"] == "PROVIDER_RETRY"
                && w["message"].as_str().unwrap().contains("attempt 2 of 3"))
    );
    assert_eq!(text(&events), "Node 24 is the current LTS.");
}

#[tokio::test]
async fn structured_questions_are_declined_and_the_turn_continues() {
    let h = setup("synthetic/question", Options::default()).await;
    let events = h.turn("ask me", "1", &[]).await;
    assert_eq!(final_status(&events), "completed");
    assert_eq!(text(&events), "Continuing without an answer.");
    let cancel = &h.sent_method("userInput/cancel")[0]["params"];
    assert_eq!(
        cancel["userInputId"],
        "01a13400-0000-7000-8000-000000000020"
    );
    assert!(
        payloads(&events, "warning")
            .iter()
            .any(|w| w["code"] == "NATIVE_QUESTION_DECLINED")
    );
}

#[tokio::test]
async fn a_signed_out_turn_is_auth_required_without_leaking_login_codes() {
    let h = setup("synthetic/auth_required", Options::default()).await;
    let events = h.turn("hi", "1", &[]).await;
    assert_eq!(error_code(&events), "AUTH_REQUIRED");
    let message = payloads(&events, "error")[0]["message"].as_str().unwrap();
    assert!(message.contains("muse login"));
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("ABCD-1234")
    );
}

#[tokio::test]
async fn an_inaccessible_model_surfaces_muses_access_error() {
    let h = setup("synthetic/model_access", Options::default()).await;
    let events = h.turn("hi", "1", &[]).await;
    assert_eq!(error_code(&events), "PROVIDER_UNAVAILABLE");
    let message = payloads(&events, "error")[0]["message"].as_str().unwrap();
    assert!(
        message.contains("does not exist or you lack access"),
        "{message}"
    );
    assert!(message.contains("modelError"));
}

#[tokio::test]
async fn effort_and_images_reach_turn_start() {
    let h = setup("echo_multi_turn", Options::default()).await;
    h.runtime
        .update_options(
            "owner",
            &h.session.id,
            BTreeMap::from([("reasoningEffort".into(), json!("medium"))]),
        )
        .unwrap();
    let image = ImageInput {
        media_type: InputMediaType::Png,
        bytes: Arc::from(&b"\x89PNG fixture"[..]),
        width: 2,
        height: 3,
    };
    h.runtime
        .start_with_inputs(
            "owner",
            &h.session.id,
            "look".into(),
            "1".into(),
            vec![image],
        )
        .await
        .unwrap();
    h.wait().await;
    let turn = &h.sent_method("turn/start")[0]["params"];
    assert_eq!(turn["input"][0], json!({"type":"text","text":"look"}));
    assert_eq!(
        turn["input"][1],
        json!({"type":"image","base64Data":"iVBORyBmaXh0dXJl","mediaType":"image/png","width":2,"height":3})
    );
    assert_eq!(turn["reasoningEffort"], "medium");
    assert!(
        !serde_json::to_string(&h.events())
            .unwrap()
            .contains("iVBORyBmaXh0dXJl")
    );
}

#[tokio::test]
async fn persistent_sessions_resume_with_their_approval_mode() {
    let dir = tempfile::tempdir().unwrap();
    let model = model("echo_multi_turn", dir.path()).await;
    let log = dir.path().join("resume.jsonl");
    let options = Options {
        persistent: true,
        resume: Some("01a13398-2865-79d3-95d5-15fefc777824".into()),
        ..Default::default()
    };
    let agent = open_spec(
        spec("echo_multi_turn", &log, "normal"),
        config(model.clone(), dir.path(), &options),
    )
    .await
    .unwrap();
    assert!(agent.native_resumption());
    let sent: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let resume = sent
        .iter()
        .find(|m| m["method"] == "session/resume")
        .unwrap();
    assert_eq!(
        resume["params"]["sessionId"],
        "01a13398-2865-79d3-95d5-15fefc777824"
    );
    assert_eq!(resume["params"]["excludeItems"], true);
    let mode = sent
        .iter()
        .find(|m| m["method"] == "session/setApprovalMode")
        .unwrap();
    assert_eq!(mode["params"]["mode"], "denyUnmatched");
    assert!(sent.iter().all(|m| m["method"] != "session/start"));
    let missing = open_spec(
        spec(
            "echo_multi_turn",
            &dir.path().join("missing.jsonl"),
            "session_missing",
        ),
        config(model.clone(), dir.path(), &options),
    )
    .await;
    assert!(matches!(missing, Err(e) if e.code == ErrorCode::ResumeUnavailable));
    let invalid = open_spec(
        spec(
            "echo_multi_turn",
            &dir.path().join("invalid.jsonl"),
            "normal",
        ),
        config(
            model,
            dir.path(),
            &Options {
                persistent: true,
                resume: Some("../not-a-session".into()),
                ..Default::default()
            },
        ),
    )
    .await;
    assert!(matches!(invalid, Err(e) if e.code == ErrorCode::ResumeUnavailable));
}

#[tokio::test]
async fn closing_one_session_leaves_another_running() {
    let a = setup("echo_multi_turn", Options::default()).await;
    let b = setup("echo_multi_turn", Options::default()).await;
    a.runtime.close("owner", &a.session.id).await.unwrap();
    let events = b.turn("first kit turn", "1", &[]).await;
    assert_eq!(text(&events), "echo: first kit turn");
    assert_eq!(final_status(&events), "completed");
}

#[tokio::test]
async fn a_stream_that_ends_mid_reply_fails_the_turn() {
    let h = setup(
        "synthetic/truncated",
        Options {
            mode: "exit_at_end",
            ..Default::default()
        },
    )
    .await;
    let events = h.turn("hi", "1", &[]).await;
    assert_eq!(text(&events), "partial repl");
    assert_eq!(error_code(&events), "PROVIDER_UNAVAILABLE");
    assert_eq!(final_status(&events), "error");
}
