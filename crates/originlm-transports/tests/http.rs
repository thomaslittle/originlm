use originlm_contracts::{DataMode, Snapshot};
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::NativeService;
use originlm_transports::http::*;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
async fn setup() -> (HttpServer, Arc<AtomicUsize>) {
    setup_with_assets(None).await
}
async fn setup_with_assets(assets: Option<std::path::PathBuf>) -> (HttpServer, Arc<AtomicUsize>) {
    setup_config(assets, Duration::from_secs(30)).await
}
async fn setup_config(
    assets: Option<std::path::PathBuf>,
    ttl: Duration,
) -> (HttpServer, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let reads = count.clone();
    let service = Arc::new(NativeService::new(
        Arc::new(move |_| {
            let revision = reads.fetch_add(1, Ordering::SeqCst);
            let mut snapshot = Snapshot::empty(DataMode::Live);
            snapshot.providers.push(originlm_contracts::Provider {
                id: "fixture".into(),
                name: format!("Fixture {revision}"),
                kind: "fixture".into(),
            });
            Ok(ScanResult {
                snapshot,
                locations: BTreeMap::new(),
                metadata_probes: 0,
            })
        }),
        ttl,
    ));
    (
        start(
            service,
            HttpOptions {
                port: 0,
                policy: ClientPolicy::passive(),
                execution: None,
                assets,
            },
        )
        .await
        .unwrap(),
        count,
    )
}
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}
async fn pair(server: &HttpServer) -> String {
    let reply: Value = client()
        .post(format!("{}/v1/pair", server.url))
        .json(&json!({"code":server.pairing_code}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    reply["token"].as_str().unwrap().into()
}
async fn rpc(s: &HttpServer, token: &str, method: &str, params: Value) -> Value {
    client()
        .post(format!("{}/v1/rpc", s.url))
        .bearer_auth(token)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
#[tokio::test]
async fn no_bootstrap_token_or_automatic_scan() {
    let (s, count) = setup().await;
    let result = client()
        .get(format!("{}/runtime/info", s.url))
        .send()
        .await
        .unwrap();
    let body = result.text().await.unwrap();
    assert!(!body.contains(&s.pairing_code));
    assert!(!body.contains("token"));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(
        client()
            .get(format!("{}/v1/snapshot", s.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    s.close().await;
}
#[tokio::test]
async fn same_origin_pairing_code_endpoint_returns_current_code_without_consuming_it() {
    let (s, _) = setup().await;
    let response = client()
        .get(format!("{}/runtime/pairing-code", s.url))
        .header("Origin", &s.url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["code"], s.pairing_code);
    assert!(
        body["expiresInSeconds"]
            .as_u64()
            .is_some_and(|n| (1..=300).contains(&n))
    );

    let denied = client()
        .get(format!("{}/runtime/pairing-code", s.url))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);

    let rotated = s.issue_pairing_code().unwrap();
    let refreshed: Value = client()
        .get(format!("{}/runtime/pairing-code", s.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(refreshed["code"], rotated);
    let redeemed = client()
        .post(format!("{}/v1/pair", s.url))
        .json(&json!({"code":rotated}))
        .send()
        .await
        .unwrap();
    assert_eq!(redeemed.status(), 200);
    let consumed = client()
        .get(format!("{}/runtime/pairing-code", s.url))
        .send()
        .await
        .unwrap();
    assert_eq!(consumed.status(), 403);
    let denied_issue = client()
        .post(format!("{}/runtime/pairing-code", s.url))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(denied_issue.status(), 403);
    let issued: Value = client()
        .post(format!("{}/runtime/pairing-code", s.url))
        .header("Origin", &s.url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(issued["code"], rotated);
    assert_eq!(issued["expiresInSeconds"], 300);
    let newly_active: Value = client()
        .get(format!("{}/runtime/pairing-code", s.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(newly_active["code"], issued["code"]);
    s.close().await;
}
#[tokio::test]
async fn single_use_pairing_and_native_rpc() {
    let (s, count) = setup().await;
    let token = pair(&s).await;
    let denied = client()
        .post(format!("{}/v1/pair", s.url))
        .json(&json!({"code":s.pairing_code}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    let h = rpc(
        &s,
        &token,
        "runtime.handshake",
        json!({"protocolVersion":"1.0"}),
    )
    .await;
    assert_eq!(h["result"]["protocolVersion"], "1.0");
    rpc(&s, &token, "discovery.refresh", json!({"level":"passive"})).await;
    let snapshot = rpc(&s, &token, "snapshot.get", json!({})).await;
    assert_eq!(snapshot["result"]["mode"], "live");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    s.close().await;
}
#[tokio::test]
async fn host_origin_and_query_credentials_are_rejected() {
    let (s, _) = setup().await;
    let token = pair(&s).await;
    for (header, value) in [("origin", "https://evil.example"), ("host", "evil.example")] {
        let r = client()
            .get(format!("{}/v1/snapshot", s.url))
            .bearer_auth(&token)
            .header(header, value)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
    }
    let r = client()
        .get(format!("{}/v1/snapshot?token={token}", s.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    s.close().await;
}
#[tokio::test]
async fn pairing_cannot_upgrade_grants() {
    let (s, count) = setup().await;
    let bad = client()
        .post(format!("{}/v1/pair", s.url))
        .json(&json!({"code":s.pairing_code,"execution":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    s.close().await;
}
#[tokio::test]
async fn foreign_origin_cannot_redeem_a_valid_code() {
    let (s, _) = setup().await;
    let result = client()
        .post(format!("{}/v1/pair", s.url))
        .header("Origin", "https://evil.example")
        .json(&json!({"code":s.pairing_code}))
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), 403);
    assert!(!pair(&s).await.is_empty());
    s.close().await;
}
#[tokio::test]
async fn malformed_requests_and_body_limits_are_enforced() {
    let (s, _) = setup().await;
    let token = pair(&s).await;
    let bad = client()
        .post(format!("{}/v1/rpc", s.url))
        .bearer_auth(&token)
        .header("Content-Type", "application/json")
        .body("x".repeat(originlm_contracts::MAX_REQUEST_FRAME_BYTES + 1))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 413);
    // Only /v1/rpc is sized for staged image inputs; other request bodies keep the 1 MiB bound.
    let pairing = client()
        .post(format!("{}/v1/pair", s.url))
        .header("Content-Type", "application/json")
        .body("x".repeat(1_048_577))
        .send()
        .await
        .unwrap();
    assert_eq!(pairing.status(), 413);
    let malformed = rpc(&s, &token, "runtime.handshake", json!([])).await;
    assert!(malformed.get("error").is_some());
    s.close().await;
}

#[tokio::test]
async fn repeated_pair_and_revoke_does_not_exhaust_broker_client_slots() {
    let (s, _) = setup().await;
    for _ in 0..80 {
        let code = s.issue_pairing_code().unwrap();
        let response: Value = client()
            .post(format!("{}/v1/pair", s.url))
            .json(&json!({"code":code}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let token = response["token"]
            .as_str()
            .expect("closed clients must release their registry slot");
        rpc(
            &s,
            token,
            "runtime.handshake",
            json!({"protocolVersion":"1.0"}),
        )
        .await;
        assert_eq!(
            rpc(&s, token, "grants.revoke", json!({})).await["result"]["revoked"],
            true
        );
    }
    s.close().await;
}

fn asset_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("originlm-assets-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><title>spa</title>").unwrap();
    std::fs::write(dir.join("x.woff2"), b"wOF2\x00\x01\x02").unwrap();
    dir
}
async fn raw_status(server: &HttpServer, target: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let addr = server.url.trim_start_matches("http://");
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).await.unwrap();
    reply.lines().next().unwrap_or_default().to_string()
}
#[tokio::test]
async fn client_routes_serve_index_and_everything_else_stays_strict() {
    let dir = asset_dir("routes");
    let (s, _) = setup_with_assets(Some(dir.clone())).await;
    let index = "<!doctype html><title>spa</title>";
    for route in [
        "/",
        "/overview",
        "/connections",
        "/models",
        "/usage",
        "/selector-lab",
        "/playground",
        "/diagnostics",
    ] {
        let response = client()
            .get(format!("{}{route}", s.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "{route}");
        assert_eq!(
            response.headers()["content-type"],
            "text/html; charset=utf-8"
        );
        assert_eq!(response.text().await.unwrap(), index, "{route}");
    }
    let head = client()
        .head(format!("{}/connections", s.url))
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-type"], "text/html; charset=utf-8");
    for route in [
        "/nope",
        "/connections/",
        "/connections/extra",
        "/Connections",
        "/overview.txt",
    ] {
        let response = client()
            .get(format!("{}{route}", s.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "{route}");
    }
    // reqwest normalizes dot segments, so send the traversal target on the wire.
    assert!(raw_status(&s, "/../connections").await.contains(" 404 "));
    assert!(raw_status(&s, "/connections").await.contains(" 200 "));
    s.close().await;
    let _ = std::fs::remove_dir_all(dir);
}
#[tokio::test]
async fn variable_font_assets_are_served_as_woff2() {
    let dir = asset_dir("font");
    let (s, _) = setup_with_assets(Some(dir.clone())).await;
    let response = client()
        .get(format!("{}/x.woff2", s.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "font/woff2");
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        b"wOF2\x00\x01\x02"
    );
    let missing = client()
        .get(format!("{}/missing.woff2", s.url))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    s.close().await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn inventory_sse_preserves_scope_and_rejects_session_mixing() {
    let (server, _) = setup_config(None, Duration::ZERO).await;
    let token = pair(&server).await;
    let handshake = rpc(
        &server,
        &token,
        "runtime.handshake",
        json!({"protocolVersion":"1.0"}),
    )
    .await;
    assert_eq!(handshake["result"]["protocolVersion"], "1.0");
    // Reads are inert: seed the inventory explicitly before taking its cursor.
    let initial = rpc(
        &server,
        &token,
        "discovery.refresh",
        json!({"level":"passive"}),
    )
    .await;
    assert!(initial.get("result").is_some());
    let before = rpc(&server, &token, "snapshot.get", json!({})).await;
    let cursor = before["result"]["cursor"].as_str().unwrap();
    assert!(cursor.contains(':'));
    let refreshed = rpc(
        &server,
        &token,
        "discovery.refresh",
        json!({"level":"passive"}),
    )
    .await;
    assert!(refreshed.get("result").is_some());
    let mut response = client()
        .get(format!(
            "{}/v1/events?scope=inventory&after={cursor}",
            server.url
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let first = response.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("event: update"));
    drop(response);
    for query in ["scope=unknown", "scope=inventory&sessionId=unowned"] {
        let response = client()
            .get(format!("{}/v1/events?{query}", server.url))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    server.close().await;
}
