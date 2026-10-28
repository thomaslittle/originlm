use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced,
    tokio::{Stream, prelude::*},
};
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_service::NativeService;
use originlm_transports::local_ipc::{Broker, BrokerOptions};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
static ENDPOINT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
fn endpoint() -> String {
    // SystemTime may return the same tick to concurrently starting Windows tests.
    let sequence = ENDPOINT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    if cfg!(windows) {
        format!(
            "originlm-test-{}-{}-{sequence}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    } else {
        std::env::temp_dir()
            .join(format!(
                "originlm-{}-{}-{sequence}.sock",
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
fn service() -> (Arc<NativeService>, Arc<AtomicUsize>) {
    let n = Arc::new(AtomicUsize::new(0));
    let c = n.clone();
    (
        Arc::new(NativeService::new(
            Arc::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(originlm_discovery::ScanResult {
                    snapshot: Snapshot::empty(DataMode::Live),
                    locations: Default::default(),
                    metadata_probes: 0,
                })
            }),
            Duration::from_secs(60),
        )),
        n,
    )
}

/// Two synthetic accounts in one provider, with a pool that spans both. The
/// broker scopes below must never expose one account's connection or route to
/// the other application.
fn scoped_service() -> Arc<NativeService> {
    let mut snapshot = Snapshot::empty(DataMode::Synthetic);
    snapshot.providers.push(Provider {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind: "synthetic".into(),
    });
    let evidence = EvidenceSource::new(EvidenceKind::Synthetic, "fixture", "broker scope test");
    for label in ["personal", "work"] {
        snapshot.installations.push(Installation {
            id: format!("install-{label}"),
            provider_id: "fixture".into(),
            environment_id: "synthetic".into(),
            location_hint: "synthetic".into(),
            version: Observation::unknown("NOT_PROBED"),
        });
        snapshot.profiles.push(Profile {
            id: format!("profile-{label}"),
            installation_id: format!("install-{label}"),
            label: label.into(),
        });
        snapshot.accounts.push(Account {
            id: format!("account-{label}"),
            provider_id: "fixture".into(),
            display_label: Observation::reported(format!("{label}@example.test"), evidence.clone()),
            identity_source: Observation::reported(format!("private-{label}"), evidence.clone()),
            scopes: vec![],
        });
        snapshot.connections.push(Connection {
            id: label.into(),
            provider_id: "fixture".into(),
            installation_id: format!("install-{label}"),
            profile_id: format!("profile-{label}"),
            display_name: label.into(),
            account_id: Some(format!("account-{label}")),
            auth: Observation::reported(AuthState::Authenticated, evidence.clone()),
            plan: Observation::unknown("NOT_REPORTED"),
            status: "ok".into(),
            diagnostics: vec![],
        });
        snapshot.routes.push(AccessRoute {
            id: format!("route-{label}"),
            connection_id: label.into(),
            provider_id: "fixture".into(),
            display_name: label.into(),
            billing: Observation::reported(BillingMode::Subscription, evidence.clone()),
            capabilities: vec![],
            catalog_revision: "1".into(),
        });
        snapshot.models.push(Model {
            id: format!("model-{label}"),
            route_id: format!("route-{label}"),
            native_id: "same-native-model".into(),
            name: "Same native model".into(),
            revision: "1".into(),
            is_default: true,
            capabilities: vec![],
            options: vec![],
            context_window: Observation::unknown("NOT_REPORTED"),
        });
    }
    snapshot.quota_pools.push(QuotaPool {
        id: "shared".into(),
        account_id: None,
        connection_ids: vec!["personal".into(), "work".into()],
        route_ids: vec!["route-personal".into(), "route-work".into()],
        sharing: "verified".into(),
        windows: vec![],
    });
    Arc::new(NativeService::new(
        Arc::new(move |_| {
            Ok(originlm_discovery::ScanResult {
                snapshot: snapshot.clone(),
                locations: Default::default(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(60),
    ))
}

fn scoped_policy(connection: &str) -> ClientPolicy {
    ClientPolicy {
        allow_passive_discovery: true,
        connection_ids: Some(BTreeSet::from([connection.into()])),
        ..ClientPolicy::default()
    }
}

fn admin_policy() -> ClientPolicy {
    ClientPolicy {
        allow_passive_discovery: true,
        connection_ids: None,
        admin: true,
        ..ClientPolicy::default()
    }
}

async fn connect(name: &str) -> BufReader<Stream> {
    let name = if cfg!(windows) {
        name.to_ns_name::<GenericNamespaced>().unwrap()
    } else {
        name.to_fs_name::<GenericFilePath>().unwrap()
    };
    BufReader::new(Stream::connect(name).await.unwrap())
}
async fn call(c: &mut BufReader<Stream>, m: &str, p: Value) -> Value {
    c.get_mut()
        .write_all(
            format!(
                "{}\n",
                json!({"jsonrpc":"2.0","id":1,"method":m,"params":p})
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(2), c.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    serde_json::from_str(&line).unwrap()
}
async fn auth(name: &str, key: String) -> BufReader<Stream> {
    let mut c = connect(name).await;
    assert!(
        call(&mut c, "broker.authenticate", json!({"token":key}))
            .await
            .get("result")
            .is_some()
    );
    assert_eq!(
        call(&mut c, "runtime.handshake", json!({})).await["result"]["protocolVersion"],
        "1.0"
    );
    c
}
#[tokio::test]
async fn two_clients_share_inventory_and_closing_one_does_not_stop_broker() {
    let (s, n) = service();
    let name = endpoint();
    let b = Broker::start(s, &name).await.unwrap();
    let mut a = auth(&name, b.authorize(ClientPolicy::passive(), None).unwrap()).await;
    let mut c = auth(&name, b.authorize(ClientPolicy::passive(), None).unwrap()).await;
    call(&mut a, "discovery.refresh", json!({})).await;
    call(&mut c, "discovery.refresh", json!({})).await;
    assert_eq!(n.load(Ordering::SeqCst), 1);
    drop(a);
    assert_eq!(
        call(&mut c, "runtime.health", json!({})).await["result"]["status"],
        "ok"
    );
    b.close().await;
}
#[tokio::test]
async fn unauthorized_connections_cannot_query_and_keys_are_single_use() {
    let (s, n) = service();
    let name = endpoint();
    let b = Broker::start(s, &name).await.unwrap();
    let mut bad = connect(&name).await;
    let r = call(&mut bad, "snapshot.get", json!({})).await;
    assert_eq!(r["error"]["data"]["code"], "LOCAL_ACCESS_DENIED");
    let key = b.authorize(ClientPolicy::passive(), None).unwrap();
    let _a = auth(&name, key.clone()).await;
    let mut reuse = connect(&name).await;
    assert!(
        call(&mut reuse, "broker.authenticate", json!({"token":key}))
            .await
            .get("error")
            .is_some()
    );
    assert_eq!(n.load(Ordering::SeqCst), 0);
    b.close().await;
}
#[tokio::test]
async fn another_broker_cannot_take_over_a_live_endpoint() {
    let (s, _) = service();
    let name = endpoint();
    let b = Broker::start(s.clone(), &name).await.unwrap();
    assert!(Broker::start(s, &name).await.is_err());
    let mut a = auth(&name, b.authorize(ClientPolicy::passive(), None).unwrap()).await;
    assert!(
        call(&mut a, "snapshot.get", json!({}))
            .await
            .get("result")
            .is_some()
    );
    b.close().await;
}
#[tokio::test]
async fn permissions_are_per_connection_and_revocation_is_local() {
    let (s, n) = service();
    let name = endpoint();
    let b = Broker::start(s, &name).await.unwrap();
    let mut a = auth(&name, b.authorize(ClientPolicy::denied(), None).unwrap()).await;
    let mut c = auth(&name, b.authorize(ClientPolicy::passive(), None).unwrap()).await;
    assert_eq!(
        call(&mut a, "discovery.refresh", json!({})).await["error"]["data"]["code"],
        "LOCAL_ACCESS_DENIED"
    );
    call(&mut c, "grants.revoke", json!({})).await;
    assert!(
        call(&mut c, "discovery.refresh", json!({}))
            .await
            .get("error")
            .is_some()
    );
    assert_eq!(n.load(Ordering::SeqCst), 0);
    b.close().await;
}
#[tokio::test]
async fn endpoint_rejects_network_names_and_ambiguous_addresses() {
    let (s, _) = service();
    for bad in [
        "",
        "relative/path",
        "\\\\remote\\pipe\\steal",
        "http://127.0.0.1:1000",
        "a\nb",
    ] {
        assert!(Broker::start(s.clone(), bad).await.is_err());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn endpoint_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let (s, _) = service();
    let name = endpoint();
    let b = Broker::start(s, &name).await.unwrap();
    let mode = std::fs::metadata(&name).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "broker endpoint must be owner-only");
    b.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_stale_unix_socket_is_reclaimed_but_a_live_one_is_not() {
    let name = endpoint();
    {
        let listener = std::os::unix::net::UnixListener::bind(&name).unwrap();
        drop(listener);
    }
    assert!(std::path::Path::new(&name).exists());
    let (s, _) = service();
    let broker = Broker::start(s, &name)
        .await
        .expect("stale socket is reclaimable");
    broker.close().await;
}

#[tokio::test]
async fn scoped_app_grants_isolate_accounts_and_revocation_removes_the_grant() {
    let service = scoped_service();
    let name = endpoint();
    let broker = Broker::start(service, &name).await.unwrap();
    let admin = broker
        .authorize_shared_for("kit-admin", admin_policy(), None)
        .unwrap();
    let app_a = broker
        .authorize_shared_for("app-a", scoped_policy("personal"), None)
        .unwrap();
    let app_b = broker
        .authorize_shared_for("app-b", scoped_policy("work"), None)
        .unwrap();
    assert_eq!(app_a.len(), 64);
    assert_ne!(app_a, app_b);
    assert!(
        broker
            .authorize_shared_for("ipc-host", ClientPolicy::passive(), None)
            .is_err(),
        "the legacy owner identity cannot be used as an application identity"
    );

    let mut admin_client = auth(&name, admin).await;
    let mut a = auth(&name, app_a.clone()).await;
    let mut b = auth(&name, app_b.clone()).await;
    let refresh = call(
        &mut admin_client,
        "discovery.refresh",
        json!({"level":"passive"}),
    )
    .await;
    assert!(refresh.get("result").is_some(), "refresh failed: {refresh}");

    let snapshot = call(&mut a, "snapshot.get", json!({})).await;
    let snapshot = &snapshot["result"];
    assert_eq!(snapshot["connections"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["connections"][0]["id"], "personal");
    assert_eq!(snapshot["routes"][0]["id"], "route-personal");
    assert_eq!(snapshot["models"][0]["id"], "model-personal");
    assert_eq!(snapshot["accounts"][0]["id"], "account-personal");
    assert_eq!(
        snapshot["accounts"][0]["displayLabel"]["state"], "unknown",
        "identity is not revealed without a host grant"
    );
    assert_eq!(
        snapshot["quotaPools"][0]["connectionIds"],
        json!(["personal"])
    );
    assert_eq!(
        snapshot["quotaPools"][0]["routeIds"],
        json!(["route-personal"])
    );

    let b_snapshot = call(&mut b, "snapshot.get", json!({})).await;
    let b_snapshot = &b_snapshot["result"];
    assert_eq!(b_snapshot["connections"][0]["id"], "work");
    assert_eq!(b_snapshot["routes"][0]["id"], "route-work");
    assert_eq!(b_snapshot["models"][0]["id"], "model-work");
    assert_eq!(b_snapshot["accounts"][0]["id"], "account-work");
    assert_eq!(
        b_snapshot["quotaPools"][0]["connectionIds"],
        json!(["work"])
    );

    let cross = call(&mut a, "models.list", json!({"routeId":"route-work"})).await;
    assert_eq!(cross["result"], json!([]), "cross-account route leaked");
    let denied = call(
        &mut a,
        "selection.validate",
        json!({"routeId":"route-work","modelId":"model-work","revision":"1"}),
    )
    .await;
    assert!(denied.get("error").is_some());

    let apps = call(&mut admin_client, "apps.list", json!({})).await;
    let apps = apps["result"].as_array().unwrap();
    let identity = |app: &str| {
        apps.iter()
            .find(|entry| entry["app"] == app)
            .unwrap_or_else(|| panic!("app {app} is missing from {apps:?}"))
    };
    let a_id = identity("app-a")["id"].as_str().unwrap().to_owned();
    let b_id = identity("app-b")["id"].as_str().unwrap().to_owned();
    assert_ne!(a_id, b_id);
    assert_eq!(identity("app-a")["connectionIds"], json!(["personal"]));
    assert_eq!(identity("app-b")["connectionIds"], json!(["work"]));

    let revoked = call(
        &mut admin_client,
        "apps.revoke",
        json!({"targetClientId":a_id}),
    )
    .await;
    assert_eq!(revoked["result"]["revoked"], true);
    assert_eq!(revoked["result"]["grantsRevoked"], true);
    assert_eq!(revoked["result"]["app"], "app-a");

    let denied = call(&mut a, "runtime.health", json!({})).await;
    assert_eq!(denied["error"]["data"]["code"], "LOCAL_ACCESS_DENIED");
    let mut reconnect = connect(&name).await;
    let reused = call(
        &mut reconnect,
        "broker.authenticate",
        json!({"token":app_a}),
    )
    .await;
    assert!(
        reused.get("error").is_some(),
        "a revoked grant must not reconnect: {reused}"
    );
    drop(reconnect);

    // The other application's credential and live connection survive.
    assert_eq!(
        call(&mut b, "runtime.health", json!({})).await["result"]["status"],
        "ok"
    );
    let _b_again = auth(&name, app_b).await;
    let apps = call(&mut admin_client, "apps.list", json!({})).await;
    assert!(
        apps["result"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["app"] != "app-a"),
        "revoked app is still registered"
    );
    broker.close().await;
}

#[tokio::test]
async fn an_independent_owner_credential_can_open_more_than_one_connection() {
    let (s, _) = service();
    let name = endpoint();
    let b = Broker::start(s, &name).await.unwrap();
    let token = b.authorize_shared(ClientPolicy::passive(), None).unwrap();
    let _first = auth(&name, token.clone()).await;
    let _second = auth(&name, token).await;
    assert_eq!(b.active_connections(), 2);
    b.close().await;
}

#[tokio::test]
async fn an_idle_independent_broker_releases_its_endpoint() {
    let (s, _) = service();
    let name = endpoint();
    let b = Broker::start_with(
        s.clone(),
        &name,
        BrokerOptions {
            idle_timeout: Some(Duration::from_millis(300)),
        },
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert_eq!(b.active_connections(), 0);
    b.close().await;
    let second = Broker::start(s, &name).await;
    assert!(second.is_ok());
    second.unwrap().close().await;
}
