use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::NativeService;
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn empty() -> ScanResult {
    ScanResult {
        snapshot: Snapshot::empty(DataMode::Live),
        locations: BTreeMap::new(),
        metadata_probes: 0,
    }
}
#[tokio::test]
async fn protocol_mismatch_and_parameter_smuggling_are_rejected() {
    let service = NativeService::new(Arc::new(|_| Ok(empty())), Duration::from_secs(60));
    service
        .register_client("app", ClientPolicy::passive())
        .unwrap();
    assert_eq!(
        service
            .handle("app", "runtime.handshake", json!({"protocolVersion":"2.0"}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::ProtocolIncompatible
    );
    for params in [
        json!({"force":true}),
        json!({"level":"passive","home":"private"}),
    ] {
        assert_eq!(
            service
                .handle("app", "discovery.refresh", params)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }
    assert!(
        service
            .handle("app", "snapshot.get", json!({"fields":["connections",2]}))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn cached_reads_remain_responsive_while_scan_runs() {
    let started = Arc::new(AtomicBool::new(false));
    let ready = started.clone();
    let service = Arc::new(NativeService::new(
        Arc::new(move |_| {
            ready.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(100));
            Ok(empty())
        }),
        Duration::from_secs(60),
    ));
    service
        .register_client("app", ClientPolicy::passive())
        .unwrap();
    let worker = service.clone();
    let task =
        tokio::spawn(async move { worker.handle("app", "discovery.refresh", json!({})).await });
    while !started.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    tokio::time::timeout(
        Duration::from_millis(30),
        service.handle("app", "snapshot.get", json!({})),
    )
    .await
    .unwrap()
    .unwrap();
    task.await.unwrap().unwrap();
}
#[tokio::test]
async fn revocation_during_scan_does_not_restore_authority() {
    let started = Arc::new(AtomicBool::new(false));
    let ready = started.clone();
    let service = Arc::new(NativeService::new(
        Arc::new(move |_| {
            ready.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(70));
            Ok(empty())
        }),
        Duration::from_secs(60),
    ));
    service
        .register_client("app", ClientPolicy::passive())
        .unwrap();
    let worker = service.clone();
    let task =
        tokio::spawn(async move { worker.handle("app", "discovery.refresh", json!({})).await });
    while !started.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    service
        .handle("app", "grants.revoke", json!({}))
        .await
        .unwrap();
    assert_eq!(
        task.await.unwrap().unwrap_err().code,
        ErrorCode::LocalAccessDenied
    );
}
#[tokio::test]
async fn source_failure_does_not_serialize_private_error_text() {
    let service = NativeService::new(
        Arc::new(|_| {
            Err(DomainError::new(
                ErrorCode::LocalAccessDenied,
                "secret-token and private-path",
            ))
        }),
        Duration::from_secs(60),
    );
    service
        .register_client("app", ClientPolicy::passive())
        .unwrap();
    let error = service
        .handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap_err();
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("secret-token")
    );
}
