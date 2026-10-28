mod support;
use originlm_transports::serve_io;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[tokio::test]
async fn native_stdio_subscriptions_stream_after_ack_and_unsubscribe() {
    let (mut client, server) = tokio::io::duplex(65536);
    let (input, output) = tokio::io::split(server);
    let s = support::service();
    let server = tokio::spawn(serve_io(
        input,
        output,
        s,
        "owner".into(),
        Default::default(),
    ));
    let mut reader = BufReader::new(&mut client);
    async fn rpc(
        reader: &mut BufReader<&mut tokio::io::DuplexStream>,
        id: u64,
        method: &str,
        params: Value,
    ) -> Value {
        reader
            .get_mut()
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }
    assert!(
        rpc(
            &mut reader,
            1,
            "runtime.handshake",
            json!({"protocolVersion":"1.0"})
        )
        .await
        .get("result")
        .is_some()
    );
    rpc(&mut reader, 2, "discovery.refresh", json!({})).await;
    let session =
        rpc(&mut reader, 3, "sessions.create", support::create()).await["result"]["id"].clone();
    let sub = rpc(
        &mut reader,
        4,
        "events.subscribe",
        json!({"sessionId":session}),
    )
    .await;
    assert!(sub["result"]["subscriptionId"].is_string());
    let turn = rpc(
        &mut reader,
        5,
        "turns.start",
        json!({"sessionId":session,"prompt":"hello","requestId":"one"}),
    )
    .await;
    assert!(turn["result"]["turnId"].is_string());
    let mut received = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let e: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(e["method"], "events.event");
            received.push(e.clone());
            if e["params"]["event"]["payload"]["type"] == "final" {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        received
            .iter()
            .any(|e| e["params"]["event"]["payload"]["text"] == "transport-event")
    );
    let out = rpc(
        &mut reader,
        6,
        "events.unsubscribe",
        json!({"subscriptionId":sub["result"]["subscriptionId"]}),
    )
    .await;
    assert_eq!(out["result"]["unsubscribed"], true);
    reader.get_mut().shutdown().await.unwrap();
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn native_sse_is_an_authorized_service_stream() {
    let s = support::service();
    let host = originlm_transports::http::start(
        s,
        originlm_transports::http::HttpOptions {
            port: 0,
            policy: originlm_core::ClientPolicy::passive(),
            execution: Some(support::grant()),
            assets: None,
        },
    )
    .await
    .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let token = client
        .post(format!("{}/v1/pair", host.url))
        .json(&json!({"code":host.pairing_code}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    async fn rpc(
        client: &reqwest::Client,
        url: &str,
        token: &str,
        method: &str,
        params: Value,
    ) -> Value {
        client
            .post(format!("{url}/v1/rpc"))
            .bearer_auth(token)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    rpc(
        &client,
        &host.url,
        &token,
        "runtime.handshake",
        json!({"protocolVersion":"1.0"}),
    )
    .await;
    rpc(&client, &host.url, &token, "discovery.refresh", json!({})).await;
    let session = rpc(
        &client,
        &host.url,
        &token,
        "sessions.create",
        support::create(),
    )
    .await["result"]["id"]
        .clone();
    let mut stream = client
        .get(format!(
            "{}/v1/events?sessionId={}",
            host.url,
            session.as_str().unwrap()
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    rpc(
        &client,
        &host.url,
        &token,
        "turns.start",
        json!({"sessionId":session,"prompt":"hello","requestId":"one"}),
    )
    .await;
    let mut data = String::new();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !data.contains("transport-event") {
            let chunk = stream.chunk().await.unwrap().unwrap();
            data.push_str(&String::from_utf8_lossy(&chunk));
        }
    })
    .await
    .unwrap();
    assert!(data.contains("event: update"));
    drop(stream);
    host.close().await;
}
