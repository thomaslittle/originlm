use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use originlm_adapters::{
    http::{Authentication, Endpoint},
    http_provider::*,
};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
async fn server() -> (Endpoint, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let app=Router::new().route("/api/tags",get(||async{Json(json!({"models":[{"name":"native"}]}))})).route("/api/show",post(||async{Json(json!({"capabilities":["completion"]}))})).route("/api/chat",post(|State(c):State<Arc<AtomicUsize>>,Json(v):Json<Value>|async move{
 c.fetch_add(1,Ordering::SeqCst);assert_eq!(v["model"],"native");assert_eq!(v["stream"],true);assert!(v.get("tools").is_none());
 let count=v["messages"].as_array().unwrap().len();
 format!("{{\"message\":{{\"content\":\"reply {count}\"}},\"done\":false}}\n{{\"message\":{{\"content\":\"\"}},\"done\":true,\"eval_count\":2}}\n")
 })).with_state(calls.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (
        Endpoint::new(
            &format!("http://127.0.0.1:{port}/"),
            Authentication::None,
            true,
        )
        .unwrap(),
        calls,
        server,
    )
}
#[tokio::test]
async fn probing_never_runs_generation_and_catalog_does_not_claim_paid_access() {
    let (endpoint, calls, task) = server().await;
    let metadata = probe(HttpKind::Ollama, endpoint, "c".into(), "r".into(), vec![])
        .await
        .unwrap();
    assert_eq!(metadata.models.len(), 1);
    assert_eq!(metadata.billing.value(), Some(&BillingMode::Local));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    task.abort();
}
#[tokio::test]
async fn native_runtime_streams_http_turns_with_owned_history() {
    let (endpoint, calls, task) = server().await;
    let metadata = probe(
        HttpKind::Ollama,
        endpoint.clone(),
        "c".into(),
        "r".into(),
        vec![],
    )
    .await
    .unwrap();
    let model = metadata.models[0].clone();
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "app",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: BTreeMap::new(),
            },
            executor(HttpKind::Ollama, endpoint, model),
            Arc::new(|| Ok(())),
        )
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for i in 0..2 {
        runtime
            .start("app", &session.id, "hello".into(), format!("request-{i}"))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while runtime.get("app", &session.id).unwrap().state == SessionState::Running {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            runtime.get("app", &session.id).unwrap().state,
            SessionState::Completed
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let events = runtime
        .events("app", Some(&session.id), None)
        .unwrap()
        .to_string();
    assert!(events.contains("reply 1"));
    assert!(events.contains("reply 3"));
    assert!(
        runtime.events("other", None, None).unwrap()["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    runtime.close("app", &session.id).await.unwrap();
    task.abort();
}
