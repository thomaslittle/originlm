#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct TestAgent(Arc<std::sync::Mutex<Vec<String>>>);
impl Agent for TestAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.0.lock().unwrap().push(prompt);
            cx.emit(EventPayload::TextDelta {
                text: "service-native-fixture".into(),
            })?;
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
fn setup(
    granted: bool,
) -> (
    Arc<NativeService>,
    Arc<AtomicUsize>,
    Arc<std::sync::Mutex<Vec<String>>>,
) {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let prompts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = prompts.clone();
    let service = Arc::new(
        NativeService::new(
            Arc::new(|_| {
                let s = common::fixture();
                let locations = s
                    .connections
                    .iter()
                    .map(|c| {
                        (
                            c.id.clone(),
                            LocatedClient {
                                executable_identity: None,
                                provider_id: "test".into(),
                                profile_home: std::env::temp_dir(),
                                executable: None,
                                scope: None,
                            },
                        )
                    })
                    .collect();
                Ok(ScanResult {
                    snapshot: s,
                    locations,
                    metadata_probes: 0,
                })
            }),
            Duration::from_secs(60),
        )
        .with_execution(
            BTreeSet::from(["test".into()]),
            Arc::new(move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                let seen = seen.clone();
                Box::pin(async move { Ok(Box::new(TestAgent(seen)) as Box<dyn Agent>) })
            }),
        ),
    );
    service
        .register_client("a", ClientPolicy::passive())
        .unwrap();
    service
        .register_client("b", ClientPolicy::denied())
        .unwrap();
    if granted {
        service
            .register_execution(
                "a",
                ExecutionGrant {
                    connection_ids: BTreeSet::from(["personal".into()]),
                    workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    (service, count, prompts)
}
fn create() -> Value {
    json!({"selection":common::selection(),"workspaceId":"scratch"})
}
#[tokio::test]
async fn revocation_during_open_then_regrant_closes_the_unpublished_agent() {
    struct CloseCounter(Arc<AtomicUsize>);
    impl Agent for CloseCounter {
        fn turn<'a>(
            &'a mut self,
            _: String,
            _: BTreeMap<String, Value>,
            _: TurnContext,
        ) -> BoxFuture<'a, ()> {
            Box::pin(async { panic!("revoked startup cannot infer") })
        }
        fn close(&mut self) -> BoxFuture<'_, ()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let closed = Arc::new(AtomicUsize::new(0));
    let (started, finish, count) = (entered.clone(), release.clone(), closed.clone());
    let service = Arc::new(
        NativeService::new(
            Arc::new(|_| {
                Ok(ScanResult {
                    snapshot: common::fixture(),
                    locations: BTreeMap::from([(
                        "personal".into(),
                        LocatedClient {
                            executable_identity: None,
                            provider_id: "test".into(),
                            profile_home: std::env::temp_dir(),
                            executable: None,
                            scope: None,
                        },
                    )]),
                    metadata_probes: 0,
                })
            }),
            Duration::ZERO,
        )
        .with_execution(
            BTreeSet::from(["test".into()]),
            Arc::new(move |_| {
                let (started, finish, count) = (started.clone(), finish.clone(), count.clone());
                Box::pin(async move {
                    started.notify_one();
                    finish.notified().await;
                    Ok(Box::new(CloseCounter(count)) as Box<dyn Agent>)
                })
            }),
        ),
    );
    service
        .register_client("a", ClientPolicy::passive())
        .unwrap();
    let grant = ExecutionGrant {
        connection_ids: BTreeSet::from(["personal".into()]),
        workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
        ..Default::default()
    };
    service.register_execution("a", grant.clone()).unwrap();
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let operation = service.clone();
    let pending =
        tokio::spawn(async move { operation.handle("a", "sessions.create", create()).await });
    entered.notified().await;
    service.revoke_execution("a");
    service.register_execution("a", grant).unwrap();
    release.notify_one();
    assert_eq!(
        pending.await.unwrap().unwrap_err().code,
        ErrorCode::LocalAccessDenied
    );
    assert_eq!(closed.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn discovery_permission_does_not_authorize_execution() {
    let (s, c, _) = setup(false);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    assert_eq!(
        s.handle("a", "sessions.create", create())
            .await
            .unwrap_err()
            .code,
        ErrorCode::LocalAccessDenied
    );
    assert_eq!(c.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn unknown_workspaces_and_permission_upgrades_are_rejected() {
    let (s, c, _) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    for extra in [
        json!({"workspaceId":"other"}),
        json!({"permissionMode":"workspace_write"}),
        json!({"reviewRequests":true}),
        json!({"allowPaid":true}),
    ] {
        let mut p = create();
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        assert!(s.handle("a", "sessions.create", p).await.is_err());
    }
    assert_eq!(c.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn session_factory_and_events_go_through_public_service() {
    let (s, c, _) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let session = s.handle("a", "sessions.create", create()).await.unwrap();
    assert_eq!(c.load(Ordering::SeqCst), 1);
    assert!(
        s.handle("b", "sessions.get", json!({"sessionId":session["id"]}))
            .await
            .is_err()
    );
    s.handle(
        "a",
        "turns.start",
        json!({"sessionId":session["id"],"prompt":"hi","requestId":"one"}),
    )
    .await
    .unwrap();
    let data = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let e = s.handle("a", "events.read", json!({})).await.unwrap();
            if e.to_string().contains("service-native-fixture") {
                break e;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert!(!data["events"].as_array().unwrap().is_empty());
    assert!(
        s.handle("b", "events.read", json!({})).await.unwrap()["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    s.handle("a", "grants.revoke", json!({})).await.unwrap();
    assert!(
        s.handle(
            "a",
            "turns.start",
            json!({"sessionId":session["id"],"prompt":"hi","requestId":"two"})
        )
        .await
        .is_err()
    );
}
#[tokio::test]
async fn long_turn_prompt_reaches_native_agent_unchanged() {
    let (service, _, prompts) = setup(true);
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let session = service
        .handle("a", "sessions.create", create())
        .await
        .unwrap();
    let prompt = "design brief\n".repeat(10_000);
    assert_eq!(prompt.len(), 130_000);
    service
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":session["id"],"prompt":prompt,"requestId":"long-design"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if prompts.lock().unwrap().len() == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(prompts.lock().unwrap()[0], prompt);
}
#[tokio::test]
async fn turn_prompt_and_serialized_params_have_separate_bounds() {
    let (service, _, prompts) = setup(true);
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let session = service
        .handle("a", "sessions.create", create())
        .await
        .unwrap();
    let session_id = session["id"].as_str().unwrap();
    let maximum_prompt = "a".repeat(512 * 1024);
    service
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":session_id,"prompt":maximum_prompt,"requestId":"max-prompt"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if service
                .handle("a", "sessions.get", json!({"sessionId":session_id}))
                .await
                .unwrap()["state"]
                == "completed"
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(prompts.lock().unwrap()[0], maximum_prompt);

    let too_long = "a".repeat(512 * 1024 + 1);
    assert_eq!(
        service
            .handle(
                "a",
                "turns.start",
                json!({"sessionId":session_id,"prompt":too_long,"requestId":"over-prompt"}),
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    // The prompt's decoded byte length fits, but its JSON escaping exceeds the
    // serialized params budget.
    let escaped_too_large = "\u{0001}".repeat(175_000);
    assert_eq!(
        service
            .handle(
                "a",
                "turns.start",
                json!({"sessionId":session_id,"prompt":escaped_too_large,"requestId":"over-wire"}),
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    assert_eq!(prompts.lock().unwrap().len(), 1);

    let escaped_within_budget = "\u{0001}".repeat(170_000);
    service
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":session_id,"prompt":escaped_within_budget,"requestId":"escaped-ok"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if prompts.lock().unwrap().len() == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(prompts.lock().unwrap()[1], escaped_within_budget);
}
#[tokio::test]
async fn long_turn_exception_keeps_typed_and_other_method_bounds() {
    let (service, _, prompts) = setup(true);
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let session = service
        .handle("a", "sessions.create", create())
        .await
        .unwrap();
    let prompt = "design brief ".repeat(10_000);
    for params in [
        json!({"sessionId":session["id"],"prompt":prompt,"requestId":"unknown","extra":true}),
        json!({"sessionId":session["id"],"prompt":prompt,"requestId":"smuggled","clientId":"b"}),
        json!({"sessionId":session["id"],"prompt":123,"requestId":"wrong-type"}),
    ] {
        assert_eq!(
            service
                .handle("a", "turns.start", params)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }
    assert_eq!(
        service
            .handle("a", "snapshot.get", json!({"fields":["a".repeat(70_000)]}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    assert!(prompts.lock().unwrap().is_empty());
}
#[tokio::test]
async fn selection_smuggling_does_not_reach_factory() {
    let (s, c, _) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let mut p = create();
    p["selection"]["routeId"] = json!("route-work");
    assert!(s.handle("a", "sessions.create", p).await.is_err());
    assert_eq!(c.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn request_cannot_supply_native_arguments_or_paths() {
    let (s, c, _) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let mut p = create();
    p["executable"] = json!("arbitrary");
    assert!(s.handle("a", "sessions.create", p).await.is_err());
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn artifacts_are_read_through_the_public_service_and_never_by_path() {
    let (s, _, _) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let session = s.handle("a", "sessions.create", create()).await.unwrap();
    s.handle(
        "a",
        "turns.start",
        json!({"sessionId":session["id"],"prompt":"hello","requestId":"artifact-check"}),
    )
    .await
    .unwrap();
    let id = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let e = s.handle("a", "events.read", json!({})).await.unwrap();
            if let Some(e) = e["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["payload"]["type"] == "artifact")
            {
                break e["payload"]["artifactId"].clone();
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let p = json!({"sessionId":session["id"],"artifactId":id,"offset":"0","maxBytes":3});
    let c = s.handle("a", "artifacts.read", p.clone()).await.unwrap();
    assert_eq!(c["dataBase64"], "c2Vy");
    assert_eq!(c["nextOffset"], "3");
    assert!(s.handle("b", "artifacts.read", p.clone()).await.is_err());
    let mut bad = p.clone();
    bad["path"] = json!("private-sentinel");
    assert!(s.handle("a", "artifacts.read", bad).await.is_err());
    s.handle("a", "grants.revoke", json!({})).await.unwrap();
    assert!(s.handle("a", "artifacts.read", p).await.is_err());
}
