//! Official v2 wire-shape fixtures; no installed client or credential is used.
use axum::{
    Json, Router,
    extract::State,
    middleware,
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::{delete, get, post},
};
use originlm_adapters::{
    http::{Authentication, Endpoint},
    opencode::client_v2,
};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio_stream::StreamExt;

#[derive(Clone)]
struct Fixture {
    sender: tokio::sync::broadcast::Sender<Value>,
    prompts: Arc<AtomicUsize>,
    deleted: Arc<AtomicUsize>,
    interrupted: Arc<AtomicUsize>,
    mode: &'static str,
    sequence: Arc<AtomicUsize>,
    present: Arc<AtomicBool>,
}
async fn server(mode: &'static str) -> (Endpoint, Fixture, tokio::task::JoinHandle<()>) {
    let (sender, _) = tokio::sync::broadcast::channel(32);
    let state = Fixture {
        sender,
        prompts: Arc::new(AtomicUsize::new(0)),
        deleted: Arc::new(AtomicUsize::new(0)),
        interrupted: Arc::new(AtomicUsize::new(0)),
        mode,
        sequence: Arc::new(AtomicUsize::new(0)),
        present: Arc::new(AtomicBool::new(true)),
    };
    let app=Router::new()
 .route("/api/session",post(|State(s):State<Fixture>,Json(v):Json<Value>|async move{
  assert_eq!(v["model"],json!({"providerID":"upstream","id":"native"}));
  assert_eq!(v["permissions"],json!([{"action":"*","resource":"*","effect":"deny"}]));
  Json(json!({"data":{"id":"ses_owned","model":if s.mode=="bad-model"{json!({"id":"wrong","providerID":"upstream"})}else{v["model"].clone()},"location":if s.mode=="bad-workspace"{json!({"directory":"/other"})}else{v["location"].clone()},"permissions":if s.mode=="bad-permissions" {json!([])}else{v["permissions"].clone()}}}))
 }))
 .route("/api/session/ses_owned/model",post(|State(s):State<Fixture>,Json(v):Json<Value>|async move{let mut expected=json!({"providerID":"upstream","id":"native"});if s.mode=="variant"{expected["variant"]=json!("future-effort");}assert_eq!(v["model"],expected);axum::http::StatusCode::NO_CONTENT}))
 .route("/api/event",get(|State(s):State<Fixture>|async move{let events=tokio_stream::wrappers::BroadcastStream::new(s.sender.subscribe()).filter_map(|e|e.ok().map(|v|Ok::<_,Infallible>(Event::default().data(v.to_string()))));Sse::new(events.take(if s.mode=="disconnect"{1}else{usize::MAX}))}))
 .route("/api/session/ses_owned/prompt",post(|State(s):State<Fixture>,Json(v):Json<Value>|async move{
  assert_eq!(v["text"],"hello"); assert!(s.sender.receiver_count()>0,"subscribe before prompt");let round=s.prompts.fetch_add(1,Ordering::SeqCst)+1;
  let send=|kind:&str,data:Value|{
   let mut event=json!({"type":kind,"data":data});
   if kind.starts_with("session.execution.") || matches!(kind,"session.text.started"|"session.text.ended") {
    event["durable"]=json!({"aggregateID":event["data"]["sessionID"],"seq":s.sequence.fetch_add(1,Ordering::SeqCst),"version":1});
   }
   let _=s.sender.send(event);
  };
  send("session.text.delta",json!({"sessionID":"ses_other","assistantMessageID":"a","ordinal":0,"delta":"secret-other"}));
  send("session.execution.succeeded",json!({"sessionID":"ses_other"}));
  send("future.unknown",json!({"sessionID":"ses_owned"}));
  send("session.execution.started",json!({"sessionID":"ses_owned"}));
  match s.mode {
   "hold"=>{},
   "two-turns" | "stale-text" | "late-terminal"=>{
    if s.mode=="stale-text" && round==2 {send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"text":"old reply"}));}
    send("session.text.started",json!({"sessionID":"ses_owned","assistantMessageID":if round==1{"a"}else{"b"},"ordinal":0}));
    send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":if round==1{"a"}else{"b"},"ordinal":0,"text":if round==1{"first"}else{"second"}}));
    if s.mode=="late-terminal" && round==2 {
     let _=s.sender.send(json!({"type":"session.execution.succeeded","data":{"sessionID":"ses_owned"},"durable":{"aggregateID":"ses_owned","seq":1,"version":1}}));
     send("session.text.started",json!({"sessionID":"ses_owned","assistantMessageID":"b","ordinal":1}));
    }
    send("session.execution.succeeded",json!({"sessionID":"ses_owned"}));
   },
   "unfinished-start"=>{
    send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"text":"first"}));
    send("session.text.started",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":1}));
    send("session.execution.succeeded",json!({"sessionID":"ses_owned"}));
   },
   "reverse-ordinal"=>{
    for (ordinal,text) in [(2,"world"),(1,"hello ")] {send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":ordinal,"text":text}));}
    send("session.execution.succeeded",json!({"sessionID":"ses_owned"}));
   },
   "unicode"=>{
    send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"text":"€".repeat(5000)}));
    send("session.execution.succeeded",json!({"sessionID":"ses_owned"}));
   },
   "error"=>send("session.execution.failed",json!({"sessionID":"ses_owned","error":{"message":"provider error"}})),
   "empty"=>send("session.execution.succeeded",json!({"sessionID":"ses_owned"})),
   "unterminated"=>{send("session.text.delta",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"delta":"partial"}));send("session.execution.succeeded",json!({"sessionID":"ses_owned"}));},
   "tool"=>send("permission.asked",json!({"sessionID":"ses_owned","id":"per_1"})),
   _=>{
    if s.mode!="full-only"{send("session.text.delta",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"delta":"hello"}));}
    send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"text":if s.mode=="mismatch"{"different"}else{"hello world"}}));
    if s.mode=="duplicate"{send("session.text.ended",json!({"sessionID":"ses_owned","assistantMessageID":"a","ordinal":0,"text":"hello world"}));}
    send("session.execution.succeeded",json!({"sessionID":"ses_owned"}));
   }
  }
  Json(json!({"data":{"id":"input_1","sessionID":"ses_owned"}}))
 }))
 .route("/api/session/ses_owned/interrupt",post(|State(s):State<Fixture>|async move{s.interrupted.fetch_add(1,Ordering::SeqCst);Json(json!({"interrupted":true}))}))
 .route("/api/session/ses_owned",delete(|State(s):State<Fixture>|async move{let attempt=s.deleted.fetch_add(1,Ordering::SeqCst);
 if attempt==0 && s.mode=="delete-fails"{return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();}
 if s.mode=="delete-committed" {
  if !s.present.swap(false,Ordering::SeqCst) {
   return (axum::http::StatusCode::NOT_FOUND,Json(json!({"_tag":"SessionNotFoundError","sessionID":"ses_owned","message":"Session is absent"}))).into_response();
  }
  tokio::time::sleep(Duration::from_secs(3)).await;
 }
 if matches!(s.mode,"delete-wrong-id"|"delete-wrong-tag"|"delete-denied"|"delete-html"|"delete-large") {
  if s.mode=="delete-html" {return (axum::http::StatusCode::NOT_FOUND,"<html>Not found</html>").into_response();}
  let body=json!({"_tag":if s.mode=="delete-wrong-tag"{"RouteNotFoundError"}else{"SessionNotFoundError"},"sessionID":if s.mode=="delete-wrong-id"{"ses_other"}else{"ses_owned"},"message":if s.mode=="delete-large"{"x".repeat(9000)}else{"Not found".into()}});
  return (if s.mode=="delete-denied"{axum::http::StatusCode::FORBIDDEN}else{axum::http::StatusCode::NOT_FOUND},Json(body)).into_response();
 }
 if attempt==0 && s.mode=="delete-delays"{tokio::time::sleep(Duration::from_secs(3)).await;}
 axum::http::StatusCode::NO_CONTENT.into_response()})).with_state(state.clone()).layer(middleware::from_fn(|request: axum::extract::Request, next: middleware::Next| async move {
 assert_eq!(request.headers().get("authorization").unwrap(), "Basic b3BlbmNvZGU6Zml4dHVyZS1vbmx5");
 next.run(request).await
 }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (
        Endpoint::new(
            &format!("http://127.0.0.1:{port}/"),
            Authentication::Basic("opencode".into(), "fixture-only".into()),
            true,
        )
        .unwrap(),
        state,
        task,
    )
}
fn model() -> Model {
    Model {
        id: "model".into(),
        route_id: "route".into(),
        native_id: "native".into(),
        name: "Native".into(),
        revision: "r".into(),
        is_default: true,
        capabilities: vec![],
        options: vec![OptionDescriptor {
            id: "variant".into(),
            label: "Native variant".into(),
            scope: OptionScope::Turn,
            revision: "r".into(),
            control: OptionControl::Select {
                choices: vec![Choice {
                    value: "future-effort".into(),
                    label: "Future effort".into(),
                    disabled_reason: None,
                }],
                default_value: None,
            },
            current_value: None,
            category: None,
            source: None,
            billing_sensitive: false,
            permission_sensitive: false,
        }],
        context_window: Observation::unknown("fixture"),
    }
}
async fn run(mode: &'static str) -> (String, SessionState, usize, usize) {
    let (e, s, task) = server(mode).await;
    let dir = tempfile::tempdir().unwrap();
    let agent = client_v2::open(e, model(), "upstream".into(), dir.path().into())
        .await
        .unwrap();
    assert!(!agent.native_resumption());
    assert!(!agent.image_input());
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "app",
            "connection".into(),
            Selection {
                route_id: "route".into(),
                model_id: "model".into(),
                revision: "r".into(),
                options: if mode == "variant" {
                    BTreeMap::from([("variant".into(), json!("future-effort"))])
                } else {
                    BTreeMap::new()
                },
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    let turn = runtime
        .start("app", &session.id, "hello".into(), "request".into())
        .await
        .unwrap();
    if mode == "hold" {
        tokio::time::timeout(Duration::from_secs(3), async {
            while s.prompts.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        runtime
            .cancel("app", turn["turnId"].as_str().unwrap())
            .await
            .unwrap();
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while runtime.get("app", &session.id).unwrap().state == SessionState::Running {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let status = runtime.get("app", &session.id).unwrap().state;
    let events = runtime
        .events("app", Some(&session.id), None)
        .unwrap()
        .to_string();
    runtime.close("app", &session.id).await.unwrap();
    task.abort();
    (
        events,
        status,
        s.interrupted.load(Ordering::SeqCst),
        s.deleted.load(Ordering::SeqCst),
    )
}
#[tokio::test]
async fn receives_streamed_and_final_only_replies() {
    for mode in ["reply", "full-only", "duplicate", "variant"] {
        let (events, status, interrupt, deleted) = run(mode).await;
        assert_eq!(status, SessionState::Completed);
        assert!(events.contains("hello"));
        assert!(events.contains("world"));
        let journal: Value = serde_json::from_str(&events).unwrap();
        let text: String = journal["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["payload"]["type"] == "text_delta")
            .filter_map(|event| event["payload"]["text"].as_str())
            .collect();
        assert_eq!(text, "hello world");
        assert!(!events.contains("secret-other"));
        assert_eq!(interrupt, 0);
        assert_eq!(deleted, 1);
    }
}
#[tokio::test]
async fn errors_and_tools_fail_without_approvals() {
    for mode in [
        "error",
        "tool",
        "mismatch",
        "empty",
        "unterminated",
        "disconnect",
        "unfinished-start",
        "reverse-ordinal",
    ] {
        let (events, status, interrupt, deleted) = run(mode).await;
        assert_eq!(status, SessionState::Error);
        assert!(interrupt >= 1);
        assert_eq!(deleted, 1);
        assert!(!events.contains("secret-other"));
    }
}
#[tokio::test]
async fn cancellation_interrupts_only_owned_session() {
    let (_, status, interrupt, deleted) = run("hold").await;
    assert_eq!(status, SessionState::Cancelled);
    assert!(interrupt >= 1);
    assert_eq!(deleted, 1);
}
#[tokio::test]
async fn rejects_missing_permission_echo_before_prompt() {
    for mode in ["bad-permissions", "bad-model", "bad-workspace"] {
        let (e, s, t) = server(mode).await;
        let dir = tempfile::tempdir().unwrap();
        assert!(
            client_v2::open(e, model(), "upstream".into(), dir.path().into())
                .await
                .is_err()
        );
        assert_eq!(s.prompts.load(Ordering::SeqCst), 0);
        assert_eq!(s.deleted.load(Ordering::SeqCst), 1);
        t.abort();
    }
}

#[tokio::test]
async fn text_chunks_are_utf8_bounded() {
    let (events, status, _, _) = run("unicode").await;
    assert_eq!(status, SessionState::Completed);
    let journal: Value = serde_json::from_str(&events).unwrap();
    let chunks: Vec<_> = journal["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["payload"]["type"] == "text_delta")
        .map(|event| event["payload"]["text"].as_str().unwrap())
        .collect();
    assert!(chunks.iter().all(|text| text.len() <= 8192));
    assert_eq!(chunks.concat(), "€".repeat(5000));
}
#[tokio::test]
async fn close_can_retry_a_failed_or_cancelled_delete() {
    for mode in ["delete-fails", "delete-delays", "delete-committed"] {
        let (endpoint, state, task) = server(mode).await;
        let dir = tempfile::tempdir().unwrap();
        let mut agent = client_v2::open(endpoint, model(), "upstream".into(), dir.path().into())
            .await
            .unwrap();
        if matches!(mode, "delete-delays" | "delete-committed") {
            assert!(
                tokio::time::timeout(Duration::from_millis(50), agent.close())
                    .await
                    .is_err()
            );
        } else {
            assert!(agent.close().await.is_err());
        }
        agent.close().await.unwrap();
        assert_eq!(state.deleted.load(Ordering::SeqCst), 2);
        agent.close().await.unwrap();
        assert_eq!(state.deleted.load(Ordering::SeqCst), 2);
        task.abort();
    }
}

#[tokio::test]
async fn rejects_delayed_prior_turn_events_without_breaking_fresh_continuations() {
    for mode in ["two-turns", "stale-text", "late-terminal"] {
        let (endpoint, _, task) = server(mode).await;
        let dir = tempfile::tempdir().unwrap();
        let agent = client_v2::open(endpoint, model(), "upstream".into(), dir.path().into())
            .await
            .unwrap();
        let runtime = Runtime::new();
        let session = runtime
            .insert(
                "app",
                "connection".into(),
                Selection {
                    route_id: "route".into(),
                    model_id: "model".into(),
                    revision: "r".into(),
                    options: BTreeMap::new(),
                },
                agent,
                Arc::new(|| Ok(())),
            )
            .unwrap();
        for round in 1..=2 {
            runtime
                .start(
                    "app",
                    &session.id,
                    "hello".into(),
                    format!("request-{round}"),
                )
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
                if matches!(mode, "stale-text" | "late-terminal") && round == 2 {
                    SessionState::Error
                } else {
                    SessionState::Completed
                }
            );
        }
        let events = runtime
            .events("app", Some(&session.id), None)
            .unwrap()
            .to_string();
        assert!(!events.contains("old reply"));
        runtime.close("app", &session.id).await.unwrap();
        task.abort();
    }
}

#[tokio::test]
async fn uncertain_delete_responses_keep_owned_id_for_retry() {
    for mode in [
        "delete-wrong-id",
        "delete-wrong-tag",
        "delete-denied",
        "delete-html",
        "delete-large",
    ] {
        let (endpoint, state, task) = server(mode).await;
        let dir = tempfile::tempdir().unwrap();
        let mut agent = client_v2::open(endpoint, model(), "upstream".into(), dir.path().into())
            .await
            .unwrap();
        assert!(agent.close().await.is_err(), "{mode}");
        assert!(agent.close().await.is_err(), "{mode}");
        assert_eq!(
            state.deleted.load(Ordering::SeqCst),
            2,
            "uncertain cleanup must retain identity: {mode}"
        );
        task.abort();
    }
}
