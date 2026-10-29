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
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
struct Delayed(Arc<tokio::sync::Notify>);
impl Agent for Delayed {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.0.notified().await;
            cx.emit(EventPayload::TextDelta {
                text: "late output".into(),
            })?;
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
#[tokio::test]
async fn cached_metadata_age_does_not_cancel_an_already_authorized_turn() {
    let stale = Arc::new(AtomicBool::new(false));
    let check = stale.clone();
    let notify = Arc::new(tokio::sync::Notify::new());
    let signal = notify.clone();
    let service = NativeService::new(
        Arc::new(move |_| {
            let mut snapshot = common::fixture();
            if check.load(Ordering::SeqCst) {
                for c in &mut snapshot.connections {
                    c.auth.mark_stale();
                }
            }
            let locations = snapshot
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
                snapshot,
                locations,
                metadata_probes: 0,
            })
        }),
        Duration::ZERO,
    )
    .with_execution(
        BTreeSet::from(["test".into()]),
        Arc::new(move |_| {
            let n = signal.clone();
            Box::pin(async { Ok(Box::new(Delayed(n)) as Box<dyn Agent>) })
        }),
    );
    service
        .register_client("a", ClientPolicy::passive())
        .unwrap();
    service
        .register_execution(
            "a",
            ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("default".into(), std::env::temp_dir())]),
                ..Default::default()
            },
        )
        .unwrap();
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let s = service
        .handle(
            "a",
            "sessions.create",
            json!({"selection":common::selection(),"workspaceId":"default"}),
        )
        .await
        .unwrap();
    service
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":s["id"],"prompt":"x","requestId":"one"}),
        )
        .await
        .unwrap();
    stale.store(true, Ordering::SeqCst);
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    notify.notify_one();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let x = service
                .handle("a", "sessions.get", json!({"sessionId":s["id"]}))
                .await
                .unwrap();
            if x["state"] != "running" {
                assert_eq!(x["state"], "completed");
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        service
            .handle(
                "a",
                "turns.start",
                json!({"sessionId":s["id"],"prompt":"x","requestId":"two"})
            )
            .await
            .is_ok(),
        "a stale sign-in stands until the client reports it signed out"
    );
}
