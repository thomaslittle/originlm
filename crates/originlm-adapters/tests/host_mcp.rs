//! Host MCP servers: per-provider launch wiring and tool-event enforcement.
use originlm_adapters::{
    acp::client as acp,
    claude::{self, client as claude_client},
    execution::{CodexExecution, open_codex},
    host_mcp::HostTools,
    process::LaunchSpec,
};
use originlm_contracts::*;
use originlm_runtime::{Agent, Runtime};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

fn token() -> String {
    "k".repeat(40)
}
fn servers() -> Vec<HostMcpServer> {
    vec![HostMcpServer {
        name: "design".into(),
        url: "http://127.0.0.1:48123/mcp".into(),
        bearer_token: token(),
        allowed_tools: vec!["echo_color".into()],
    }]
}
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
async fn run_turn(agent: Box<dyn Agent>, model: &Model, options: Value) -> (SessionState, String) {
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: serde_json::from_value(options).unwrap(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("owner", &session.id, "call echo_color".into(), "1".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while matches!(
            runtime.get("owner", &session.id).unwrap().state,
            SessionState::Running | SessionState::AwaitingApproval
        ) {
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .unwrap();
    let state = runtime.get("owner", &session.id).unwrap().state;
    let events = runtime.events("owner", None, None).unwrap().to_string();
    runtime.close("owner", &session.id).await.unwrap();
    (state, events)
}
fn tool_events(events: &str) -> Vec<(String, String)> {
    let events: Value = serde_json::from_str(events).unwrap();
    events["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["payload"]["type"] == "tool")
        .map(|e| {
            (
                e["payload"]["name"].as_str().unwrap().to_owned(),
                e["payload"]["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
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
    };
    (dir, located)
}
#[test]
fn claude_launch_writes_a_private_config_and_keeps_tools_disabled() {
    let (_home, located) = fake_claude();
    let launch =
        claude::launch::launch_with_host_mcp(&located, std::env::temp_dir(), &servers()).unwrap();
    let args: Vec<String> = launch
        .spec
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let tools = args.iter().position(|a| a == "--tools").unwrap();
    assert_eq!(args[tools + 1], "");
    assert!(args.contains(&"--strict-mcp-config".into()));
    assert!(args.contains(&"--restricted".into()));
    assert!(args.contains(&"--disable-slash-commands".into()));
    // Claude Code ignores --mcp-config servers in safe mode.
    assert!(!args.contains(&"--safe-mode".into()));
    assert!(args.contains(&"--allowedTools=mcp__design__echo_color".into()));
    let config = args.iter().position(|a| a == "--mcp-config").unwrap();
    let path = PathBuf::from(&args[config + 1]);
    let dir = launch.config_dir.as_ref().unwrap().path().to_path_buf();
    assert!(path.starts_with(&dir));
    // The token lives only in the private file: never argv or environment.
    assert!(!args.iter().any(|a| a.contains(&token())));
    assert!(
        !launch
            .spec
            .env
            .values()
            .any(|v| v.to_string_lossy().contains(&token()))
    );
    let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        written,
        json!({"mcpServers":{"design":{"type":"http","url":"http://127.0.0.1:48123/mcp","headers":{"Authorization":format!("Bearer {}", token())}}}})
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(launch);
    assert!(!path.exists() && !dir.exists());
}
#[test]
fn claude_launch_without_host_servers_is_unchanged() {
    let (_home, located) = fake_claude();
    let plain = claude::launch::launch(&located, std::env::temp_dir()).unwrap();
    let launch = claude::launch::launch_with_host_mcp(&located, std::env::temp_dir(), &[]).unwrap();
    assert_eq!(plain.args, launch.spec.args);
    assert!(launch.config_dir.is_none());
}
#[tokio::test]
async fn claude_session_deletes_the_config_on_close() {
    let (_home, located) = fake_claude();
    let launch =
        claude::launch::launch_with_host_mcp(&located, std::env::temp_dir(), &servers()).unwrap();
    let dir = launch.config_dir.as_ref().unwrap().path().to_path_buf();
    let report = claude_client::probe_spec(peer("claude_peer.py", "normal"), "c", "r")
        .await
        .unwrap();
    let mut agent = claude_client::host_executor(
        claude::launch::HostMcpLaunch {
            spec: peer("claude_peer.py", "normal"),
            config_dir: launch.config_dir,
        },
        HostTools::new(&servers()),
        report.models[0].clone(),
        "c".into(),
        "r".into(),
    );
    assert!(dir.exists());
    agent.close().await.unwrap();
    assert!(!dir.exists());
}
async fn claude_turn(mode: &str) -> (SessionState, String) {
    let report = claude_client::probe_spec(peer("claude_peer.py", "normal"), "c", "r")
        .await
        .unwrap();
    let model = report.models[0].clone();
    let agent = claude_client::host_executor(
        claude::launch::HostMcpLaunch {
            spec: peer("claude_peer.py", mode),
            config_dir: None,
        },
        HostTools::new(&servers()),
        model.clone(),
        "c".into(),
        "r".into(),
    );
    run_turn(agent, &model, json!({"reasoningEffort":"future-effort"})).await
}
#[tokio::test]
async fn claude_host_tool_is_allowed_and_reported_neutrally() {
    let (state, events) = claude_turn("host_tool").await;
    assert_eq!(state, SessionState::Completed, "{events}");
    assert_eq!(
        tool_events(&events),
        [
            ("mcp__design__echo_color".into(), "started".into()),
            ("mcp__design__echo_color".into(), "completed".into())
        ]
    );
    assert!(events.contains("color is red"));
    assert!(!events.contains(&token()));
}
#[tokio::test]
async fn claude_host_server_that_fails_to_connect_fails_the_session() {
    let (state, events) = claude_turn("mcp_failed").await;
    assert_eq!(state, SessionState::Error);
    assert!(events.contains("PROVIDER_UNAVAILABLE"), "{events}");
    assert!(
        claude_client::probe_host_mcp(
            peer("claude_peer.py", "mcp_failed"),
            &HostTools::new(&servers())
        )
        .await
        .is_err()
    );
    claude_client::probe_host_mcp(
        peer("claude_peer.py", "normal"),
        &HostTools::new(&servers()),
    )
    .await
    .unwrap();
}
#[tokio::test]
async fn claude_native_tool_still_fails_the_turn() {
    let (state, events) = claude_turn("native_tool_use").await;
    assert_eq!(state, SessionState::Error);
    assert!(events.contains("INTEGRATION_RESTRICTED"), "{events}");
    let (state, events) = claude_turn("advertise_native").await;
    assert_eq!(state, SessionState::Error);
    assert!(events.contains("INTEGRATION_RESTRICTED"), "{events}");
}
#[test]
fn claude_event_checks_distinguish_host_tools() {
    let host = HostTools::new(&servers());
    assert!(claude_client::advertised_tools_allowed(
        &host,
        &json!(["mcp__design__echo_color"])
    ));
    assert!(claude_client::advertised_tools_allowed(&host, &json!([])));
    assert!(!claude_client::advertised_tools_allowed(
        &host,
        &json!(["mcp__design__echo_color", "Read"])
    ));
    assert!(!claude_client::advertised_tools_allowed(
        &HostTools::default(),
        &json!(["mcp__design__echo_color"])
    ));
    let call = |name: &str| json!({"type":"tool_use","id":"t","name":name});
    assert_eq!(
        claude_client::host_tool_use(&host, &call("mcp__design__echo_color")).unwrap(),
        "t"
    );
    for refused in [
        "Bash",
        "Read",
        "WebFetch",
        "mcp__design__other",
        "mcp__x__echo_color",
    ] {
        assert_eq!(
            claude_client::host_tool_use(&host, &call(refused))
                .unwrap_err()
                .code,
            ErrorCode::IntegrationRestricted
        );
    }
    assert!(
        claude_client::host_tool_use(
            &host,
            &json!({"type":"server_tool_use","id":"t","name":"mcp__design__echo_color"})
        )
        .is_err()
    );
    assert!(claude_client::check_block_start(&host, &json!({"type":"text"})).is_ok());
    assert!(
        claude_client::check_block_start(&host, &json!({"type":"tool_use","name":"Bash"})).is_err()
    );
}

// ---- Codex app-server

fn codex_config(review: bool) -> CodexExecution {
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
        review_requests: review,
        native_policy: false,
        host_mcp_servers: servers(),
        persistent: false,
        resume_thread: None,
    }
}
async fn codex_turn(mode: &str) -> (SessionState, String) {
    let config = codex_config(false);
    let model = config.model.clone();
    let agent = open_codex(peer("execution_peer.py", mode), config)
        .await
        .unwrap();
    run_turn(agent, &model, json!({"reasoningEffort":"future"})).await
}
#[tokio::test]
async fn codex_thread_config_attaches_host_server_and_allows_its_tool() {
    // The fixture asserts the exact `mcp_servers.design` thread config and
    // that the already-configured `user_global` server is disabled.
    let (state, events) = codex_turn("host").await;
    assert_eq!(state, SessionState::Completed, "{events}");
    assert_eq!(
        tool_events(&events),
        [
            ("mcp__design__echo_color".into(), "started".into()),
            ("mcp__design__echo_color".into(), "completed".into())
        ]
    );
    assert!(!events.contains(&token()));
}
#[tokio::test]
async fn codex_non_host_tools_fail_a_text_only_turn() {
    for mode in ["host_other_server", "host_command", "host_web"] {
        let (state, events) = codex_turn(mode).await;
        assert_eq!(state, SessionState::Error, "{mode}: {events}");
        assert!(
            events.contains("INTEGRATION_RESTRICTED"),
            "{mode}: {events}"
        );
        assert!(tool_events(&events).is_empty(), "{mode}: {events}");
    }
}

#[tokio::test]
async fn codex_refuses_a_host_name_already_configured() {
    let (state, events) = codex_turn("host_collision").await;
    assert_eq!(state, SessionState::Error);
    assert!(events.contains("INTEGRATION_RESTRICTED"), "{events}");
}

// ---- ACP

async fn acp_open(mode: &str) -> Result<(Box<dyn Agent>, Model)> {
    let catalog = acp::inspect(peer("acp_peer.py", "normal"), "fixture", "r").await?;
    let model = catalog.models[0].clone();
    let agent = acp::open_with_host_mcp(
        peer("acp_peer.py", mode),
        "fixture".into(),
        "r".into(),
        model.clone(),
        false,
        None,
        &servers(),
    )
    .await?;
    Ok((agent, model))
}
#[tokio::test]
async fn acp_host_servers_go_to_session_new_and_host_tools_are_allowed() {
    // The fixture exits unless session/new carries exactly the expected
    // `mcpServers` entry, and unless the host tool's permission is allowed.
    let (agent, model) = acp_open("host").await.unwrap();
    let (state, events) = run_turn(agent, &model, json!({"thought":"future"})).await;
    assert_eq!(state, SessionState::Completed, "{events}");
    assert_eq!(
        tool_events(&events),
        [
            ("mcp__design__echo_color".into(), "pending".into()),
            ("mcp__design__echo_color".into(), "completed".into())
        ]
    );
    assert!(events.contains("color is red"));
}
#[tokio::test]
async fn acp_native_tool_still_fails_the_turn() {
    let (agent, model) = acp_open("host_native").await.unwrap();
    let (state, events) = run_turn(agent, &model, json!({"thought":"future"})).await;
    assert_eq!(state, SessionState::Error);
    assert!(events.contains("INTEGRATION_RESTRICTED"), "{events}");
    assert!(!events.contains("tool-output-sentinel"));
}
#[tokio::test]
async fn acp_peer_without_http_mcp_is_refused_typed() {
    let error = match acp_open("normal").await {
        Ok(_) => panic!("peer without mcpCapabilities.http accepted host servers"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert!(!error.message.contains(&token()));
}
#[test]
fn acp_mcp_server_entries_use_http_headers() {
    assert_eq!(
        originlm_adapters::acp::acp_mcp_servers(&servers()),
        json!([{"type":"http","name":"design","url":"http://127.0.0.1:48123/mcp","headers":[{"name":"Authorization","value":format!("Bearer {}", token())}]}])
    );
    let host = HostTools::new(&servers());
    assert_eq!(
        acp::acp_host_tool(
            &host,
            &json!({"title":"mcp__design__echo_color","kind":"other"})
        )
        .as_deref(),
        Some("mcp__design__echo_color")
    );
    assert_eq!(
        acp::acp_host_tool(&host, &json!({"title":"echo_color"})).as_deref(),
        Some("mcp__design__echo_color")
    );
    assert!(acp::acp_host_tool(&host, &json!({"title":"echo_color","kind":"read"})).is_none());
    assert!(acp::acp_host_tool(&host, &json!({"title":"Read file","kind":"other"})).is_none());
}
