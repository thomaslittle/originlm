use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::*;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
fn service() -> NativeService {
    NativeService::new(
        Arc::new(|_| {
            Ok(ScanResult {
                snapshot: Snapshot::empty(DataMode::Synthetic),
                locations: BTreeMap::new(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(60),
    )
    .with_execution(
        BTreeSet::from(["codex".into()]),
        Arc::new(|_| {
            Box::pin(async { Err(DomainError::new(ErrorCode::CapabilityUnsupported, "test")) })
        }),
    )
}
#[tokio::test]
async fn health_describes_configured_runtime_not_a_stale_constant() {
    let s = service();
    s.register_client("a", ClientPolicy::passive()).unwrap();
    let h = s.handle("a", "runtime.health", json!({})).await.unwrap();
    assert_eq!(h["executionAvailable"], true);
}
#[tokio::test]
async fn diagnostics_and_grants_separate_available_execution_from_permission() {
    let s = service();
    s.register_client("a", ClientPolicy::passive()).unwrap();
    let h = s
        .handle("a", "diagnostics.report", json!({}))
        .await
        .unwrap();
    assert_eq!(h["executionAvailable"], true);
    let g = s.handle("a", "grants.list", json!({})).await.unwrap();
    assert_eq!(g["execution"], false);
    let tmp = std::env::temp_dir();
    s.register_execution(
        "a",
        ExecutionGrant {
            provider_ids: BTreeSet::from(["codex".into()]),
            workspaces: BTreeMap::from([("default".into(), tmp.clone())]),
            ..Default::default()
        },
    )
    .unwrap();
    let g = s.handle("a", "grants.list", json!({})).await.unwrap();
    assert_eq!(g["execution"], true);
    assert!(!g.to_string().contains(tmp.to_str().unwrap()));
}
#[tokio::test]
async fn adapter_listing_is_a_collection_not_fake_single_provider_status() {
    let s = service();
    s.register_client("a", ClientPolicy::passive()).unwrap();
    assert!(
        s.handle("a", "adapters.list", json!({}))
            .await
            .unwrap()
            .is_array()
    );
}
