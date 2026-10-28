use originlm_contracts::*;
use originlm_runtime::*;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
struct Flood;
impl Agent for Flood {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for _ in 0..180 {
                cx.emit(EventPayload::TextDelta {
                    text: "x".repeat(2048),
                })?;
            }
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
fn add(r: &Runtime) -> Session {
    r.insert(
        "owner",
        "c".into(),
        Selection {
            route_id: "r".into(),
            model_id: "m".into(),
            revision: "1".into(),
            options: Default::default(),
        },
        Box::new(Flood),
        Arc::new(|| Ok(())),
    )
    .unwrap()
}
#[tokio::test]
async fn replay_pages_are_bounded_without_losing_events() {
    let r = Runtime::new();
    let s = add(&r);
    r.start("owner", &s.id, "x".into(), "one".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while r.get("owner", &s.id).unwrap().state == SessionState::Running {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let first = r.events("owner", None, None).unwrap();
    assert!(serde_json::to_vec(&first).unwrap().len() < 300_000);
    assert_eq!(first["hasMore"], true);
    let next = r
        .events(
            "owner",
            None,
            Some(first["cursor"].as_str().unwrap().parse().unwrap()),
        )
        .unwrap();
    let count = first["events"]
        .as_array()
        .unwrap()
        .iter()
        .chain(next["events"].as_array().unwrap())
        .filter(|e| e["payload"]["type"] == "text_delta")
        .count();
    assert_eq!(count, 180);
}
#[tokio::test]
async fn closed_sessions_do_not_exhaust_active_capacity() {
    let r = Runtime::new();
    for _ in 0..32 {
        let s = add(&r);
        r.close("owner", &s.id).await.unwrap();
    }
}
