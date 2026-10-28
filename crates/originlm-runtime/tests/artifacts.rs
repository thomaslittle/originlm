use originlm_contracts::*;
use originlm_runtime::*;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
struct Producer;
impl Agent for Producer {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if prompt == "explicit" {
                cx.artifact("result.bin", "application/octet-stream", vec![0, 1, 2, 255])?;
            } else if prompt == "bad" {
                for name in ["../secret", "a/b", "a\\b", ""] {
                    assert!(cx.artifact(name, "text/plain", vec![]).is_err());
                }
                assert!(
                    cx.artifact("safe", "text/plain\nX: header", vec![])
                        .is_err()
                );
            } else if prompt == "forged" {
                assert!(
                    cx.emit(EventPayload::Artifact {
                        artifact_id: "fake".into(),
                        media_type: "text/plain".into(),
                        size: "1".into()
                    })
                    .is_err()
                );
            } else {
                cx.emit(EventPayload::TextDelta {
                    text: "Hello 🌍".into(),
                })?;
            }
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
async fn execute(prompt: &str) -> (Runtime, Session, Value) {
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "a",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: "m".into(),
                revision: "1".into(),
                options: Default::default(),
            },
            Box::new(Producer),
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start("a", &session.id, prompt.into(), "request".into())
        .await
        .unwrap();
    let events = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let e = runtime.events("a", Some(&session.id), None).unwrap();
            if e["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["payload"]["type"] == "final")
            {
                break e;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    (runtime, session, events)
}
fn artifact(events: &Value) -> String {
    events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["payload"]["type"] == "artifact")
        .expect("a real owned artifact event")["payload"]["artifactId"]
        .as_str()
        .unwrap()
        .into()
}
#[tokio::test]
async fn completed_text_has_owner_scoped_chunked_result() {
    let (r, s, e) = execute("text").await;
    let id = artifact(&e);
    let c = r.artifact("a", &s.id, &id, 0, 5).unwrap();
    assert_eq!(c.data_base64, "SGVsbG8=");
    assert!(!c.eof);
    assert_eq!(c.total_bytes, "10");
    let rest = r.artifact("a", &s.id, &id, 5, 20).unwrap();
    assert!(rest.eof);
    assert_eq!(rest.next_offset, "10");
    assert_eq!(
        r.artifact("b", &s.id, &id, 0, 5).unwrap_err().code,
        ErrorCode::NotFound
    );
}
#[tokio::test]
async fn binary_artifacts_are_not_forced_through_utf8() {
    let (r, s, e) = execute("explicit").await;
    let c = r.artifact("a", &s.id, &artifact(&e), 0, 100).unwrap();
    assert_eq!(c.data_base64, "AAEC/w==");
    assert_eq!(c.media_type, "application/octet-stream");
}
#[tokio::test]
async fn chunk_limits_offsets_and_close_are_enforced() {
    let (r, s, e) = execute("text").await;
    let id = artifact(&e);
    for (offset, max) in [(0, 0), (0, 49153), (11, 10)] {
        assert!(r.artifact("a", &s.id, &id, offset, max).is_err());
    }
    r.close("a", &s.id).await.unwrap();
    assert!(r.artifact("a", &s.id, &id, 0, 10).is_err());
}
#[tokio::test]
async fn revocation_removes_owned_artifacts() {
    let (r, s, e) = execute("text").await;
    let id = artifact(&e);
    r.revoke("a");
    assert_eq!(
        r.artifact("a", &s.id, &id, 0, 10).unwrap_err().code,
        ErrorCode::NotFound
    );
}
#[tokio::test]
async fn invalid_names_and_media_types_are_rejected() {
    let (_, _, e) = execute("bad").await;
    assert!(!e.to_string().contains("error"));
}
#[tokio::test]
async fn unregistered_artifact_events_cannot_be_forged() {
    let (_, _, e) = execute("forged").await;
    assert!(!e.to_string().contains("fake"));
    assert!(!e.to_string().contains("error"));
}
