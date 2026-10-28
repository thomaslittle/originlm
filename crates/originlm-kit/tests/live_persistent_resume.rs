//! Opt-in live check of durable resume through the real kit: one Claude Code
//! turn, suspend, a fresh helper (new service over the same state directory),
//! resume, and a second turn that must remember a word from the first.
//! Skipped unless `LAI_LIVE_RESUME_CLAUDE=1`. Uses the installed Claude Code
//! and its signed-in profile; `LAI_LIVE_CLAUDE_MODEL` picks the model by
//! substring (default `haiku`). Two short turns in total.
use originlm_kit::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

fn helper(state: &std::path::Path) -> Arc<NativeService> {
    build_service_with_state(
        inventory_source(None),
        true,
        Arc::new(EndpointRegistry::default()),
        Arc::new(NativeClientRegistry::default()),
        Arc::new(BillingDeclarations::default()),
        Some(state.to_path_buf()),
    )
}
async fn connect(service: &NativeService, workspace: &std::path::Path) -> (String, Selection) {
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids = ["claude_code".to_owned()].into();
    service.register_client("live", policy).unwrap();
    service
        .set_app_identity("live", "live-resume-test")
        .unwrap();
    service
        .register_execution(
            "live",
            ExecutionGrant {
                provider_ids: ["claude_code".to_owned()].into(),
                workspaces: BTreeMap::from([("w".into(), workspace.to_path_buf())]),
                ..Default::default()
            },
        )
        .unwrap();
    service
        .handle("live", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let snapshot = service
        .handle("live", "snapshot.get", json!({}))
        .await
        .unwrap();
    let connection = snapshot["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["providerId"] == "claude_code")
        .expect("Claude Code connection")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    service
        .handle(
            "live",
            "connections.probe",
            json!({"connectionId":connection}),
        )
        .await
        .unwrap();
    let wanted = std::env::var("LAI_LIVE_CLAUDE_MODEL").unwrap_or_else(|_| "haiku".into());
    let snapshot = service
        .handle("live", "snapshot.get", json!({}))
        .await
        .unwrap();
    let route = snapshot["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["connectionId"] == connection.as_str())
        .unwrap()
        .clone();
    let cap = route["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "session.resume.persistent")
        .expect("persistent resume capability");
    assert_eq!(cap["support"], "supported");
    let model = snapshot["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| {
            m["routeId"] == route["id"]
                && (m["nativeId"].as_str().unwrap_or("").contains(&wanted)
                    || m["id"].as_str().unwrap_or("").contains(&wanted))
        })
        .expect("model listed")
        .clone();
    (
        connection,
        Selection {
            route_id: route["id"].as_str().unwrap().into(),
            model_id: model["id"].as_str().unwrap().into(),
            revision: model["revision"].as_str().unwrap().into(),
            options: BTreeMap::new(),
        },
    )
}
async fn turn(service: &NativeService, session: &str, prompt: &str) -> String {
    let started = service
        .handle(
            "live",
            "turns.start",
            json!({"sessionId":session,"prompt":prompt,"requestId":prompt.len().to_string()}),
        )
        .await
        .unwrap();
    let _ = started;
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let state = service
                .handle("live", "sessions.get", json!({"sessionId":session}))
                .await
                .unwrap()["state"]
                .clone();
            if state != "running" {
                assert_eq!(state, "completed");
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    let events: Value = service
        .handle("live", "events.read", json!({"sessionId":session}))
        .await
        .unwrap();
    events["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["payload"]["type"] == "text_delta")
        .filter_map(|e| e["payload"]["text"].as_str())
        .collect()
}
#[tokio::test]
async fn live_claude_conversation_resumes_after_a_helper_restart() {
    if std::env::var_os("LAI_LIVE_RESUME_CLAUDE").is_none() {
        return;
    }
    let state = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let workspace = workspace.path().canonicalize().unwrap();
    let token = {
        let service = helper(state.path());
        let (_, selection) = connect(&service, &workspace).await;
        let session = service
            .handle(
                "live",
                "sessions.create",
                json!({"selection":selection,"workspaceId":"w","persistent":true}),
            )
            .await
            .unwrap();
        let id = session["id"].as_str().unwrap().to_owned();
        let first = turn(
            &service,
            &id,
            "Remember the code word PELICAN-7. Reply with only: OK",
        )
        .await;
        println!("turn 1: {first}");
        let suspended = service
            .handle("live", "sessions.suspend", json!({"sessionId":id}))
            .await
            .unwrap();
        suspended["resumeToken"].as_str().unwrap().to_owned()
    };
    assert!(token.starts_with("lai1_"));
    // A restarted helper: a new service over the same state directory.
    let service = helper(state.path());
    connect(&service, &workspace).await;
    let resumed = service
        .handle("live", "sessions.resume", json!({"resumeToken":token}))
        .await
        .unwrap();
    let second = turn(
        &service,
        resumed["id"].as_str().unwrap(),
        "What was the code word? Reply with only the code word.",
    )
    .await;
    println!("turn 2: {second}");
    assert!(second.to_uppercase().contains("PELICAN-7"), "{second}");
    service
        .handle("live", "sessions.close", json!({"sessionId":resumed["id"]}))
        .await
        .unwrap();
}
