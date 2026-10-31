#![cfg(unix)]
//! Synthetic broker peer: no provider, credential, or real network traffic.

use originlm_contracts::ErrorCode;
use originlm_control::BrokerClient;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[test]
fn cancellation_interrupts_a_blocked_write() {
    let directory = tempfile::tempdir().unwrap();
    let address = directory.path().join("broker.sock");
    let listener = UnixListener::bind(&address).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap() == 0 {
                break;
            }
            let request: Value = serde_json::from_str(line.trim()).unwrap();
            let method = request["method"].as_str().unwrap();
            let result = match method {
                "broker.authenticate" => json!({"authenticated": true}),
                "runtime.handshake" => json!({"protocolVersion": "1.0", "methods": []}),
                other => panic!("unexpected method before block: {other}"),
            };
            let response = json!({"jsonrpc": "2.0", "id": request["id"], "result": result});
            writeln!(writer, "{response}").unwrap();
            writer.flush().unwrap();
            if method == "runtime.handshake" {
                break;
            }
        }
        // Deliberately stop reading. A frame larger than the socket buffer now
        // blocks the peer's write until cancellation aborts the transaction.
        while !worker_stop.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(10));
        }
        drop(reader);
        drop(writer);
    });

    let token = "a".repeat(64);
    let mut client =
        BrokerClient::connect(&address.to_string_lossy(), &token).expect("synthetic handshake");
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let stopper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        signal.store(true, Ordering::Release);
    });
    let started = Instant::now();
    let result = client.call_typed_with(
        "blocked.write",
        json!({"blob": "x".repeat(900 * 1024)}),
        Duration::from_secs(10),
        Some(&cancel),
    );
    let elapsed = started.elapsed();
    stopper.join().unwrap();
    let error = result.expect_err("cancellation must win over a blocked write");
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert!(
        elapsed < Duration::from_secs(2),
        "blocked write ignored cancellation for {elapsed:?}"
    );
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
}
