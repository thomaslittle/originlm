use originlm_contracts::*;
use originlm_runtime::{Agent, BoxFuture, Runtime, TurnContext};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct RetryCleanup {
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    delayed: bool,
}
impl Drop for RetryCleanup {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl Agent for RetryCleanup {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        _: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {
            Err(DomainError::new(
                ErrorCode::ProviderUnavailable,
                "synthetic turn failed",
            ))
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                if self.delayed {
                    std::future::pending::<()>().await;
                }
                return Err(DomainError::new(
                    ErrorCode::ProviderUnavailable,
                    "synthetic cleanup failed",
                ));
            }
            Ok(())
        })
    }
}
fn insert(r: &Runtime, calls: Arc<AtomicUsize>, drops: Arc<AtomicUsize>, delayed: bool) -> Session {
    r.insert(
        "owner",
        "connection".into(),
        Selection {
            route_id: "r".into(),
            model_id: "m".into(),
            revision: "v".into(),
            options: BTreeMap::new(),
        },
        Box::new(RetryCleanup {
            calls,
            drops,
            delayed,
        }),
        Arc::new(|| Ok(())),
    )
    .unwrap()
}
#[tokio::test]
async fn failed_or_timed_out_close_retains_owned_driver_for_retry_and_pruning() {
    for delayed in [false, true] {
        let runtime = Runtime::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let session = insert(&runtime, calls.clone(), drops.clone(), delayed);
        assert_eq!(
            runtime.close("owner", &session.id).await.unwrap_err().code,
            if delayed {
                ErrorCode::Timeout
            } else {
                ErrorCode::ProviderUnavailable
            }
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(
            runtime.get("owner", &session.id).unwrap().state,
            SessionState::Closed
        );
        assert!(
            runtime
                .start("owner", &session.id, "no".into(), "request".into())
                .await
                .is_err()
        );
        let _other = insert(
            &runtime,
            Arc::new(AtomicUsize::new(1)),
            Arc::new(AtomicUsize::new(0)),
            false,
        );
        runtime.close("owner", &session.id).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        runtime.close("owner", &session.id).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
#[tokio::test]
async fn failed_turn_cleanup_retains_driver_until_explicit_close_retry() {
    let runtime = Runtime::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let session = insert(&runtime, calls.clone(), drops.clone(), false);
    runtime
        .start("owner", &session.id, "fail".into(), "request".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while runtime.get("owner", &session.id).unwrap().state == SessionState::Running {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    runtime.close("owner", &session.id).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
