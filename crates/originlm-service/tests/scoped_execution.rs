#[allow(dead_code)]
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::{InventorySource, NativeService};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
struct Never;
impl Agent for Never {
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
async fn connection_scoped_executor_does_not_bless_sibling_account() {
    let source: InventorySource = Arc::new(|_: &AtomicBool| {
        let mut s = common::fixture();
        for r in &mut s.routes {
            r.capabilities.push(Capability {
                id: "generation.text".into(),
                support: Support::Unsupported,
                origin: CapabilityOrigin::Runtime,
                evidence: Observation::reported(
                    "driver pending".into(),
                    EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture"),
                ),
                conditions: vec![],
            });
        }
        Ok(ScanResult {
            snapshot: s,
            locations: Default::default(),
            metadata_probes: 0,
        })
    });
    let service = NativeService::new(source, Duration::ZERO).with_execution_scoped(
        BTreeSet::new(),
        BTreeSet::from(["personal".into()]),
        Arc::new(|_| Box::pin(async { Ok(Box::new(Never) as Box<dyn Agent>) })),
    );
    service
        .register_client("app", ClientPolicy::passive())
        .unwrap();
    service
        .handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let s = service
        .handle("app", "snapshot.get", json!({}))
        .await
        .unwrap();
    let routes = s["routes"].as_array().unwrap();
    let support = |id: &str| {
        routes.iter().find(|r| r["connectionId"] == id).unwrap()["capabilities"][0]["support"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(support("personal"), "supported");
    assert_eq!(support("work"), "unsupported");
}
