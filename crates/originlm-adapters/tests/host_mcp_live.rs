//! Opt-in live check: one Claude Code turn that calls a host MCP tool.
//! Skipped unless `LAI_LIVE_CLAUDE=<claude executable>`,
//! `LAI_LIVE_CLAUDE_HOME=<Claude config dir>`, `LAI_LIVE_MCP_URL` (a loopback
//! server exposing `echo_color`) and `LAI_LIVE_MCP_TOKEN` are all set.
//! `LAI_LIVE_CLAUDE_MODEL` picks the native model by substring (default `haiku`).
//! `LAI_LIVE_MCP_STATUS_ONLY=1` stops after confirming the server connected
//! (no prompt, no inference).
use originlm_adapters::{
    claude, codex,
    execution::{CodexExecution, open_codex},
    host_mcp::HostTools,
    process::LaunchSpec,
};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
#[tokio::test]
async fn live_claude_turn_calls_host_and_native_tools() {
    let (Some(exe), Some(home), Ok(url), Ok(token)) = (
        std::env::var_os("LAI_LIVE_CLAUDE"),
        std::env::var_os("LAI_LIVE_CLAUDE_HOME"),
        std::env::var("LAI_LIVE_MCP_URL"),
        std::env::var("LAI_LIVE_MCP_TOKEN"),
    ) else {
        return;
    };
    let wanted = std::env::var("LAI_LIVE_CLAUDE_MODEL").unwrap_or_else(|_| "haiku".into());
    let native_tools = std::env::var_os("LAI_LIVE_MCP_NATIVE_TOOLS").is_some();
    let located = originlm_discovery::LocatedClient {
        executable_identity: None,
        provider_id: "claude_code".into(),
        profile_home: home.into(),
        executable: Some(exe.into()),
        scope: None,
    };
    let report = claude::launch::probe(located.clone(), "c".into(), "r".into())
        .await
        .unwrap();
    let model = report
        .models
        .iter()
        .find(|m| m.native_id.contains(&wanted) || m.id.contains(&wanted))
        .expect("model listed")
        .clone();
    let servers = vec![HostMcpServer {
        name: "design".into(),
        url,
        bearer_token: token,
        allowed_tools: vec!["echo_color".into()],
    }];
    let workspace = tempfile::tempdir().unwrap();
    let launch = claude::launch::launch_with_host_mcp_policy(
        &located,
        workspace.path().canonicalize().unwrap(),
        &servers,
        native_tools,
    )
    .unwrap();
    claude::client::probe_host_mcp_policy(
        launch.spec.clone(),
        &HostTools::new(&servers),
        native_tools,
    )
    .await
    .unwrap();
    if std::env::var_os("LAI_LIVE_MCP_STATUS_ONLY").is_some() {
        return;
    }
    let agent = claude::client::host_executor_policy(
        launch,
        HostTools::new(&servers),
        model.clone(),
        "c".into(),
        "r".into(),
        native_tools,
        native_tools,
    );
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
    let prompt = if native_tools {
        "Use the native Write tool to write exactly native-policy-ok to native-policy.txt in the current workspace, then use Read to read it back. Use WebSearch to check the current weather in New York City. Call the host echo_color tool exactly once with color teal. Reply with the file content, whether the weather search succeeded, and the host tool result."
    } else {
        "Call the echo_color tool with color \"teal\" exactly once, then reply with the tool's result and nothing else."
    };
    runtime
        .start("owner", &session.id, prompt.into(), "1".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(120), async {
        while matches!(
            runtime.get("owner", &session.id).unwrap().state,
            SessionState::Running | SessionState::AwaitingApproval
        ) {
            if native_tools {
                let events = runtime.events("owner", Some(&session.id), None).unwrap();
                for event in events["events"].as_array().into_iter().flatten() {
                    if event["payload"]["type"] == "approval_request" {
                        let approval_id = event["payload"]["approvalId"].as_str().unwrap();
                        let _ = runtime.approve("owner", approval_id, true);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let events = runtime.events("owner", None, None).unwrap();
    assert_eq!(
        runtime.get("owner", &session.id).unwrap().state,
        SessionState::Completed
    );
    let tools: Vec<_> = events["events"]
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
        .collect();
    assert!(tools.contains(&("mcp__design__echo_color".into(), "completed".into())));
    if native_tools {
        let approvals = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["payload"]["type"] == "approval_request")
            .count();
        let errors: Vec<_> = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["payload"]["type"] == "error")
            .map(|event| {
                (
                    event["payload"]["code"].clone(),
                    event["payload"]["message"].clone(),
                )
            })
            .collect();
        eprintln!(
            "native tool statuses: {tools:?}; approval requests: {approvals}; errors: {errors:?}"
        );
        let contents = std::fs::read_to_string(workspace.path().join("native-policy.txt")).unwrap();
        assert_eq!(contents, "native-policy-ok");
        assert!(
            approvals > 0,
            "the host must receive a native approval request"
        );
        assert!(tools.iter().any(|(name, _)| name == "Write"));
        assert!(tools.iter().any(|(name, _)| name == "Read"));
        assert!(
            tools
                .iter()
                .any(|(name, _)| name.to_lowercase().contains("web"))
        );
    } else {
        assert!(
            tools
                .iter()
                .all(|(name, _)| name == "mcp__design__echo_color")
        );
    }
    runtime.close("owner", &session.id).await.unwrap();
}

/// Opt-in end-to-end qualification of Codex native tools plus a host MCP call.
/// Uses the user's normal Codex CLI login; no credentials are read by this test.
#[tokio::test]
async fn live_codex_native_tools_and_host_mcp_share_a_session() {
    let (Some(exe), Some(home), Ok(url), Ok(token)) = (
        std::env::var_os("LAI_LIVE_CODEX"),
        std::env::var_os("LAI_LIVE_CODEX_HOME"),
        std::env::var("LAI_LIVE_MCP_URL"),
        std::env::var("LAI_LIVE_MCP_TOKEN"),
    ) else {
        return;
    };
    let wanted = std::env::var("LAI_LIVE_CODEX_MODEL").unwrap_or_default();
    let located = originlm_discovery::LocatedClient {
        executable_identity: None,
        provider_id: "codex".into(),
        profile_home: home.clone().into(),
        executable: Some(exe.into()),
        scope: None,
    };
    let metadata = codex::probe(located.clone(), "c".into(), "r".into())
        .await
        .unwrap();
    let model = metadata
        .models
        .iter()
        .find(|model| {
            !wanted.is_empty()
                && model
                    .native_id
                    .to_lowercase()
                    .contains(&wanted.to_lowercase())
        })
        .or_else(|| {
            metadata.models.iter().find(|model| {
                let id = model.native_id.to_lowercase();
                id.contains("luna") || id.contains("mini")
            })
        })
        .or_else(|| metadata.models.iter().find(|model| model.is_default))
        .expect("Codex reports an available model")
        .clone();
    let workspace = tempfile::tempdir().unwrap();
    let servers = vec![HostMcpServer {
        name: "design".into(),
        url,
        bearer_token: token,
        allowed_tools: vec!["echo_color".into()],
    }];
    let executable = codex::resolve_executable(located.executable.as_deref().unwrap()).unwrap();
    let mut args: Vec<std::ffi::OsString> = vec!["app-server".into(), "--stdio".into()];
    for feature in [
        "hooks",
        "plugins",
        "remote_plugin",
        "apps",
        "shell_snapshot",
    ] {
        args.extend(["--disable".into(), feature.into()]);
    }
    let spec = LaunchSpec {
        executable,
        args,
        cwd: workspace.path().canonicalize().unwrap(),
        env: std::collections::BTreeMap::from([("CODEX_HOME".into(), home)]),
    };
    let agent = open_codex(
        spec,
        CodexExecution {
            model: model.clone(),
            connection: "c".into(),
            route: "r".into(),
            writable: false,
            review_requests: true,
            native_policy: true,
            host_mcp_servers: servers,
            persistent: false,
            resume_thread: None,
        },
    )
    .await
    .unwrap();
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
    runtime
        .start(
            "owner",
            &session.id,
            "Use your native shell tool to write exactly native-policy-ok to native-policy.txt in the current workspace, then read it back. Use web search to check the current weather in New York City. Call the host echo_color tool exactly once with color teal. Reply with the file content, whether the weather search succeeded, and the host tool result.".into(),
            "1".into(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(180), async {
        while matches!(
            runtime.get("owner", &session.id).unwrap().state,
            SessionState::Running | SessionState::AwaitingApproval
        ) {
            let events = runtime.events("owner", Some(&session.id), None).unwrap();
            for event in events["events"].as_array().into_iter().flatten() {
                if event["payload"]["type"] == "approval_request" {
                    let approval_id = event["payload"]["approvalId"].as_str().unwrap();
                    let _ = runtime.approve("owner", approval_id, true);
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let events = runtime.events("owner", None, None).unwrap();
    assert_eq!(
        runtime.get("owner", &session.id).unwrap().state,
        SessionState::Completed
    );
    let tools: Vec<_> = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["payload"]["type"] == "tool")
        .map(|event| {
            (
                event["payload"]["name"].as_str().unwrap().to_owned(),
                event["payload"]["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(tools.contains(&("mcp__design__echo_color".into(), "completed".into())));
    assert!(tools.iter().any(|(name, _)| name == "commandExecution"));
    assert!(tools.iter().any(|(name, _)| name == "webSearch"));
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("native-policy.txt")).unwrap(),
        "native-policy-ok"
    );
    runtime.close("owner", &session.id).await.unwrap();
}
