use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::NativeService;
use serde_json::json;
use std::{sync::Arc, time::Duration};
fn service() -> NativeService {
    NativeService::new(
        Arc::new(|_| {
            Ok(ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: Default::default(),
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(60),
    )
}
#[tokio::test]
async fn closed_clients_do_not_exhaust_the_long_lived_broker() {
    let s = service();
    for n in 0..150 {
        let id = format!("app-{n}");
        s.register_client(&id, ClientPolicy::passive()).unwrap();
        assert_eq!(
            s.handle(&id, "runtime.closeClient", json!({}))
                .await
                .unwrap()["closed"],
            true
        );
        assert!(s.handle(&id, "snapshot.get", json!({})).await.is_err());
    }
}
#[tokio::test]
async fn closing_one_app_preserves_the_other_apps_permissions() {
    let s = service();
    s.register_client("a", ClientPolicy::passive()).unwrap();
    s.register_client("b", ClientPolicy::passive()).unwrap();
    s.handle("a", "runtime.closeClient", json!({}))
        .await
        .unwrap();
    assert!(s.handle("b", "discovery.refresh", json!({})).await.is_ok());
}
#[tokio::test]
async fn rpc_close_cannot_choose_a_different_owner() {
    let s = service();
    s.register_client("a", ClientPolicy::passive()).unwrap();
    s.register_client("b", ClientPolicy::passive()).unwrap();
    assert!(
        s.handle("a", "runtime.closeClient", json!({"clientId":"b"}))
            .await
            .is_err()
    );
    assert!(s.handle("b", "snapshot.get", json!({})).await.is_ok());
}
