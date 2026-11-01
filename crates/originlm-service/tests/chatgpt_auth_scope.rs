use originlm_chatgpt_auth::AuthManager;
use originlm_contracts::SessionState;
use originlm_contracts::{DataMode, ErrorCode, Snapshot};
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::NativeService;
use serde_json::json;
use std::collections::BTreeSet;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;

struct IdleAgent;
impl Agent for IdleAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, serde_json::Value>,
        _: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn chatgpt_auth_methods_require_managed_metadata_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let auth = Arc::new(AuthManager::new(tmp.path().join("auth"), "example-app".into()).unwrap());
    let source = Arc::new(|_: &std::sync::atomic::AtomicBool| {
        Ok(ScanResult {
            snapshot: Snapshot::empty(DataMode::Live),
            locations: BTreeMap::new(),
            metadata_probes: 0,
        })
    });
    let service =
        NativeService::new(source.clone(), Duration::from_secs(1)).with_chatgpt_auth(auth);
    service
        .register_client("denied", ClientPolicy::passive())
        .unwrap();
    let mut allowed = ClientPolicy::passive();
    allowed.probe_provider_ids.insert("codex_managed".into());
    service.register_client("allowed", allowed).unwrap();
    assert_eq!(
        service
            .handle("denied", "auth.chatgpt.status", json!({}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::LocalAccessDenied,
    );
    let status = service
        .handle("allowed", "auth.chatgpt.status", json!({}))
        .await
        .unwrap();
    assert_eq!(status["state"], "signed_out");
    assert_eq!(status["loginId"], serde_json::Value::Null);
    let methods = service
        .handle("allowed", "runtime.handshake", json!({}))
        .await
        .unwrap();
    assert!(
        methods["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "auth.chatgpt.status")
    );
    assert!(
        methods["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capability| capability == "auth.chatgpt")
    );
    let unopted = NativeService::new(source, Duration::from_secs(1));
    unopted
        .register_client("allowed", ClientPolicy::passive())
        .unwrap();
    let unopted_handshake = unopted
        .handle("allowed", "runtime.handshake", json!({}))
        .await
        .unwrap();
    assert!(
        !unopted_handshake["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| method
                .as_str()
                .is_some_and(|method| method.starts_with("auth.chatgpt.")))
    );
    assert!(
        !unopted_handshake["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capability| capability == "auth.chatgpt")
    );
}

#[tokio::test]
async fn disconnect_waits_for_inflight_managed_create_then_closes_its_session() {
    let tmp = tempfile::tempdir().unwrap();
    let auth = Arc::new(AuthManager::new(tmp.path().join("auth"), "example-app".into()).unwrap());
    auth.status().await.unwrap();
    let mut snapshot = common::fixture();
    for provider in &mut snapshot.providers {
        provider.id = "codex_managed".into();
    }
    for installation in &mut snapshot.installations {
        installation.provider_id = "codex_managed".into();
    }
    for account in &mut snapshot.accounts {
        account.provider_id = "codex_managed".into();
    }
    for connection in &mut snapshot.connections {
        connection.provider_id = "codex_managed".into();
    }
    for route in &mut snapshot.routes {
        route.provider_id = "codex_managed".into();
    }
    let inventory = Arc::new(move |_: &std::sync::atomic::AtomicBool| {
        Ok(ScanResult {
            snapshot: snapshot.clone(),
            locations: BTreeMap::from([(
                "personal".into(),
                originlm_discovery::LocatedClient {
                    executable_identity: None,
                    provider_id: "codex_managed".into(),
                    profile_home: std::env::temp_dir(),
                    executable: None,
                    scope: None,
                },
            )]),
            metadata_probes: 0,
        })
    });
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (started, proceed) = (entered.clone(), release.clone());
    let service = Arc::new(
        NativeService::new(inventory, Duration::ZERO)
            .with_chatgpt_auth(auth)
            .with_execution(
                BTreeSet::from(["codex_managed".into()]),
                Arc::new(move |_| {
                    let (started, proceed) = (started.clone(), proceed.clone());
                    Box::pin(async move {
                        started.notify_one();
                        proceed.notified().await;
                        Ok(Box::new(IdleAgent) as Box<dyn Agent>)
                    })
                }),
            ),
    );
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids.insert("codex_managed".into());
    service.register_client("a", policy).unwrap();
    service
        .register_execution(
            "a",
            originlm_service::ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
                ..Default::default()
            },
        )
        .unwrap();
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let create_service = service.clone();
    let create = tokio::spawn(async move {
        create_service
            .handle(
                "a",
                "sessions.create",
                json!({
                    "selection": common::selection(), "workspaceId": "scratch",
                }),
            )
            .await
    });
    entered.notified().await;
    let disconnect_service = service.clone();
    let mut disconnect = tokio::spawn(async move {
        disconnect_service
            .handle("a", "auth.chatgpt.disconnect", json!({}))
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(40), &mut disconnect)
            .await
            .is_err()
    );
    release.notify_one();
    let created = create.await.unwrap().unwrap();
    let disconnected = disconnect.await.unwrap();
    assert!(disconnected.is_ok(), "disconnect failed: {disconnected:?}");
    let closed = service
        .handle("a", "sessions.get", json!({"sessionId": created["id"]}))
        .await;
    assert!(
        closed.is_err()
            || closed.unwrap()["state"] == serde_json::to_value(SessionState::Closed).unwrap()
    );
}

#[tokio::test]
async fn unrelated_provider_startup_does_not_delay_managed_disconnect() {
    let tmp = tempfile::tempdir().unwrap();
    let auth = Arc::new(AuthManager::new(tmp.path().join("auth"), "example-app".into()).unwrap());
    auth.status().await.unwrap();
    let snapshot = common::fixture();
    let inventory = Arc::new(move |_: &std::sync::atomic::AtomicBool| {
        Ok(ScanResult {
            snapshot: snapshot.clone(),
            locations: BTreeMap::from([(
                "personal".into(),
                originlm_discovery::LocatedClient {
                    executable_identity: None,
                    provider_id: "test".into(),
                    profile_home: std::env::temp_dir(),
                    executable: None,
                    scope: None,
                },
            )]),
            metadata_probes: 0,
        })
    });
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (started, proceed) = (entered.clone(), release.clone());
    let service = Arc::new(
        NativeService::new(inventory, Duration::ZERO)
            .with_chatgpt_auth(auth)
            .with_execution(
                BTreeSet::from(["test".into()]),
                Arc::new(move |_| {
                    let (started, proceed) = (started.clone(), proceed.clone());
                    Box::pin(async move {
                        started.notify_one();
                        proceed.notified().await;
                        Ok(Box::new(IdleAgent) as Box<dyn Agent>)
                    })
                }),
            ),
    );
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids.insert("codex_managed".into());
    service.register_client("a", policy).unwrap();
    service
        .register_execution(
            "a",
            originlm_service::ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
                ..Default::default()
            },
        )
        .unwrap();
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let create_service = service.clone();
    let create = tokio::spawn(async move {
        create_service
            .handle(
                "a",
                "sessions.create",
                json!({
                    "selection": common::selection(), "workspaceId": "scratch",
                }),
            )
            .await
    });
    entered.notified().await;
    let disconnected = tokio::time::timeout(
        Duration::from_secs(2),
        service.handle("a", "auth.chatgpt.disconnect", json!({})),
    )
    .await;
    assert!(
        disconnected.is_ok(),
        "unrelated provider startup blocked managed disconnect"
    );
    assert!(disconnected.unwrap().is_ok());
    release.notify_one();
    assert!(create.await.unwrap().is_ok());
}
