//! Persistent native conversations: each adapter keeps history on suspend and
//! reattaches the same conversation instead of starting a new one.
use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, post},
};
use originlm_adapters::{
    claude::{self, client as claude_client},
    execution::{
        CodexExecution, ManagedCredential, ManagedCredentialSource, open_codex, open_managed_codex,
    },
    host_mcp::HostTools,
    http::{Authentication, Endpoint},
    opencode::client as opencode,
    process::LaunchSpec,
};
use originlm_contracts::*;
use originlm_runtime::{Agent, Runtime};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio_stream::StreamExt;

fn python() -> PathBuf {
    std::env::var_os("LAI_TEST_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "C:/Python313/python.exe".into()
            } else {
                "/usr/bin/python3".into()
            }
        })
}
fn peer(script: &str, mode: &str) -> LaunchSpec {
    LaunchSpec {
        executable: python(),
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support")
                .join(script)
                .into(),
            mode.into(),
        ],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}
/// Runs one turn and returns the runtime, session id and final state.
async fn one_turn(agent: Box<dyn Agent>, model: &Model, options: Value) -> (Runtime, String) {
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: model.route_id.clone(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: serde_json::from_value(options).unwrap(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("owner", &session.id, "hello".into(), "1".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.get("owner", &session.id).unwrap().state == SessionState::Running {
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        runtime.get("owner", &session.id).unwrap().state,
        SessionState::Completed,
        "{}",
        runtime.events("owner", None, None).unwrap()
    );
    (runtime, session.id)
}

// ---- Codex app-server

fn codex_config(persistent: bool, resume: Option<&str>) -> CodexExecution {
    let model = json!({"id":"native-id","model":"test-native","displayName":"Test native","description":"Fixture","hidden":false,"isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"future","description":"Future"}],"defaultReasoningEffort":"future","inputModalities":["text"]});
    let report = originlm_adapters::codex::parse_metadata(
        "c",
        "r",
        &json!({"userAgent":"codex/0.156.1"}),
        &json!({"account":{"type":"chatgpt","email":"test@example.test","planType":"test"},"requiresOpenaiAuth":true}),
        &[model],
        Some(&json!({"rateLimits":{}})),
    )
    .unwrap();
    CodexExecution {
        model: report.models[0].clone(),
        connection: "c".into(),
        route: "r".into(),
        writable: false,
        review_requests: false,
        native_policy: false,
        host_mcp_servers: vec![],
        persistent,
        resume_thread: resume.map(str::to_owned),
    }
}
#[tokio::test]
async fn codex_persistent_thread_suspends_to_its_native_id() {
    let config = codex_config(true, None);
    let model = config.model.clone();
    let agent = open_codex(peer("execution_peer.py", "persist"), config)
        .await
        .unwrap();
    assert!(agent.native_resumption());
    let (runtime, session) = one_turn(agent, &model, json!({"reasoningEffort":"future"})).await;
    assert_eq!(
        runtime.suspend("owner", &session).await.unwrap(),
        "native-thread"
    );
}

#[tokio::test]
async fn managed_codex_rotated_token_restarts_and_resumes_same_thread() {
    let tmp = tempfile::tempdir().unwrap();
    let identity = originlm_adapters::codex::ManagedIdentity {
        app_name: "example-app".into(),
        subject: "subject".into(),
        client_id: "oaiapp_client".into(),
        email: Some("person@example.test".into()),
    };
    let model_json = json!({"model":"test-native","displayName":"Test native","hidden":false,"isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"future","description":"Future"}],"defaultReasoningEffort":"future","inputModalities":["text"]});
    let report = originlm_adapters::codex::parse_managed_metadata(
        "c",
        "r",
        &json!({"userAgent":"codex/0.157.0"}),
        &identity,
        &[model_json],
    )
    .unwrap();
    let model = report.models[0].clone();
    let mut config = codex_config(true, None);
    config.model = model.clone();
    let mut spec = peer("execution_peer.py", "managed");
    spec.env.insert("ACCESS_TOKEN".into(), "token-old".into());
    let log = tmp.path().join("managed.log");
    spec.env
        .insert("TEST_METHODS_LOG".into(), log.clone().into_os_string());
    let calls = Arc::new(AtomicUsize::new(0));
    let source: ManagedCredentialSource = Arc::new(move || {
        let token = if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            "token-old"
        } else {
            "token-new"
        };
        Box::pin(async move {
            Ok(ManagedCredential {
                access_token: token.into(),
                subject: "subject".into(),
                client_id: "oaiapp_client".into(),
                email: Some("person@example.test".into()),
            })
        })
    });
    let agent = open_managed_codex(spec, config, identity, source)
        .await
        .unwrap();
    let (runtime, session) = one_turn(agent, &model, json!({"reasoningEffort":"future"})).await;
    runtime
        .start("owner", &session, "again".into(), "2".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.get("owner", &session).unwrap().state == SessionState::Running {
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        runtime.get("owner", &session).unwrap().state,
        SessionState::Completed
    );
    let log = std::fs::read_to_string(log).unwrap();
    assert!(log.contains("process initial\n"));
    assert!(log.contains("process rotated\n"));
    assert!(log.contains("thread/resume\n"));
    assert!(!log.contains("token-old"));
    assert!(!log.contains("token-new"));
}
#[tokio::test]
async fn managed_codex_changed_identity_rejects_turn_before_native_thread_start() {
    let tmp = tempfile::tempdir().unwrap();
    let identity = originlm_adapters::codex::ManagedIdentity {
        app_name: "example-app".into(),
        subject: "subject".into(),
        client_id: "oaiapp_client".into(),
        email: None,
    };
    let model_json = json!({"model":"test-native","displayName":"Test native","hidden":false,"isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"future","description":"Future"}],"defaultReasoningEffort":"future","inputModalities":["text"]});
    let report = originlm_adapters::codex::parse_managed_metadata(
        "c",
        "r",
        &json!({"userAgent":"codex/0.157.0"}),
        &identity,
        &[model_json],
    )
    .unwrap();
    let model = report.models[0].clone();
    let mut config = codex_config(true, None);
    config.model = model.clone();
    let mut spec = peer("execution_peer.py", "managed");
    spec.env.insert("ACCESS_TOKEN".into(), "token-old".into());
    let log = tmp.path().join("changed-identity.log");
    spec.env
        .insert("TEST_METHODS_LOG".into(), log.clone().into_os_string());
    let source: ManagedCredentialSource = Arc::new(|| {
        Box::pin(async {
            Ok(ManagedCredential {
                access_token: "token-new".into(),
                subject: "different-subject".into(),
                client_id: "oaiapp_client".into(),
                email: None,
            })
        })
    });
    let agent = open_managed_codex(spec, config, identity, source)
        .await
        .unwrap();
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: model.route_id.clone(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: serde_json::from_value(json!({"reasoningEffort":"future"})).unwrap(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("owner", &session.id, "hello".into(), "1".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.get("owner", &session.id).unwrap().state == SessionState::Running {
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        runtime.get("owner", &session.id).unwrap().state,
        SessionState::Error
    );
    let log = std::fs::read_to_string(log).unwrap();
    assert!(!log.contains("thread/start"));
    assert!(!log.contains("turn/start"));
    assert!(!log.contains("token-new"));
}
#[tokio::test]
async fn codex_ephemeral_threads_stay_unresumable() {
    let config = codex_config(false, None);
    let agent = open_codex(peer("execution_peer.py", "normal"), config)
        .await
        .unwrap();
    assert!(!agent.native_resumption());
}
#[tokio::test]
async fn codex_resume_reattaches_the_thread_without_starting_one() {
    // The fixture fails any thread/start in resume modes.
    let config = codex_config(true, Some("native-thread"));
    let model = config.model.clone();
    let agent = open_codex(peer("execution_peer.py", "resume"), config)
        .await
        .unwrap();
    let (runtime, session) = one_turn(agent, &model, json!({"reasoningEffort":"future"})).await;
    let events = runtime.events("owner", None, None).unwrap().to_string();
    assert!(events.contains("native reply"));
    assert_eq!(
        runtime.suspend("owner", &session).await.unwrap(),
        "native-thread"
    );
}
#[tokio::test]
async fn codex_missing_or_different_thread_is_resume_unavailable() {
    for mode in ["resume_missing", "resume_other"] {
        let error = match open_codex(
            peer("execution_peer.py", mode),
            codex_config(true, Some("native-thread")),
        )
        .await
        {
            Ok(_) => panic!("{mode} reattached"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::ResumeUnavailable, "{mode}");
        assert!(!error.message.contains("native-thread"));
    }
}

// ---- Claude Code

fn fake_claude() -> (tempfile::TempDir, originlm_discovery::LocatedClient) {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join(if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    });
    std::fs::write(&exe, b"").unwrap();
    let located = originlm_discovery::LocatedClient {
        executable_identity: None,
        provider_id: "claude_code".into(),
        profile_home: dir.path().into(),
        executable: Some(exe),
        scope: None,
    };
    (dir, located)
}
#[test]
fn claude_persistence_replaces_no_persistence_with_session_or_resume() {
    let (_home, located) = fake_claude();
    let id = claude::launch::new_session_id().unwrap();
    assert!(claude::launch::valid_session_id(&id));
    assert_ne!(id, claude::launch::new_session_id().unwrap());
    let args = |resume: bool| {
        let mut spec = claude::launch::launch(&located, std::env::temp_dir()).unwrap();
        claude::launch::persist(&mut spec, &id, resume).unwrap();
        spec.args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    let fresh = args(false);
    assert!(!fresh.contains(&"--no-session-persistence".into()));
    let at = fresh.iter().position(|a| a == "--session-id").unwrap();
    assert_eq!(fresh[at + 1], id);
    assert!(!fresh.contains(&"--resume".into()));
    assert!(!fresh.contains(&"--fork-session".into()));
    let resumed = args(true);
    let at = resumed.iter().position(|a| a == "--resume").unwrap();
    assert_eq!(resumed[at + 1], id);
    assert!(!resumed.contains(&"--session-id".into()));
    // Text-only restrictions are unchanged.
    for kept in [
        "--tools",
        "--strict-mcp-config",
        "--restricted",
        "--safe-mode",
    ] {
        assert!(resumed.contains(&kept.into()), "{kept}");
    }
    let mut spec = claude::launch::launch(&located, std::env::temp_dir()).unwrap();
    for bad in ["not-a-uuid", "--fork-session", &id.to_uppercase()] {
        assert!(claude::launch::persist(&mut spec, bad, true).is_err());
    }
}
#[test]
fn claude_resume_requires_the_stored_conversation() {
    let (home, _) = fake_claude();
    let id = claude::launch::new_session_id().unwrap();
    assert!(!claude::launch::persisted_session_exists(home.path(), &id));
    let project = home.path().join("projects").join("C--work-scratch");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join(format!("{id}.jsonl")), b"{}\n").unwrap();
    assert!(claude::launch::persisted_session_exists(home.path(), &id));
    assert!(!claude::launch::persisted_session_exists(
        home.path(),
        "../../etc/passwd"
    ));
}
async fn claude_agent(mode: &str, id: &str, resumed: bool) -> (Box<dyn Agent>, Model) {
    let report = claude_client::probe_spec(peer("claude_peer.py", "normal"), "c", "r")
        .await
        .unwrap();
    let model = report.models[0].clone();
    let mut spec = peer("claude_peer.py", mode);
    claude::launch::persist(&mut spec, id, resumed).unwrap();
    let agent = claude_client::persistent_executor(
        claude::launch::HostMcpLaunch {
            spec,
            config_dir: None,
        },
        HostTools::default(),
        model.clone(),
        "c".into(),
        "r".into(),
        id.into(),
        resumed,
    );
    (agent, model)
}
#[tokio::test]
async fn claude_persistent_session_suspends_to_its_id_after_a_turn() {
    let id = claude::launch::new_session_id().unwrap();
    let (mut early, _) = claude_agent("persist", &id, false).await;
    assert!(early.native_resumption());
    // No history yet: nothing to resume.
    assert_eq!(early.suspend().await.unwrap_err().code, ErrorCode::Conflict);
    let (agent, model) = claude_agent("persist", &id, false).await;
    let (runtime, session) =
        one_turn(agent, &model, json!({"reasoningEffort":"future-effort"})).await;
    assert_eq!(runtime.suspend("owner", &session).await.unwrap(), id);
}
#[tokio::test]
async fn claude_resumed_session_keeps_the_same_id() {
    let id = claude::launch::new_session_id().unwrap();
    let (agent, model) = claude_agent("persist_resume", &id, true).await;
    // The fixture reports the resumed id; a different id would fail the turn.
    let (runtime, session) =
        one_turn(agent, &model, json!({"reasoningEffort":"future-effort"})).await;
    assert_eq!(runtime.suspend("owner", &session).await.unwrap(), id);
}

// ---- OpenCode

#[derive(Clone)]
struct Mock {
    calls: Arc<AtomicUsize>,
    deleted: Arc<AtomicUsize>,
    sender: tokio::sync::broadcast::Sender<Value>,
}
fn deny() -> Value {
    json!([{"permission":"*","pattern":"*","action":"deny"}])
}
async fn opencode_server() -> (Endpoint, Mock, tokio::task::JoinHandle<()>) {
    let (sender, _) = tokio::sync::broadcast::channel(32);
    let mock = Mock {
        calls: Arc::default(),
        deleted: Arc::default(),
        sender,
    };
    let message = |n: usize| json!({"info":{"id":format!("msg_{n}"),"role":"assistant","modelID":"native","providerID":"upstream","time":{"completed":123},"tokens":{"input":2,"output":1}},"parts":[{"type":"text","text":format!("reply {n}")}]});
    let app = Router::new()
        .route(
            "/session",
            post(|| async { Json(json!({"id":"ses_kept","permission":deny()})) }),
        )
        .route(
            "/session/ses_kept",
            get(|| async { Json(json!({"id":"ses_kept","permission":deny()})) }).delete(
                |State(m): State<Mock>| async move {
                    m.deleted.fetch_add(1, Ordering::SeqCst);
                    Json(true)
                },
            ),
        )
        .route(
            "/event",
            get(|State(m): State<Mock>| async move {
                let stream = tokio_stream::wrappers::BroadcastStream::new(m.sender.subscribe())
                    .filter_map(|v| {
                        v.ok()
                            .map(|v| Ok::<_, Infallible>(Event::default().data(v.to_string())))
                    });
                Sse::new(stream)
            }),
        )
        .route(
            "/session/ses_kept/prompt_async",
            post(|State(m): State<Mock>| async move {
                let n = m.calls.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = m.sender.send(json!({"type":"message.updated","properties":{"info":{"id":format!("msg_{n}"),"role":"assistant","sessionID":"ses_kept","modelID":"native","providerID":"upstream"}}}));
                let _ = m.sender.send(json!({"type":"session.status","properties":{"sessionID":"ses_kept","status":{"type":"idle"}}}));
                axum::http::StatusCode::NO_CONTENT
            }),
        )
        .route(
            "/session/ses_kept/message",
            get(move |State(m): State<Mock>| async move {
                Json(json!([message(m.calls.load(Ordering::SeqCst))]))
            }),
        )
        .route("/session/ses_kept/abort", post(|| async { Json(true) }))
        .with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (
        Endpoint::new(
            &format!("http://127.0.0.1:{port}/"),
            Authentication::None,
            true,
        )
        .unwrap(),
        mock,
        task,
    )
}
fn opencode_model() -> Model {
    Model {
        id: "m".into(),
        route_id: "r".into(),
        native_id: "native".into(),
        name: "Native".into(),
        revision: "v".into(),
        is_default: true,
        capabilities: vec![],
        options: vec![],
        context_window: Observation::unknown("fixture"),
    }
}
#[tokio::test]
async fn opencode_suspend_keeps_the_session_and_resume_reattaches_it() {
    let (endpoint, mock, task) = opencode_server().await;
    let dir = tempfile::tempdir().unwrap();
    let agent = opencode::open_session(
        endpoint.clone(),
        opencode_model(),
        "upstream".into(),
        dir.path().into(),
        true,
        None,
    )
    .await
    .unwrap();
    assert!(agent.native_resumption());
    let (runtime, session) = one_turn(agent, &opencode_model(), json!({})).await;
    assert_eq!(
        runtime.suspend("owner", &session).await.unwrap(),
        "ses_kept"
    );
    assert_eq!(mock.deleted.load(Ordering::SeqCst), 0, "suspend kept it");
    let agent = opencode::open_session(
        endpoint.clone(),
        opencode_model(),
        "upstream".into(),
        dir.path().into(),
        true,
        Some("ses_kept".into()),
    )
    .await
    .unwrap();
    // The resumed turn must return the new reply, not the stored one.
    let (runtime, session) = one_turn(agent, &opencode_model(), json!({})).await;
    let events = runtime.events("owner", None, None).unwrap().to_string();
    assert!(events.contains("reply 2"), "{events}");
    runtime.close("owner", &session).await.unwrap();
    assert_eq!(mock.deleted.load(Ordering::SeqCst), 1);
    let error = match opencode::open_session(
        endpoint,
        opencode_model(),
        "upstream".into(),
        dir.path().into(),
        true,
        Some("ses_missing".into()),
    )
    .await
    {
        Ok(_) => panic!("missing session reattached"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::ResumeUnavailable);
    task.abort();
}
#[tokio::test]
async fn opencode_non_persistent_sessions_are_not_resumable() {
    let (endpoint, _mock, task) = opencode_server().await;
    let dir = tempfile::tempdir().unwrap();
    let agent = opencode::open(
        endpoint,
        opencode_model(),
        "upstream".into(),
        dir.path().into(),
    )
    .await
    .unwrap();
    assert!(!agent.native_resumption());
    task.abort();
}
