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
struct Driver;
impl Agent for Driver {
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
fn setup(trust: bool) -> (NativeService, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let source: InventorySource = Arc::new(|_| {
        Ok(ScanResult {
            snapshot: common::fixture(),
            locations: BTreeMap::from([(
                "personal".into(),
                LocatedClient {
                    executable_identity: None,
                    provider_id: "test".into(),
                    profile_home: std::env::temp_dir(),
                    executable: None,
                },
            )]),
            metadata_probes: 0,
        })
    });
    let service = NativeService::new(source, Duration::from_secs(60)).with_execution(
        BTreeSet::from(["test".into()]),
        Arc::new(move |input| {
            assert!(input.native_policy);
            count.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(Box::new(Driver) as Box<dyn Agent>) })
        }),
    );
    service
        .register_client("a", ClientPolicy::passive())
        .unwrap();
    service
        .register_execution(
            "a",
            ExecutionGrant {
                provider_ids: BTreeSet::from(["test".into()]),
                workspaces: BTreeMap::from([("safe".into(), std::env::temp_dir())]),
                allow_native_policy: trust,
                ..Default::default()
            },
        )
        .unwrap();
    (service, calls)
}
#[tokio::test]
async fn native_client_policy_needs_separate_host_authorization() {
    let (s, c) = setup(false);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    assert_eq!(s.handle("a","sessions.create",json!({"selection":common::selection(),"workspaceId":"safe","permissionMode":"native_policy"})).await.unwrap_err().code,ErrorCode::LocalAccessDenied);
    assert_eq!(c.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn explicit_native_policy_is_preserved_for_the_provider_not_claimed_readonly() {
    let (s, c) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let v=s.handle("a","sessions.create",json!({"selection":common::selection(),"workspaceId":"safe","permissionMode":"native_policy","hostMcpServers":[{"name":"design","url":"http://127.0.0.1:48123/mcp","bearerToken":"host-secret-token-0123456789abcdef","allowedTools":["echo_color"]}]})).await.unwrap();
    assert!(v["id"].is_string());
    assert_eq!(c.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn native_policy_does_not_bypass_workspace_or_billing_grants() {
    let (s, c) = setup(true);
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    for extra in [json!({"workspaceId":"unknown"}), json!({"allowPaid":true})] {
        let mut p = json!({"selection":common::selection(),"workspaceId":"safe","permissionMode":"native_policy"});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        assert!(s.handle("a", "sessions.create", p).await.is_err());
    }
    assert_eq!(c.load(Ordering::SeqCst), 0);
}
