use serde_json::{Value, json};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn endpoint() -> String {
    if cfg!(windows) {
        format!(
            "originlm-cli-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    } else {
        std::env::temp_dir()
            .join(format!(
                "originlm-cli-{}-{}.sock",
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

async fn connect(name: &str) -> BufReader<originlm_transports::local_ipc::IpcStream> {
    let stream = originlm_transports::local_ipc::connect(name).await.unwrap();
    BufReader::new(stream)
}

async fn call(
    stream: &mut BufReader<originlm_transports::local_ipc::IpcStream>,
    id: u64,
    method: &str,
    params: Value,
) -> Value {
    stream
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
    tokio::time::timeout(Duration::from_secs(5), stream.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    serde_json::from_str(&line).unwrap()
}

async fn authenticated(
    name: &str,
    token: &str,
) -> BufReader<originlm_transports::local_ipc::IpcStream> {
    let mut stream = connect(name).await;
    let result = call(
        &mut stream,
        1,
        "broker.authenticate",
        json!({"token":token}),
    )
    .await;
    assert_eq!(result["result"]["authenticated"], true);
    let handshake = call(
        &mut stream,
        2,
        "runtime.handshake",
        json!({"protocolVersion":"1.0","clientName":"cli-e2e"}),
    )
    .await;
    assert_eq!(handshake["result"]["protocolVersion"], "1.0");
    stream
}

async fn wait_token(path: &std::path::Path, deadline: Instant) -> Value {
    loop {
        if let Ok(bytes) = std::fs::read(path)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "token file {} was not published",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independently_scoped_app_grants_revoke_one_app_without_the_owner_credential() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().to_path_buf();
    let address = endpoint();
    let admin_file = directory.path().join("admin-token.json");
    let app_a_file = directory.path().join("app-a-token.json");
    let app_b_file = directory.path().join("app-b-token.json");
    let grants_file = directory.path().join("grants.json");
    std::fs::write(
        &grants_file,
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "apps": [
                {
                    "id": "app-a",
                    "tokenFile": app_a_file.to_str().unwrap(),
                    "allowPassiveDiscovery": true,
                    "connectionIds": ["personal"]
                },
                {
                    "id": "app-b",
                    "tokenFile": app_b_file.to_str().unwrap(),
                    "allowPassiveDiscovery": true,
                    "connectionIds": ["work"]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "serve",
            "--ipc",
            &address,
            "--independent",
            "--grants-file",
            grants_file.to_str().unwrap(),
            "--admin-token-file",
            admin_file.to_str().unwrap(),
            "--idle-timeout",
            "30",
            "--home",
            home.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let admin = wait_token(&admin_file, deadline).await;
    let a = wait_token(&app_a_file, deadline).await;
    let b = wait_token(&app_b_file, deadline).await;
    assert_eq!(a["appId"], "app-a");
    assert_eq!(b["appId"], "app-b");
    assert_ne!(a["token"], b["token"]);
    assert_eq!(admin["appId"], "kit-admin");

    let mut admin_client = authenticated(&address, admin["token"].as_str().unwrap()).await;
    let mut a_client = authenticated(&address, a["token"].as_str().unwrap()).await;
    let mut b_client = authenticated(&address, b["token"].as_str().unwrap()).await;
    let apps = call(&mut admin_client, 3, "apps.list", json!({})).await;
    let apps = apps["result"].as_array().unwrap();
    let a_id = apps
        .iter()
        .find(|app| app["app"] == "app-a")
        .and_then(|app| app["id"].as_str())
        .unwrap()
        .to_owned();
    let b_id = apps
        .iter()
        .find(|app| app["app"] == "app-b")
        .and_then(|app| app["id"].as_str())
        .unwrap()
        .to_owned();
    assert_ne!(a_id, b_id);

    let revoked = call(
        &mut admin_client,
        4,
        "apps.revoke",
        json!({"targetClientId": a_id}),
    )
    .await;
    assert_eq!(revoked["result"]["grantsRevoked"], true);
    let a_health = call(&mut a_client, 5, "runtime.health", json!({})).await;
    assert_eq!(a_health["error"]["data"]["code"], "LOCAL_ACCESS_DENIED");
    let mut reconnect = connect(&address).await;
    assert!(
        call(
            &mut reconnect,
            1,
            "broker.authenticate",
            json!({"token":a["token"]})
        )
        .await
        .get("error")
        .is_some()
    );
    let b_health = call(&mut b_client, 5, "runtime.health", json!({})).await;
    assert_eq!(b_health["result"]["status"], "ok");
    let listed = call(&mut admin_client, 6, "apps.list", json!({})).await;
    assert_eq!(
        listed["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|app| app["app"] == "app-a")
            .count(),
        0
    );
    let _ = child.kill();
    let _ = child.wait();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_independent_broker_survives_consumer_disconnects_and_converges() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().to_path_buf();
    let address = endpoint();
    let token_file = directory.path().join("broker-token.json");
    let admin_token_file = directory.path().join("broker-admin-token.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "serve",
            "--ipc",
            &address,
            "--independent",
            "--token-file",
            token_file.to_str().unwrap(),
            "--admin-token-file",
            admin_token_file.to_str().unwrap(),
            "--allow-passive-discovery",
            "--idle-timeout",
            "5",
            "--home",
            home.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let descriptor: Value = loop {
        if let Ok(bytes) = std::fs::read(&token_file)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            break value;
        }
        assert!(
            Instant::now() < deadline,
            "broker did not publish a token file"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let token = descriptor["token"].as_str().unwrap().to_owned();
    assert_eq!(descriptor["address"], address);
    let mut first = authenticated(&address, &token).await;
    assert!(
        call(&mut first, 3, "snapshot.get", json!({}))
            .await
            .get("result")
            .is_some()
    );
    drop(first);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        child.try_wait().unwrap().is_none(),
        "closing one consumer must not stop the independent broker"
    );
    let mut second = authenticated(&address, &token).await;
    assert!(
        call(&mut second, 3, "runtime.health", json!({}))
            .await
            .get("result")
            .is_some()
    );
    drop(second);

    let admin_deadline = Instant::now() + Duration::from_secs(10);
    while !admin_token_file.exists() {
        assert!(
            Instant::now() < admin_deadline,
            "admin token was not published"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let admin: Value = serde_json::from_slice(&std::fs::read(&admin_token_file).unwrap()).unwrap();
    let admin_token = admin["token"].as_str().unwrap();
    let mut admin_client = authenticated(&address, admin_token).await;
    let apps = call(&mut admin_client, 3, "apps.list", json!({})).await;
    assert!(
        apps["result"].as_array().is_some(),
        "admin can list applications"
    );
    let mut victim = authenticated(&address, &token).await;
    let victims: Vec<String> = {
        let listed = call(&mut admin_client, 4, "apps.list", json!({})).await;
        listed["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|app| app["admin"] == false)
            .filter_map(|app| app["id"].as_str().map(str::to_owned))
            .collect()
    };
    assert!(!victims.is_empty());
    for (index, victim_id) in victims.iter().enumerate() {
        let result = call(
            &mut admin_client,
            (10 + index) as u64,
            "apps.revoke",
            json!({"targetClientId": victim_id}),
        )
        .await;
        assert_eq!(
            result["result"]["revoked"], true,
            "revocation failed for {victim_id}: {result}"
        );
    }
    let denied = call(&mut victim, 5, "runtime.health", json!({})).await;
    assert_eq!(denied["error"]["data"]["code"], "LOCAL_ACCESS_DENIED");
    drop(victim);
    drop(admin_client);

    // A concurrent launch must fail to take over the live endpoint.
    let mut duplicate = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "serve",
            "--ipc",
            &address,
            "--independent",
            "--token-file",
            token_file.to_str().unwrap(),
            "--allow-passive-discovery",
            "--home",
            home.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(status) = duplicate.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("duplicate broker must exit");
    assert!(!status.success());

    // After the idle timeout the broker releases the endpoint and token file.
    let status = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("idle broker must stop");
    assert!(status.success());
    assert!(
        !token_file.exists(),
        "a stopped broker removes its owner token"
    );
}
