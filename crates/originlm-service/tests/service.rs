#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::*;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
fn service(counter: Arc<AtomicUsize>) -> Arc<NativeService> {
    Arc::new(NativeService::new(
        Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
            Ok(ScanResult {
                snapshot: common::fixture(),
                locations: BTreeMap::new(),
                metadata_probes: 2,
            })
        }),
        Duration::from_secs(60),
    ))
}
#[tokio::test]
async fn construction_handshake_and_snapshot_never_scan() {
    let count = Arc::new(AtomicUsize::new(0));
    let service = service(count.clone());
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    let h = service
        .handle(
            "owner",
            "runtime.handshake",
            json!({"protocolVersion":"1.0"}),
        )
        .await
        .unwrap();
    assert_eq!(h["protocolVersion"], "1.0");
    let snapshot = service
        .handle("owner", "snapshot.get", json!({}))
        .await
        .unwrap();
    assert!(snapshot["connections"].as_array().unwrap().is_empty());
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn unknown_client_cannot_read_or_scan() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    assert_eq!(
        service
            .handle("intruder", "snapshot.get", json!({}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::LocalAccessDenied
    );
}
#[tokio::test]
async fn ungranted_client_cannot_trigger_discovery() {
    let count = Arc::new(AtomicUsize::new(0));
    let service = service(count.clone());
    service
        .register_client("denied", ClientPolicy::denied())
        .unwrap();
    assert_eq!(
        service
            .handle("denied", "discovery.refresh", json!({"level":"passive"}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::LocalAccessDenied
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn simultaneous_refreshes_are_singleflight() {
    let count = Arc::new(AtomicUsize::new(0));
    let service = service(count.clone());
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    let mut tasks = Vec::new();
    for _ in 0..30 {
        let s = service.clone();
        tasks.push(tokio::spawn(async move {
            s.handle("owner", "discovery.refresh", json!({"level":"passive"}))
                .await
                .unwrap()
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn alternate_data_methods_respect_the_same_projection() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service
        .register_client(
            "limited",
            ClientPolicy {
                connection_ids: Some(BTreeSet::from(["personal".into()])),
                ..ClientPolicy::denied()
            },
        )
        .unwrap();
    service
        .handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    for method in [
        "snapshot.get",
        "connections.list",
        "models.list",
        "usage.read",
        "diagnostics.report",
    ] {
        let data = service.handle("limited", method, json!({})).await.unwrap();
        assert!(!data.to_string().contains("work"), "{method}");
    }
    assert_eq!(
        service
            .handle(
                "limited",
                "capabilities.read",
                json!({"modelId":"model-work"})
            )
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}
#[tokio::test]
async fn browser_request_cannot_grant_itself_more_permissions() {
    let count = Arc::new(AtomicUsize::new(0));
    let service = service(count.clone());
    service
        .register_client("app", ClientPolicy::denied())
        .unwrap();
    let response = service
        .handle(
            "app",
            "grants.request",
            json!({"scopes":["discovery"],"connectionIds":[]}),
        )
        .await
        .unwrap();
    assert_eq!(response["status"], "requires_host_approval");
    assert!(
        service
            .handle("app", "discovery.refresh", json!({}))
            .await
            .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn revocation_immediately_hides_cached_data() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service
        .handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    service
        .handle("owner", "grants.revoke", json!({}))
        .await
        .unwrap();
    let s = service
        .handle("owner", "snapshot.get", json!({}))
        .await
        .unwrap();
    assert!(s["connections"].as_array().unwrap().is_empty());
    assert!(
        service
            .handle("owner", "discovery.refresh", json!({}))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn request_payload_does_not_choose_caller_identity() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service
        .register_client("limited", ClientPolicy::denied())
        .unwrap();
    service
        .handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert!(
        service
            .handle("limited", "snapshot.get", json!({"clientId":"owner"}))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn native_selection_validation_uses_authorized_catalog() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service
        .register_client("limited", ClientPolicy::denied())
        .unwrap();
    service
        .handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let selected = common::selection();
    assert_eq!(
        service
            .handle("owner", "selection.validate", json!({"selection":selected}))
            .await
            .unwrap()["valid"],
        true
    );
    assert!(
        service
            .handle(
                "limited",
                "selection.validate",
                json!({"selection":common::selection()})
            )
            .await
            .is_err()
    );
}
#[tokio::test]
async fn unsupported_methods_and_nonpassive_scans_fail_closed() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    assert_eq!(
        service
            .handle("owner", "sessions.create", json!({}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        service
            .handle("owner", "discovery.refresh", json!({"level":"secret"}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[tokio::test]
async fn duplicate_host_registration_does_not_replace_policy() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("app", ClientPolicy::denied())
        .unwrap();
    assert_eq!(
        service
            .register_client("app", ClientPolicy::passive())
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert!(
        service
            .handle("app", "discovery.refresh", json!({}))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn requests_are_bounded_and_projection_fields_are_allowlisted() {
    let service = service(Arc::new(AtomicUsize::new(0)));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    assert!(
        service
            .handle("owner", "snapshot.get", json!({"fields":["locations"]}))
            .await
            .is_err()
    );
    assert!(
        service
            .handle("owner", "snapshot.get", json!([]))
            .await
            .is_err()
    );
}
