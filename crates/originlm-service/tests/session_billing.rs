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
    sync::Arc,
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
#[tokio::test]
async fn host_billing_allowance_does_not_widen_session_consent() {
    let s = NativeService::new(
        Arc::new(|_| {
            let mut snapshot = common::fixture();
            for m in &mut snapshot.models {
                m.options.push(OptionDescriptor {
                    id: "paidOption".into(),
                    label: "Paid".into(),
                    scope: OptionScope::Turn,
                    revision: "1".into(),
                    control: OptionControl::Boolean,
                    current_value: None,
                    category: None,
                    source: None,
                    billing_sensitive: true,
                    permission_sensitive: false,
                });
            }
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
    .with_execution(
        BTreeSet::from(["test".into()]),
        Arc::new(|_| Box::pin(async { Ok(Box::new(Noop) as Box<dyn Agent>) })),
    );
    s.register_client("a", ClientPolicy::passive()).unwrap();
    s.register_execution(
        "a",
        ExecutionGrant {
            connection_ids: BTreeSet::from(["personal".into()]),
            workspaces: BTreeMap::from([("default".into(), std::env::temp_dir())]),
            allow_paid: true,
            ..Default::default()
        },
    )
    .unwrap();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let session = s
        .handle(
            "a",
            "sessions.create",
            json!({"selection":common::selection(),"workspaceId":"default","allowPaid":false}),
        )
        .await
        .unwrap();
    assert_eq!(
        s.handle(
            "a",
            "sessions.updateOptions",
            json!({"sessionId":session["id"],"options":{"paidOption":true}})
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::BillingConsentRequired
    );
}
