use axum::{
    Json, Router,
    routing::{get, post},
};
use originlm_kit::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[tokio::test]
async fn accountless_ollama_flows_through_discovery_selection_grants_and_streaming() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let server = Router::new()
        .route("/api/tags", get(|| async { Json(json!({"models":[{"name":"fixture-native"}]})) }))
        .route("/api/show", post(|| async { Json(json!({"capabilities":["completion"]})) }))
        .route("/api/chat", post(move |Json(body): Json<Value>| {
            let count = count.clone();
            async move {
                assert_eq!(body["model"], "fixture-native");
                assert_eq!(body["stream"], true);
                count.fetch_add(1, Ordering::SeqCst);
                "{\"message\":{\"content\":\"fixture output\"},\"done\":false}\n{\"done\":true}\n"
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, server).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    let endpoints = EndpointRegistry::parse(&json!({"endpoints":[{"id":"fixture-local","label":"Fixture local","kind":"ollama","baseUrl":format!("http://{address}/")}]}).to_string(), "fixture").unwrap();
    let kit = Kit::builder()
        .home(root.path().into())
        .endpoints(endpoints)
        .execution(true)
        .build()
        .unwrap();
    let policy = ClientPolicy {
        probe_provider_ids: BTreeSet::from(["ollama".into()]),
        ..ClientPolicy::passive()
    };
    let app = kit.connect("scene-designer-fixture", policy).unwrap();
    let detected = app.scan().await.unwrap();
    let connection = detected
        .connections
        .iter()
        .find(|c| c.provider_id == "ollama")
        .unwrap();
    let snapshot = app.probe(&connection.id).await.unwrap();
    assert!(snapshot.connections[0].auth.value().is_none());
    let model = &snapshot.models[0];
    let selection = Selection {
        route_id: model.route_id.clone(),
        model_id: model.id.clone(),
        revision: model.revision.clone(),
        options: Default::default(),
    };
    let _: Value = app
        .request("selection.validate", json!({"selection":selection}))
        .await
        .unwrap();
    assert!(
        app.request::<Value>(
            "sessions.create",
            json!({"selection":selection,"workspaceId":"scratch"})
        )
        .await
        .is_err()
    );
    app.authorize_execution(ExecutionGrant {
        connection_ids: BTreeSet::from([connection.id.clone()]),
        workspaces: BTreeMap::from([("scratch".into(), root.path().into())]),
        ..Default::default()
    })
    .unwrap();
    let session: Value = app
        .request(
            "sessions.create",
            json!({"selection":selection,"workspaceId":"scratch"}),
        )
        .await
        .unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "metadata and session creation never infer"
    );
    let turn: Value = app.request("turns.start", json!({"sessionId":session["id"],"prompt":"Generate fixture output","requestId":"fixture-1"})).await.unwrap();
    let events = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let events: Value = app
                .request("events.read", json!({"sessionId":session["id"]}))
                .await
                .unwrap();
            if events["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["turnId"] == turn["turnId"] && e["payload"]["type"] == "final")
            {
                break events;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(events.to_string().contains("fixture output"));
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["payload"]["type"] == "final" && e["payload"]["status"] == "completed")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    app.close();
    server.abort();
}
