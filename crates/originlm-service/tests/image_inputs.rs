#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use base64::Engine;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};
type Seen = Arc<Mutex<Vec<(String, u32, u32, usize)>>>;
struct ImageAgent {
    carries: bool,
    seen: Seen,
}
impl Agent for ImageAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let mut seen = self.seen.lock().unwrap();
            for image in cx.images() {
                seen.push((
                    image.media_type.as_str().into(),
                    image.width,
                    image.height,
                    image.bytes.len(),
                ));
            }
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn image_input(&self) -> bool {
        self.carries
    }
}
fn png(width: u32, height: u32, padding: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes.resize(bytes.len() + padding, 0);
    bytes
}
fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn setup(model_image: bool, agent_image: bool) -> (Arc<NativeService>, Seen) {
    let seen: Seen = Arc::default();
    let recorded = seen.clone();
    let service = Arc::new(
        NativeService::new(
            Arc::new(move |_| {
                let mut snapshot = common::fixture();
                if model_image {
                    for model in &mut snapshot.models {
                        let mut capability = model.capabilities[0].clone();
                        capability.id = "input.image".into();
                        model.capabilities.push(capability);
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
            Duration::from_secs(60),
        )
        .with_execution(
            BTreeSet::from(["test".into()]),
            Arc::new(move |_| {
                let seen = recorded.clone();
                Box::pin(async move {
                    Ok(Box::new(ImageAgent {
                        carries: agent_image,
                        seen,
                    }) as Box<dyn Agent>)
                })
            }),
        ),
    );
    for client in ["a", "c"] {
        service
            .register_client(client, ClientPolicy::passive())
            .unwrap();
        service
            .register_execution(
                client,
                ExecutionGrant {
                    connection_ids: BTreeSet::from(["personal".into()]),
                    workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    (service, seen)
}
async fn session(service: &NativeService, client: &str) -> String {
    service
        .handle(client, "discovery.refresh", json!({}))
        .await
        .unwrap();
    service
        .handle(
            client,
            "sessions.create",
            json!({"selection":common::selection(),"workspaceId":"scratch"}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}
async fn put(service: &NativeService, client: &str, session: &str, bytes: &[u8]) -> Result<Value> {
    service
        .handle(
            client,
            "inputs.put",
            json!({"sessionId":session,"mediaType":"image/png","dataBase64":encode(bytes),"displayName":"frame.png"}),
        )
        .await
}
async fn wait_idle(service: &NativeService, client: &str, session: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = service
                .handle(client, "sessions.get", json!({"sessionId":session}))
                .await
                .unwrap()["state"]
                .clone();
            if state != "running" {
                assert_eq!(state, "completed");
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn handshake_advertises_image_input_and_put_method() {
    let (service, _) = setup(true, true);
    let handshake = service
        .handle("a", "runtime.handshake", json!({}))
        .await
        .unwrap();
    assert_eq!(handshake["protocolVersion"], "1.0");
    assert!(
        handshake["methods"]
            .as_array()
            .unwrap()
            .contains(&json!("inputs.put"))
    );
    assert_eq!(
        handshake["capabilities"],
        json!([
            "turn.input.image",
            "session.host_mcp",
            "session.host_mcp.native_tools"
        ])
    );
}
#[tokio::test]
async fn staged_image_reaches_a_capable_adapter_once() {
    let (service, seen) = setup(true, true);
    let id = session(&service, "a").await;
    // Larger than the general 64 KiB params bound, well inside the 4 MiB image limit.
    let bytes = png(1280, 720, 900 * 1024);
    let staged = put(&service, "a", &id, &bytes).await.unwrap();
    assert_eq!(staged["width"], 1280);
    assert_eq!(staged["height"], 720);
    assert_eq!(staged["byteLength"], bytes.len());
    assert!(staged.get("dataBase64").is_none());
    let reference = json!([{"inputId":staged["inputId"]}]);
    service
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":id,"prompt":"describe","requestId":"r1","inputs":reference}),
        )
        .await
        .unwrap();
    wait_idle(&service, "a", &id).await;
    assert_eq!(
        *seen.lock().unwrap(),
        vec![("image/png".to_owned(), 1280, 720, bytes.len())]
    );
    // A started turn consumes its inputs.
    assert_eq!(
        service
            .handle(
                "a",
                "turns.start",
                json!({"sessionId":id,"prompt":"again","requestId":"r2","inputs":reference}),
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    // Text-only turns stay unchanged.
    service
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":id,"prompt":"text","requestId":"r3"}),
        )
        .await
        .unwrap();
    wait_idle(&service, "a", &id).await;
    assert_eq!(seen.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn invalid_puts_are_typed_request_errors() {
    let (service, _) = setup(true, true);
    let id = session(&service, "a").await;
    let valid = encode(&png(4, 4, 0));
    for params in [
        json!({"sessionId":id,"mediaType":"image/gif","dataBase64":valid}),
        json!({"sessionId":id,"mediaType":"image/png","dataBase64":"%%%not-base64%%%"}),
        json!({"sessionId":id,"mediaType":"image/jpeg","dataBase64":valid}),
        json!({"sessionId":id,"mediaType":"image/png","dataBase64":encode(&png(4096, 16, 0))}),
        json!({"sessionId":id,"mediaType":"image/png","dataBase64":valid,"extra":true}),
        json!({"sessionId":id,"mediaType":"image/png"}),
    ] {
        assert_eq!(
            service
                .handle("a", "inputs.put", params)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }
    let oversize = encode(&png(16, 16, MAX_INPUT_IMAGE_BYTES));
    assert_eq!(
        service
            .handle(
                "a",
                "inputs.put",
                json!({"sessionId":id,"mediaType":"image/png","dataBase64":oversize}),
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::FrameTooLarge
    );
    for _ in 0..MAX_INPUTS_PER_SESSION {
        put(&service, "a", &id, &png(4, 4, 0)).await.unwrap();
    }
    assert_eq!(
        put(&service, "a", &id, &png(4, 4, 0))
            .await
            .unwrap_err()
            .code,
        ErrorCode::RateLimited
    );
    let refs = (0..5)
        .map(|n| json!({"inputId":format!("input-{n}")}))
        .collect::<Vec<_>>();
    assert_eq!(
        service
            .handle(
                "a",
                "turns.start",
                json!({"sessionId":id,"prompt":"p","requestId":"r","inputs":refs}),
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
}
#[tokio::test]
async fn inputs_are_owner_and_session_scoped() {
    let (service, seen) = setup(true, true);
    let owned = session(&service, "a").await;
    let other_session = session(&service, "a").await;
    let foreign = session(&service, "c").await;
    // Another client cannot stage into, or reference inputs from, a session it does not own.
    assert_eq!(
        put(&service, "c", &owned, &png(4, 4, 0))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    let staged = put(&service, "a", &owned, &png(4, 4, 0)).await.unwrap();
    let reference = json!([{"inputId":staged["inputId"]}]);
    for (client, id) in [("c", &foreign), ("a", &other_session)] {
        assert_eq!(
            service
                .handle(
                    client,
                    "turns.start",
                    json!({"sessionId":id,"prompt":"p","requestId":"x","inputs":reference}),
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
    }
    // Closing the session expires its staged inputs; revocation expires the owner's inputs.
    service
        .handle("a", "sessions.close", json!({"sessionId":owned}))
        .await
        .unwrap();
    service.revoke_execution("a");
    assert!(
        put(&service, "a", &other_session, &png(4, 4, 0))
            .await
            .is_err()
    );
    assert!(seen.lock().unwrap().is_empty());
}
#[tokio::test]
async fn unsupported_model_or_adapter_fails_the_turn_instead_of_dropping_images() {
    for (model_image, agent_image, message) in [
        (false, true, "Selected model"),
        (true, false, "adapter cannot carry"),
    ] {
        let (service, seen) = setup(model_image, agent_image);
        let id = session(&service, "a").await;
        let staged = put(&service, "a", &id, &png(4, 4, 0)).await.unwrap();
        let reference = json!([{"inputId":staged["inputId"]}]);
        let error = service
            .handle(
                "a",
                "turns.start",
                json!({"sessionId":id,"prompt":"p","requestId":"r","inputs":reference}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(error.message.contains(message), "{}", error.message);
        assert_eq!(
            service
                .handle("a", "sessions.get", json!({"sessionId":id}))
                .await
                .unwrap()["state"],
            "ready"
        );
        assert!(seen.lock().unwrap().is_empty());
        // A rejected turn does not consume the input; the same request ID stays available.
        if model_image {
            continue;
        }
        assert_eq!(
            service
                .handle(
                    "a",
                    "turns.start",
                    json!({"sessionId":id,"prompt":"p","requestId":"r","inputs":reference}),
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
