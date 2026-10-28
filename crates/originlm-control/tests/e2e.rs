use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_service::NativeService;
use originlm_transports::local_ipc::Broker;
use std::{process::Command, sync::Arc, time::Duration};

fn endpoint() -> String {
    if cfg!(windows) {
        format!(
            "originlm-control-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    } else {
        std::env::temp_dir()
            .join(format!(
                "originlm-control-{}-{}.sock",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .to_string_lossy()
            .into_owned()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_control_center_uses_only_the_public_authenticated_api() {
    let directory = tempfile::tempdir().unwrap();
    let address = endpoint();
    let service = Arc::new(NativeService::new(
        Arc::new(|_| {
            Ok(originlm_discovery::ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: Default::default(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(1),
    ));
    let broker = Broker::start(service, &address).await.unwrap();
    let token = broker
        .authorize_shared(
            ClientPolicy {
                admin: true,
                ..ClientPolicy::passive()
            },
            None,
        )
        .unwrap();
    let token_file = directory.path().join("broker-token.json");
    let descriptor = serde_json::json!({
        "protocolVersion":"1.0",
        "address":address,
        "token":token,
        "expiresInSeconds":0,
        "independent":true,
    });
    std::fs::write(&token_file, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_originlm-control"))
        .arg("--token-file")
        .arg(&token_file)
        .arg("--once")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Revision"));
    assert!(stdout.contains("No supported AI installation"));
    assert!(!stdout.contains(&token));
    assert_eq!(broker.active_connections(), 0);
    broker.close().await;
}

#[cfg(unix)]
fn fake_broker(replies: Vec<String>) -> (String, std::thread::JoinHandle<()>, tempfile::TempDir) {
    use std::io::{BufRead, BufReader, Write};
    let directory = tempfile::tempdir().unwrap();
    let address = directory.path().join("broker.sock");
    let listener = std::os::unix::net::UnixListener::bind(&address).unwrap();
    let task = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        for reply in replies {
            let mut request = String::new();
            if reader.read_line(&mut request).unwrap_or(0) == 0 {
                break;
            }
            if reader.get_mut().write_all(reply.as_bytes()).is_err() {
                break;
            }
        }
    });
    (address.to_string_lossy().into_owned(), task, directory)
}

#[test]
#[cfg(unix)]
fn rejects_unauthenticated_or_incompatible_brokers() {
    for replies in [
        vec!["{\"id\":1,\"result\":{\"authenticated\":false}}\n".into()],
        vec![
            "{\"id\":1,\"result\":{\"authenticated\":true}}\n".into(),
            "{\"id\":2,\"result\":{\"protocolVersion\":\"2.0\",\"methods\":[]}}\n".into(),
        ],
    ] {
        let (address, task, _directory) = fake_broker(replies);
        assert!(originlm_control::BrokerClient::connect(&address, &"a".repeat(64)).is_err());
        task.join().unwrap();
    }
}

#[test]
#[cfg(unix)]
fn rejects_oversized_response_without_waiting_for_newline() {
    let (address, task, _directory) = fake_broker(vec!["x".repeat(1024 * 1024 + 1)]);
    let error = originlm_control::BrokerClient::connect(&address, &"a".repeat(64))
        .err()
        .unwrap();
    assert!(error.contains("frame bound"), "{error}");
    task.join().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_valid_denial_preserves_the_connection_and_typed_error() {
    let address = endpoint();
    let service = Arc::new(NativeService::new(
        Arc::new(|_| {
            Ok(originlm_discovery::ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: Default::default(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(1),
    ));
    let broker = Broker::start(service, &address).await.unwrap();
    let token = broker
        .authorize_shared(ClientPolicy::passive(), None)
        .unwrap();
    tokio::task::spawn_blocking(move || {
        let mut client = originlm_control::BrokerClient::connect(&address, &token).unwrap();
        let error = client
            .call_typed("apps.list", serde_json::json!({}))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::LocalAccessDenied);
        assert!(!error.retryable);
        assert!(
            client
                .call_typed("snapshot.get", serde_json::json!({}))
                .is_ok()
        );
        client.close();
    })
    .await
    .unwrap();
    broker.close().await;
}

#[cfg(unix)]
#[test]
fn typed_remote_errors_keep_retry_hints_without_remote_message_content() {
    let (address, task, _directory) = fake_broker(vec![
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"authenticated\":true}}\n".into(),
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"protocolVersion\":\"1.0\",\"methods\":[]}}\n".into(),
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"error\":{\"data\":{\"code\":\"RATE_LIMITED\",\"message\":\"secret fixture marker\",\"retryable\":true,\"retryAfterMs\":2500}}}\n".into(),
        "{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{\"ok\":true}}\n".into(),
    ]);
    let mut client = originlm_control::BrokerClient::connect(&address, &"a".repeat(64)).unwrap();
    let error = client
        .call_typed("fixture.limited", serde_json::json!({}))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(error.retry_after_ms, Some(2500));
    assert!(error.retryable);
    assert!(!error.message.contains("secret fixture marker"));
    assert_eq!(
        client
            .call_typed("fixture.next", serde_json::json!({}))
            .unwrap()["ok"],
        true
    );
    drop(client);
    task.join().unwrap();
}
