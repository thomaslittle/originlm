//! Opt-in checks against an installed Muse Code. They never spend account
//! quota: metadata uses `initialize`/`account/read`/`model/list`/`usage/read`
//! only, and turns run on Muse's deterministic `echo` provider.
//!
//! `LAI_LIVE_MUSE=<absolute path to muse.cmd, muse or muse-bin-*.exe>`
//! `cargo test -p originlm-adapters --test muse_live -- --nocapture`
use originlm_adapters::muse::{MuseOpen, Posture, Routing, open, probe};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

fn muse() -> Option<PathBuf> {
    std::env::var_os("LAI_LIVE_MUSE").map(PathBuf::from)
}

fn echo_model() -> Model {
    Model {
        id: "echo".into(),
        route_id: "r".into(),
        native_id: "echo".into(),
        name: "Muse echo provider".into(),
        revision: "echo".into(),
        is_default: true,
        capabilities: vec![],
        options: vec![],
        context_window: Observation::unknown("ECHO"),
    }
}

async fn echo_turn(prompt: &str, servers: Vec<HostMcpServer>) -> Vec<Value> {
    let workspace = tempfile::tempdir().unwrap();
    let agent = open(
        &muse().unwrap(),
        MuseOpen {
            model: echo_model(),
            connection: "c".into(),
            route: "r".into(),
            workspace: workspace.path().into(),
            posture: Posture::ReadOnly,
            review: false,
            full_access: false,
            host_mcp_servers: servers,
            persistent: false,
            resume: None,
            routing: Routing::Echo,
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
                model_id: "echo".into(),
                revision: "echo".into(),
                options: BTreeMap::new(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("owner", &session.id, prompt.into(), "1".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), async {
        while !matches!(
            runtime.get("owner", &session.id).unwrap().state,
            SessionState::Completed | SessionState::Error | SessionState::Cancelled
        ) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let events = runtime.events("owner", Some(&session.id), None).unwrap();
    runtime.close("owner", &session.id).await.unwrap();
    events["events"].as_array().unwrap().clone()
}

/// Also needs `LAI_LIVE_MUSE_PERSIST=1`: a persistent session is written to
/// Muse's own session history (an echo conversation, no quota).
#[tokio::test]
async fn live_echo_session_resumes_after_its_host_exits() {
    let (Some(muse), Some(_)) = (muse(), std::env::var_os("LAI_LIVE_MUSE_PERSIST")) else {
        return;
    };
    let workspace = tempfile::tempdir().unwrap();
    let config = |resume: Option<String>| MuseOpen {
        model: echo_model(),
        connection: "c".into(),
        route: "r".into(),
        workspace: workspace.path().into(),
        posture: Posture::ReadOnly,
        review: false,
        full_access: false,
        host_mcp_servers: vec![],
        persistent: true,
        resume,
        routing: Routing::Echo,
    };
    let runtime = Runtime::new();
    let selection = Selection {
        route_id: "r".into(),
        model_id: "echo".into(),
        revision: "echo".into(),
        options: BTreeMap::new(),
    };
    let run = |session: String, request: &'static str| {
        let runtime = &runtime;
        async move {
            runtime
                .start("owner", &session, format!("turn {request}"), request.into())
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(60), async {
                while runtime.get("owner", &session).unwrap().state == SessionState::Running {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                runtime.get("owner", &session).unwrap().state,
                SessionState::Completed
            );
        }
    };
    let agent = open(&muse, config(None)).await.unwrap();
    let first = runtime
        .insert(
            "owner",
            "c".into(),
            selection.clone(),
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    run(first.id.clone(), "1").await;
    let native = runtime.suspend("owner", &first.id).await.unwrap();
    eprintln!("suspended native session {native}");
    let agent = open(&muse, config(Some(native))).await.unwrap();
    let second = runtime
        .insert("owner", "c".into(), selection, agent, Arc::new(|| Ok(())))
        .unwrap();
    run(second.id.clone(), "2").await;
    let events = runtime.events("owner", Some(&second.id), None).unwrap();
    assert!(events.to_string().contains("echo: turn 2"));
    runtime.close("owner", &second.id).await.unwrap();
    // A session Muse never had is refused, not replaced by a fresh one.
    let missing = open(
        &muse,
        config(Some("01a13398-0000-7000-8000-000000000000".into())),
    )
    .await;
    assert!(matches!(missing, Err(e) if e.code == ErrorCode::ResumeUnavailable));
}

#[tokio::test]
async fn live_metadata_reports_account_lane_and_catalog_without_inference() {
    let Some(muse) = muse() else {
        return;
    };
    let report = probe(&muse, "c", "r").await.unwrap();
    eprintln!(
        "version={:?} auth={:?} billing={:?} plan={:?} models={:?} diagnostics={:?}",
        report.version.value(),
        report.auth.value(),
        report.billing.value(),
        report.plan.value(),
        report
            .models
            .iter()
            .map(|m| (&m.native_id, m.options.len()))
            .collect::<Vec<_>>(),
        report.diagnostics
    );
    assert!(report.version.value().is_some());
}

#[tokio::test]
async fn live_echo_turn_streams_text_through_the_runtime() {
    if muse().is_none() {
        return;
    }
    let events = echo_turn("hello from local ai kit", vec![]).await;
    for event in &events {
        eprintln!("{}", event["payload"]);
    }
    let text: String = events
        .iter()
        .filter(|e| e["payload"]["type"] == "text_delta")
        .map(|e| e["payload"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(text, "echo: hello from local ai kit");
    assert!(
        events
            .iter()
            .any(|e| e["payload"]["type"] == "final" && e["payload"]["status"] == "completed")
    );
}

#[tokio::test]
async fn live_echo_turn_runs_when_the_host_mcp_server_is_unreachable() {
    if muse().is_none() {
        return;
    }
    // Nothing listens on this port: Muse must still run the turn (optional server).
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let events = echo_turn(
        "host tools may be missing",
        vec![HostMcpServer {
            name: "design".into(),
            url: format!("http://127.0.0.1:{port}/mcp"),
            bearer_token: "t".repeat(40),
            allowed_tools: vec!["set_fill".into()],
        }],
    )
    .await;
    for event in &events {
        eprintln!("{}", event["payload"]);
    }
    assert!(
        events
            .iter()
            .any(|e| e["payload"]["type"] == "final" && e["payload"]["status"] == "completed")
    );
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains(&"t".repeat(40))
    );
}
