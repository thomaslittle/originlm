use originlm_contracts::*;
use originlm_runtime::*;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct TestAgent(Arc<AtomicUsize>);
struct DiagnosticFailureAgent;
struct FailingSuspend(bool);
impl Agent for FailingSuspend {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        _: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async { panic!("uncertain session must never execute") })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn native_resumption(&self) -> bool {
        true
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            if self.0 {
                std::future::pending::<()>().await;
            }
            Err(DomainError::new(
                ErrorCode::ProviderUnavailable,
                "fixture failed",
            ))
        })
    }
}
#[tokio::test]
async fn failed_or_timed_out_suspend_never_reenables_an_uncertain_conversation() {
    for timeout in [false, true] {
        let runtime = Runtime::new();
        let session = runtime
            .insert(
                "a",
                "connection".into(),
                Selection {
                    route_id: "r".into(),
                    model_id: "m".into(),
                    revision: "1".into(),
                    options: Default::default(),
                },
                Box::new(FailingSuspend(timeout)),
                Arc::new(|| Ok(())),
            )
            .unwrap();
        assert_eq!(
            runtime.suspend("a", &session.id).await.unwrap_err().code,
            if timeout {
                ErrorCode::Timeout
            } else {
                ErrorCode::ProviderUnavailable
            }
        );
        assert_eq!(
            runtime.get("a", &session.id).unwrap().state,
            SessionState::Error
        );
        assert!(
            runtime
                .start("a", &session.id, "hello".into(), "request".into())
                .await
                .is_err()
        );
        runtime.close("a", &session.id).await.unwrap();
    }
}
impl Agent for TestAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        _: BTreeMap<String, Value>,
        mut cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            if prompt == "approval" {
                let allow = cx
                    .approve("Read fixture".into(), "No real files".into())
                    .await?;
                if !allow {
                    return Err(DomainError::new(ErrorCode::LocalAccessDenied, "Denied"));
                }
            }
            if prompt == "wait" {
                cx.cancelled().await;
                return Err(DomainError::new(ErrorCode::Cancelled, "Cancelled"));
            }
            cx.emit(EventPayload::TextDelta {
                text: "fixture reply".into(),
            })?;
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
impl Agent for DiagnosticFailureAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        _: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {
            Err(DomainError::new(
                ErrorCode::ProviderUnavailable,
                "HTTP 401: Authorization: Bearer private-token-value",
            ))
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
fn setup() -> (Arc<Runtime>, Session, Arc<AtomicUsize>) {
    let r = Arc::new(Runtime::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let s = r
        .insert(
            "a",
            "connection".into(),
            Selection {
                route_id: "route".into(),
                model_id: "model".into(),
                revision: "1".into(),
                options: Default::default(),
            },
            Box::new(TestAgent(calls.clone())),
            Arc::new(|| Ok(())),
        )
        .unwrap();
    (r, s, calls)
}
async fn done(r: &Runtime, s: &Session) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if !matches!(
                r.get("a", &s.id).unwrap().state,
                SessionState::Running | SessionState::AwaitingApproval
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn creation_is_not_generation_and_turn_streams_once() {
    let (r, s, c) = setup();
    assert_eq!(c.load(Ordering::SeqCst), 0);
    let t = r
        .start("a", &s.id, "hello".into(), "request-1".into())
        .await
        .unwrap();
    done(&r, &s).await;
    let events = r.events("a", Some(&s.id), None).unwrap();
    assert!(events.to_string().contains("fixture reply"));
    assert_eq!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["payload"]["type"] == "final")
            .count(),
        1
    );
    assert!(t["turnId"].is_string());
    assert_eq!(c.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn error_events_keep_provider_diagnostics_and_redact_credentials() {
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "owner",
            "connection".into(),
            Selection {
                route_id: "route".into(),
                model_id: "model".into(),
                revision: "1".into(),
                options: Default::default(),
            },
            Box::new(DiagnosticFailureAgent),
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("owner", &session.id, "hello".into(), "request".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if runtime.get("owner", &session.id).unwrap().state != SessionState::Running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let events = runtime.events("owner", Some(&session.id), None).unwrap();
    let error = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["payload"]["type"] == "error")
        .unwrap();
    assert_eq!(error["payload"]["code"], "PROVIDER_UNAVAILABLE");
    assert_eq!(
        error["payload"]["message"],
        "HTTP 401: Authorization: Bearer [REDACTED]"
    );
    assert!(!events.to_string().contains("private-token-value"));
}
#[tokio::test]
async fn session_and_events_are_owner_scoped() {
    let (r, s, _) = setup();
    assert_eq!(r.get("b", &s.id).unwrap_err().code, ErrorCode::NotFound);
    assert!(r.start("b", &s.id, "x".into(), "1".into()).await.is_err());
    assert!(
        r.events("b", None, None).unwrap()["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn cancellation_is_terminal_and_does_not_retry() {
    let (r, s, c) = setup();
    let t = r
        .start("a", &s.id, "wait".into(), "1".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    r.cancel("a", t["turnId"].as_str().unwrap()).await.unwrap();
    done(&r, &s).await;
    assert_eq!(r.get("a", &s.id).unwrap().state, SessionState::Cancelled);
    assert_eq!(c.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn duplicate_request_never_runs_twice() {
    let (r, s, c) = setup();
    r.start("a", &s.id, "hello".into(), "same".into())
        .await
        .unwrap();
    done(&r, &s).await;
    assert_eq!(
        r.start("a", &s.id, "hello".into(), "same".into())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(c.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn approval_is_scoped_and_single_use() {
    let (r, s, _) = setup();
    r.start("a", &s.id, "approval".into(), "1".into())
        .await
        .unwrap();
    let id = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let events = r.events("a", None, None).unwrap();
            if let Some(e) = events["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["payload"]["type"] == "approval_request")
            {
                break e["payload"]["approvalId"].as_str().unwrap().to_owned();
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert!(r.approve("b", &id, true).is_err());
    r.approve("a", &id, false).unwrap();
    assert!(r.approve("a", &id, true).is_err());
    done(&r, &s).await;
    assert!(
        !r.events("a", None, None)
            .unwrap()
            .to_string()
            .contains("fixture reply")
    );
}
#[tokio::test]
async fn revocation_cancels_owned_work_and_hides_results() {
    let (r, s, _) = setup();
    r.start("a", &s.id, "wait".into(), "1".into())
        .await
        .unwrap();
    r.revoke("a");
    assert!(r.get("a", &s.id).is_err());
    assert!(
        r.events("a", None, None).unwrap()["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn close_blocks_further_turns() {
    let (r, s, _) = setup();
    r.close("a", &s.id).await.unwrap();
    assert!(
        r.start("a", &s.id, "hello".into(), "1".into())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn per_turn_guard_is_rechecked() {
    let r = Runtime::new();
    let s = r
        .insert(
            "a",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: "m".into(),
                revision: "1".into(),
                options: Default::default(),
            },
            Box::new(TestAgent(Arc::new(AtomicUsize::new(0)))),
            Arc::new(|| Err(DomainError::new(ErrorCode::AuthRequired, "Changed"))),
        )
        .unwrap();
    assert_eq!(
        r.start("a", &s.id, "x".into(), "1".into())
            .await
            .unwrap_err()
            .code,
        ErrorCode::AuthRequired
    );
}
