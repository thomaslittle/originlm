use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::NativeService;
use originlm_transports::{StdioOptions, serve_io};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn request(id: Value, method: &str, params: Value) -> String {
    format!(
        "{}\n",
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    )
}
fn handshake() -> String {
    request(
        json!(1),
        "runtime.handshake",
        json!({"protocolVersion":"1.0"}),
    )
}
async fn exchange(bytes: &[u8], options: StdioOptions) -> (Vec<Value>, usize) {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let service = Arc::new(NativeService::new(
        Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(60));
            Ok(ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: BTreeMap::new(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(60),
    ));
    service
        .register_client("sdk", ClientPolicy::passive())
        .unwrap();
    let (mut client, server) = tokio::io::duplex(2_097_152);
    let (r, w) = tokio::io::split(server);
    let worker = tokio::spawn(async move { serve_io(r, w, service, "sdk".into(), options).await });
    for chunk in bytes.chunks(7) {
        client.write_all(chunk).await.unwrap();
    }
    client.shutdown().await.unwrap();
    let mut output = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), client.read_to_end(&mut output))
        .await
        .unwrap()
        .unwrap();
    let _ = worker.await.unwrap();
    let replies = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (replies, count.load(Ordering::SeqCst))
}
#[tokio::test]
async fn real_service_negotiates_and_returns_empty_live_snapshot() {
    let input = handshake() + &request(json!(2), "snapshot.get", json!({}));
    let (out, count) = exchange(input.as_bytes(), StdioOptions::default()).await;
    assert_eq!(out[0]["result"]["protocolVersion"], "1.0");
    assert_eq!(out[1]["result"]["mode"], "live");
    assert!(
        out[1]["result"]["connections"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(count, 0);
}
#[tokio::test]
async fn handshake_is_required_before_a_scan() {
    let (out, count) = exchange(
        request(json!(1), "discovery.refresh", json!({})).as_bytes(),
        StdioOptions::default(),
    )
    .await;
    assert_eq!(out[0]["error"]["data"]["code"], "PROTOCOL_INCOMPATIBLE");
    assert_eq!(count, 0);
}
#[tokio::test]
async fn notifications_cannot_trigger_work() {
    let input =
        handshake() + "{\"jsonrpc\":\"2.0\",\"method\":\"discovery.refresh\",\"params\":{}}\n";
    let (out, count) = exchange(input.as_bytes(), StdioOptions::default()).await;
    assert_eq!(out.len(), 1);
    assert_eq!(count, 0);
}
#[tokio::test]
async fn malformed_frames_and_batches_have_jsonrpc_errors() {
    for raw in [
        b"{not-json}\n".as_slice(),
        b"[1,2]\n",
        b"\xff\n",
        b"{\"jsonrpc\":\"2.0\",\"id\":false}\n",
    ] {
        let (out, _) = exchange(raw, StdioOptions::default()).await;
        assert!(out[0]["error"]["code"].as_i64().unwrap() <= -32600);
        assert_eq!(out[0]["id"], Value::Null);
    }
}
#[tokio::test]
async fn unfinished_and_oversized_lines_fail_closed() {
    let (partial, _) = exchange(b"{\"jsonrpc\":", StdioOptions::default()).await;
    assert_eq!(partial[0]["error"]["code"], -32700);
    let (large, count) = exchange(
        &[b'x'; 1100],
        StdioOptions {
            max_frame_bytes: 1024,
            ..StdioOptions::default()
        },
    )
    .await;
    assert_eq!(large[0]["error"]["data"]["code"], "FRAME_TOO_LARGE");
    assert_eq!(count, 0);
}
#[tokio::test]
async fn responses_are_correlated_without_blocking_cached_reads() {
    let input = handshake()
        + &request(json!(2), "discovery.refresh", json!({}))
        + &request(json!(3), "snapshot.get", json!({}));
    let (out, count) = exchange(input.as_bytes(), StdioOptions::default()).await;
    assert_eq!(
        out.iter().map(|v| v["id"].clone()).collect::<Vec<_>>(),
        vec![json!(1), json!(3), json!(2)]
    );
    assert_eq!(count, 1);
}
#[tokio::test]
async fn string_ids_and_typed_domain_errors_are_preserved() {
    let input = handshake() + &request(json!("alpha"), "snapshot.get", json!({"clientId":"other"}));
    let (out, _) = exchange(input.as_bytes(), StdioOptions::default()).await;
    assert_eq!(out[1]["id"], "alpha");
    assert_eq!(out[1]["error"]["data"]["code"], "INVALID_REQUEST");
}
#[tokio::test]
async fn protocol_mismatch_never_unlocks_discovery() {
    let input = request(
        json!(1),
        "runtime.handshake",
        json!({"protocolVersion":"9.0"}),
    ) + &request(json!(2), "discovery.refresh", json!({}));
    let (out, count) = exchange(input.as_bytes(), StdioOptions::default()).await;
    assert!(out.iter().all(|reply| reply.get("error").is_some()));
    assert_eq!(count, 0);
}
