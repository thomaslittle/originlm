#[path = "../../originlm-core/tests/common/mod.rs"]
#[allow(dead_code)]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::NativeService;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};
async fn setup(count: usize) -> (NativeService, Arc<Mutex<Snapshot>>) {
    let mut snapshot = common::fixture();
    let templates = snapshot.models.clone();
    snapshot.models = (0..count)
        .map(|i| {
            let mut m = templates[i % 2].clone();
            m.id = format!("model-{i}");
            m.native_id = format!("native-{i}");
            m
        })
        .collect();
    let state = Arc::new(Mutex::new(snapshot));
    let source = state.clone();
    let service = NativeService::new(
        Arc::new(move |_| {
            Ok(ScanResult {
                snapshot: source.lock().unwrap().clone(),
                locations: BTreeMap::new(),
                metadata_probes: 0,
            })
        }),
        Duration::ZERO,
    );
    for id in ["a", "b"] {
        service
            .register_client(id, ClientPolicy::passive())
            .unwrap();
    }
    service
        .register_client(
            "restricted",
            ClientPolicy {
                connection_ids: Some(BTreeSet::from(["personal".into()])),
                ..ClientPolicy::passive()
            },
        )
        .unwrap();
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    (service, state)
}
#[tokio::test]
async fn paged_catalog_preserves_order_and_every_route_identity() {
    let (s, _) = setup(501).await;
    let mut ids = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let mut p = json!({"limit":37});
        if !cursor.is_null() {
            p["cursor"] = cursor;
        }
        let page = s.handle("a", "models.page", p).await.unwrap();
        assert!(page["models"].as_array().unwrap().len() <= 37);
        ids.extend(
            page["models"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["id"].as_str().unwrap().to_owned()),
        );
        cursor = page["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(
        ids,
        (0..501).map(|i| format!("model-{i}")).collect::<Vec<_>>()
    );
}
#[tokio::test]
async fn page_cursors_cannot_move_between_clients_or_route_filters() {
    let (s, _) = setup(10).await;
    let page = s
        .handle("a", "models.page", json!({"limit":2}))
        .await
        .unwrap();
    for (owner, p) in [
        ("b", json!({"cursor":page["nextCursor"],"limit":2})),
        (
            "a",
            json!({"cursor":page["nextCursor"],"routeId":"route-personal","limit":2}),
        ),
    ] {
        assert!(s.handle(owner, "models.page", p).await.is_err());
    }
}
#[tokio::test]
async fn page_rechecks_scope_and_rejects_changed_catalogs() {
    let (s, state) = setup(10).await;
    let page = s
        .handle("a", "models.page", json!({"limit":2}))
        .await
        .unwrap();
    state.lock().unwrap().models[0].revision = "changed".into();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    assert_eq!(
        s.handle("a", "models.page", json!({"cursor":page["nextCursor"]}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::SelectionStale
    );
    let restricted = s
        .handle("restricted", "models.page", json!({}))
        .await
        .unwrap();
    assert!(
        restricted["models"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["routeId"] == "route-personal")
    );
    s.handle("restricted", "grants.revoke", json!({}))
        .await
        .unwrap();
    assert!(
        s.handle("restricted", "models.page", json!({}))
            .await
            .unwrap()["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn pages_validate_limits_and_never_return_oversized_frames() {
    let (s, state) = setup(20).await;
    for p in [
        json!({"limit":0}),
        json!({"limit":501}),
        json!({"limit":-1}),
        json!({"cursor":"garbage"}),
        json!({"clientId":"b"}),
    ] {
        assert!(s.handle("a", "models.page", p).await.is_err());
    }
    for m in &mut state.lock().unwrap().models {
        m.name = "x".repeat(40_000);
    }
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let page = s
        .handle("a", "models.page", json!({"limit":500}))
        .await
        .unwrap();
    assert!(serde_json::to_vec(&page).unwrap().len() < 262_144);
    assert!(!page["nextCursor"].is_null());
    state.lock().unwrap().models[0].name = "x".repeat(300_000);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    assert_eq!(
        s.handle("a", "models.page", json!({}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::FrameTooLarge
    );
}
#[tokio::test]
async fn empty_catalog_returns_a_terminal_page_without_scanning() {
    let s = NativeService::new(Arc::new(|_| panic!("page must not scan")), Duration::ZERO);
    s.register_client("a", ClientPolicy::passive()).unwrap();
    let page = s.handle("a", "models.page", json!({})).await.unwrap();
    assert_eq!(page["models"], json!([]));
    assert!(page["nextCursor"].is_null());
}
