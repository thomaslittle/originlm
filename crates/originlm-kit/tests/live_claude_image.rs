//! Opt-in live check of image input through the real kit: one Claude Code
//! turn carrying a staged 32x32 solid red PNG. Skipped unless
//! `LAI_LIVE_IMAGE_CLAUDE=1`. Uses the installed Claude Code and its signed-in
//! profile; `LAI_LIVE_CLAUDE_MODEL` picks the model by substring (default
//! `haiku`). One short turn.
use originlm_kit::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

const RED_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAIAAAD8GO2jAAAAKElEQVR4nO3NsQ0AAAzCMP5/un0CNkuZ41wybXsHAAAAAAAAAAAAxR4yw/wuPL6QkAAAAABJRU5ErkJggg==";

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
    service.set_app_identity("live", "live-image-test").unwrap();
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
    let _ = cap;
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
async fn turn(service: &NativeService, session: &str, prompt: &str, inputs: Value) -> String {
    let started = service
        .handle(
            "live",
            "turns.start",
            json!({"sessionId":session,"prompt":prompt,"requestId":prompt.len().to_string(),"inputs":inputs}),
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
async fn live_claude_turn_sees_a_staged_image() {
    if std::env::var_os("LAI_LIVE_IMAGE_CLAUDE").is_none() {
        return;
    }
    let state = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let workspace = workspace.path().canonicalize().unwrap();
    let service = helper(state.path());
    let (_, selection) = connect(&service, &workspace).await;
    let session = service
        .handle(
            "live",
            "sessions.create",
            json!({"selection":selection,"workspaceId":"w"}),
        )
        .await
        .unwrap();
    let id = session["id"].as_str().unwrap().to_owned();
    let staged = service
        .handle(
            "live",
            "inputs.put",
            json!({"sessionId":id,"mediaType":"image/png","dataBase64":RED_PNG}),
        )
        .await
        .unwrap();
    let reply = turn(
        &service,
        &id,
        "What single color fills this image? Reply with one lowercase word.",
        json!([{"inputId": staged["inputId"]}]),
    )
    .await;
    println!("reply: {reply}");
    assert!(reply.to_lowercase().contains("red"), "{reply}");
    service
        .handle("live", "sessions.close", json!({"sessionId":id}))
        .await
        .unwrap();
}
