use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_service::NativeService;
use originlm_transports::native_messaging::*;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
fn service() -> (Arc<NativeService>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = Arc::new(NativeService::new(
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(originlm_discovery::ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: Default::default(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(60),
    ));
    s.register_client("extension", ClientPolicy::passive())
        .unwrap();
    (s, calls)
}
fn frame(v: Value) -> Vec<u8> {
    let b = serde_json::to_vec(&v).unwrap();
    let mut out = (b.len() as u32).to_ne_bytes().to_vec();
    out.extend(b);
    out
}
async fn transact(bytes: Vec<u8>, origin: &str) -> (Vec<Value>, Result<()>, usize) {
    let (s, c) = service();
    let (a, mut b) = tokio::io::duplex(2 * MAX_MESSAGE + 128);
    let (ar, aw) = tokio::io::split(a);
    let o = origin.to_owned();
    let task = tokio::spawn(async move {
        serve_native(ar, aw, s, "extension".into(), &o, &[ORIGIN.into()]).await
    });
    let write = tokio::spawn(async move {
        let _ = b.write_all(&bytes).await;
        let _ = b.shutdown().await;
        let mut out = Vec::new();
        let _ = b.read_to_end(&mut out).await;
        out
    });
    let status = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    let bytes = write.await.unwrap();
    let mut i = 0;
    let mut frames = vec![];
    while i < bytes.len() {
        let n = u32::from_ne_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        frames.push(serde_json::from_slice(&bytes[i..i + n]).unwrap());
        i += n;
    }
    (frames, status, c.load(Ordering::SeqCst))
}
#[tokio::test]
async fn real_service_roundtrip_uses_native_length_prefix_not_text_lines() {
    let bytes = [
        frame(json!({"jsonrpc":"2.0","id":1,"method":"runtime.handshake","params":{}})),
        frame(json!({"jsonrpc":"2.0","id":2,"method":"discovery.refresh","params":{}})),
    ]
    .concat();
    let (out, status, c) = transact(bytes, ORIGIN).await;
    status.unwrap();
    assert_eq!(out[0]["result"]["protocolVersion"], "1.0");
    assert_eq!(c, 1);
    assert_eq!(out.len(), 2);
}
#[tokio::test]
async fn missing_or_unregistered_origins_never_touch_the_service() {
    for origin in [
        "",
        "chrome-extension://pppppppppppppppppppppppppppppppp/",
        "https://example.invalid/",
    ] {
        let (_, status, c) = transact(frame(json!({})), origin).await;
        assert_eq!(status.unwrap_err().code, ErrorCode::LocalAccessDenied);
        assert_eq!(c, 0);
    }
}
#[tokio::test]
async fn oversized_input_is_rejected_before_allocating_payload() {
    let (_, status, c) = transact(((MAX_MESSAGE + 1) as u32).to_ne_bytes().to_vec(), ORIGIN).await;
    assert_eq!(status.unwrap_err().code, ErrorCode::FrameTooLarge);
    assert_eq!(c, 0);
}
#[tokio::test]
async fn partial_header_and_body_are_protocol_errors() {
    for bytes in [vec![1, 0], vec![5, 0, 0, 0, b'{']] {
        let (_, status, _) = transact(bytes, ORIGIN).await;
        assert_eq!(status.unwrap_err().code, ErrorCode::ProtocolIncompatible);
    }
}
#[tokio::test]
async fn invalid_utf8_and_json_fail_without_echoing_payload() {
    for bytes in [vec![1, 0, 0, 0, 255], frame(json!("private-sentinel"))] {
        let (_, status, _) = transact(bytes, ORIGIN).await;
        let e = status.unwrap_err();
        assert!(!e.message.contains("private-sentinel"));
    }
}
#[tokio::test]
async fn multiline_json_remains_one_native_request() {
    let text = b"{\n\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"runtime.handshake\",\"params\":{}}";
    let mut bytes = (text.len() as u32).to_ne_bytes().to_vec();
    bytes.extend(text);
    let (out, status, c) = transact(bytes, ORIGIN).await;
    status.unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(c, 0);
}
