#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Noop;
impl Agent for Noop {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        _: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
fn service(probes: Arc<AtomicUsize>, launches: Arc<AtomicUsize>) -> NativeService {
    NativeService::new(
        Arc::new(|_| {
            let snapshot = common::fixture();
            let locations = snapshot
                .connections
                .iter()
                .map(|c| {
                    (
                        c.id.clone(),
                        LocatedClient {
                            executable_identity: None,
                            provider_id: "test".into(),
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
        Duration::from_secs(30),
    )
    // The client's live account read now says the user signed out.
    .with_metadata(Arc::new(move |_, connection, route| {
        probes.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let evidence = EvidenceSource::new(EvidenceKind::Synthetic, "test", "live");
            // Same catalog as the inventory, so only the auth verdict changes.
            let models = common::fixture()
                .models
                .into_iter()
                .filter(|m| m.route_id == route)
                .collect();
            Ok(ConnectionMetadata {
                connection_id: connection,
                route_id: route,
                version: Observation::unknown("TEST"),
                auth: Observation::reported(AuthState::Unauthenticated, evidence),
                plan: Observation::unknown("NOT_REPORTED"),
                billing: Observation::unknown("NOT_REPORTED"),
                account: None,
                models,
                additional_routes: vec![],
                providers: vec![],
                quota_pools: vec![],
                included_usage_allowed: Observation::unknown("NOT_REPORTED"),
                diagnostics: vec![],
            })
        })
    }))
    .with_execution(
        BTreeSet::from(["test".into()]),
        Arc::new(move |_| {
            launches.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(Box::new(Noop) as Box<dyn Agent>) })
        }),
    )
}
#[tokio::test]
async fn session_start_rereads_live_auth_instead_of_trusting_cached_login() {
    let probes = Arc::new(AtomicUsize::new(0));
    let launches = Arc::new(AtomicUsize::new(0));
    let s = service(probes.clone(), launches.clone());
    s.register_client("a", ClientPolicy::passive()).unwrap();
    s.register_execution(
        "a",
        ExecutionGrant {
            connection_ids: BTreeSet::from(["personal".into()]),
            workspaces: BTreeMap::from([("default".into(), std::env::temp_dir())]),
            ..Default::default()
        },
    )
    .unwrap();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    // The cached inventory still says Authenticated; only the live read knows better.
    let error = s
        .handle(
            "a",
            "sessions.create",
            json!({"selection":common::selection(),"workspaceId":"default"}),
        )
        .await
        .unwrap_err();
    assert_eq!(probes.load(Ordering::SeqCst), 1);
    assert_eq!(launches.load(Ordering::SeqCst), 0);
    assert_eq!(error.code, ErrorCode::AuthRequired, "{error:?}");
}
