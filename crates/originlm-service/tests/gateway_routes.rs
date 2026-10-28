#[allow(dead_code)]
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_service::*;
use serde_json::json;
use std::{collections::BTreeSet, sync::Arc, time::Duration};
fn create(escape: bool) -> NativeService {
    NativeService::new(
        Arc::new(|_| {
            let mut snapshot = common::fixture();
            snapshot.models.clear();
            let locations = snapshot
                .connections
                .iter()
                .map(|c| {
                    (
                        c.id.clone(),
                        LocatedClient {
                            executable_identity: None,
                            provider_id: "gateway".into(),
                            profile_home: std::env::temp_dir(),
                            executable: None,
                        },
                    )
                })
                .collect();
            Ok(ScanResult {
                snapshot,
                locations,
                metadata_probes: 0,
            })
        }),
        Duration::from_secs(60),
    )
    .with_metadata(Arc::new(move |_, connection, route| {
        Box::pin(async move {
            let evidence = EvidenceSource::new(EvidenceKind::Synthetic, "gateway", "fixture");
            let child = stable_id("route", &[&route, "upstream"]);
            let model = Model {
                id: route_model_key(&child, "same-native"),
                route_id: child.clone(),
                native_id: "same-native".into(),
                name: "Model".into(),
                revision: "1".into(),
                is_default: false,
                capabilities: vec![],
                options: vec![],
                context_window: Observation::unknown("NOT_REPORTED"),
            };
            Ok(ConnectionMetadata {
                connection_id: connection.clone(),
                route_id: route,
                version: Observation::unknown("TEST"),
                auth: Observation::reported(AuthState::Authenticated, evidence.clone()),
                plan: Observation::unknown("NOT_REPORTED"),
                billing: Observation::unknown("GATEWAY_HAS_ROUTE_SPECIFIC_BILLING"),
                account: None,
                models: vec![model],
                additional_routes: vec![AccessRoute {
                    id: child,
                    connection_id: if escape { "work".into() } else { connection },
                    provider_id: "upstream".into(),
                    display_name: "Upstream".into(),
                    billing: Observation::reported(BillingMode::ApiMetered, evidence),
                    capabilities: vec![],
                    catalog_revision: "1".into(),
                }],
                providers: vec![Provider {
                    id: "upstream".into(),
                    name: "Upstream".into(),
                    kind: "api".into(),
                }],
                quota_pools: vec![],
                included_usage_allowed: Observation::unknown("NOT_REPORTED"),
                diagnostics: vec![],
            })
        })
    }))
}
#[tokio::test]
async fn gateway_subroute_keeps_model_vendor_and_billing_identity() {
    let s = create(false);
    s.register_client(
        "owner",
        ClientPolicy {
            probe_connection_ids: BTreeSet::from(["personal".into()]),
            ..ClientPolicy::passive()
        },
    )
    .unwrap();
    s.handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "owner",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    let snapshot = s.handle("owner", "snapshot.get", json!({})).await.unwrap();
    let model = &snapshot["models"][0];
    let route = snapshot["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == model["routeId"])
        .unwrap();
    assert_eq!(route["providerId"], "upstream");
    assert_eq!(route["connectionId"], "personal");
    assert_eq!(route["billing"]["value"], "api_metered");
    assert!(
        snapshot["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "upstream")
    );
}
#[tokio::test]
async fn gateway_cannot_attach_model_routes_to_another_account() {
    let s = create(true);
    s.register_client(
        "owner",
        ClientPolicy {
            probe_connection_ids: BTreeSet::from(["personal".into()]),
            ..ClientPolicy::passive()
        },
    )
    .unwrap();
    s.handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert!(
        s.handle(
            "owner",
            "connections.probe",
            json!({"connectionId":"personal"})
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn gateway_metadata_does_not_claim_an_execution_driver_exists() {
    let s = create(false);
    s.register_client(
        "owner",
        ClientPolicy {
            probe_connection_ids: BTreeSet::from(["personal".into()]),
            ..ClientPolicy::passive()
        },
    )
    .unwrap();
    s.handle("owner", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "owner",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    let value = s.handle("owner", "snapshot.get", json!({})).await.unwrap();
    let child = value["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["providerId"] == "upstream")
        .unwrap();
    assert!(
        child["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "generation.text"
                && c["origin"] == "runtime"
                && c["support"] == "unsupported")
    );
}
