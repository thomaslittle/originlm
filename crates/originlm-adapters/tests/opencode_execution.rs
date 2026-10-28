use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event, Sse},
    routing::{delete, get, post},
};
use originlm_adapters::{
    http::{Authentication, Endpoint},
    opencode::client,
};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio_stream::StreamExt;
#[derive(Clone)]
struct StateData {
    calls: Arc<AtomicUsize>,
    deleted: Arc<AtomicUsize>,
    sender: tokio::sync::broadcast::Sender<Value>,
    bad: bool,
}
async fn server(bad: bool) -> (Endpoint, StateData, tokio::task::JoinHandle<()>) {
    let (sender, _) = tokio::sync::broadcast::channel(32);
    let state = StateData {
        calls: Arc::new(AtomicUsize::new(0)),
        deleted: Arc::new(AtomicUsize::new(0)),
        sender,
        bad,
    };
    let app=Router::new().route("/session",post(|State(s):State<StateData>,Json(body):Json<Value>|async move{assert_eq!(body["permission"],json!([{"permission":"*","pattern":"*","action":"deny"}]));Json(json!({"id":"ses_owned","permission":if s.bad{json!([])}else{body["permission"].clone()}}))}))
 .route("/event",get(|State(s):State<StateData>|async move{let stream=tokio_stream::wrappers::BroadcastStream::new(s.sender.subscribe()).filter_map(|value|value.ok().map(|v|Ok::<_,Infallible>(Event::default().data(v.to_string()))));Sse::new(stream)}))
 .route("/session/ses_owned/prompt_async",post(|State(s):State<StateData>,Json(v):Json<Value>|async move{assert_eq!(v["model"],json!({"providerID":"upstream","modelID":"native"}));assert_eq!(v["parts"][0]["type"],"text");s.calls.fetch_add(1,Ordering::SeqCst);
 for event in [json!({"type":"message.updated","properties":{"info":{"id":"msg_assistant","role":"assistant","sessionID":"ses_owned","modelID":"native","providerID":"upstream"}}}),json!({"type":"message.part.delta","properties":{"sessionID":"ses_other","messageID":"msg_assistant","field":"text","delta":"secret-other-session"}}),json!({"type":"message.part.delta","properties":{"sessionID":"ses_owned","messageID":"msg_assistant","field":"text","delta":"hello"}}),json!({"type":"session.status","properties":{"sessionID":"ses_owned","status":{"type":"idle"}}})]{let _=s.sender.send(event);}axum::http::StatusCode::NO_CONTENT}))
 .route("/session/ses_owned/message",get(||async{Json(json!([{"info":{"id":"msg_assistant","role":"assistant","modelID":"native","providerID":"upstream","time":{"completed":123},"tokens":{"input":2,"output":1}},"parts":[{"type":"text","text":"hello world"}]}]))}))
 .route("/session/ses_owned",delete(|State(s):State<StateData>|async move{s.deleted.fetch_add(1,Ordering::SeqCst);Json(true)}))
 .route("/session/ses_owned/abort",post(||async{Json(true)})).with_state(state.clone());
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
        state,
        task,
    )
}
fn model() -> Model {
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
async fn permission_echo_is_required_before_any_generation() {
    let (e, s, t) = server(true).await;
    let dir = tempfile::tempdir().unwrap();
    assert!(
        client::open(e, model(), "upstream".into(), dir.path().into())
            .await
            .is_err()
    );
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
    t.abort();
}
#[tokio::test]
async fn owns_one_native_session_streams_and_deletes_only_that_session() {
    let (e, s, t) = server(false).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = client::open(e, model(), "upstream".into(), dir.path().into())
        .await
        .unwrap();
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "app",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: "m".into(),
                revision: "v".into(),
                options: BTreeMap::new(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("app", &session.id, "hello".into(), "one".into())
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
    let events = runtime
        .events("app", Some(&session.id), None)
        .unwrap()
        .to_string();
    assert!(events.contains("hello"));
    assert!(events.contains(" world"));
    assert!(!events.contains("secret-other-session"));
    runtime.close("app", &session.id).await.unwrap();
    assert_eq!(s.deleted.load(Ordering::SeqCst), 1);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    t.abort();
}
