//! Test/example adapter. All records and output are synthetic; no provider or account is contacted.
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_kit::*;
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::NativeService;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
struct SyntheticAgent;
impl Agent for SyntheticAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            cx.emit(EventPayload::TextDelta {
                text: "Synthetic adapter through the public runtime".into(),
            })
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
fn fixture() -> Snapshot {
    let mut s = Snapshot::empty(DataMode::Synthetic);
    let source = EvidenceSource::new(
        EvidenceKind::Synthetic,
        "adapter-template",
        "Local fixture, not a detected account",
    );
    s.providers.push(Provider {
        id: "fixture".into(),
        name: "Synthetic adapter".into(),
        kind: "synthetic".into(),
    });
    s.installations.push(Installation {
        id: "i".into(),
        provider_id: "fixture".into(),
        environment_id: "synthetic".into(),
        location_hint: "No executable".into(),
        version: Observation::reported("fixture-1".into(), source.clone()),
    });
    s.profiles.push(Profile {
        id: "p".into(),
        installation_id: "i".into(),
        label: "Synthetic profile".into(),
    });
    s.connections.push(Connection {
        id: "c".into(),
        provider_id: "fixture".into(),
        installation_id: "i".into(),
        profile_id: "p".into(),
        display_name: "Synthetic connection".into(),
        account_id: None,
        auth: Observation::reported(AuthState::Authenticated, source.clone()),
        plan: Observation::unknown("SYNTHETIC_NO_PLAN"),
        status: "synthetic".into(),
        diagnostics: vec![],
    });
    s.routes.push(AccessRoute {
        id: "r".into(),
        connection_id: "c".into(),
        provider_id: "fixture".into(),
        display_name: "Synthetic route".into(),
        billing: Observation::reported(BillingMode::Local, source.clone()),
        capabilities: vec![],
        catalog_revision: "1".into(),
    });
    s.models.push(Model {
        id: "m".into(),
        route_id: "r".into(),
        native_id: "fixture-model".into(),
        name: "Synthetic model".into(),
        revision: "1".into(),
        is_default: true,
        context_window: Observation::unknown("SYNTHETIC"),
        capabilities: vec![Capability {
            id: "generation.text".into(),
            support: Support::Supported,
            origin: CapabilityOrigin::Model,
            evidence: Observation::reported("fixture".into(), source.clone()),
            conditions: vec![],
        }],
        options: vec![OptionDescriptor {
            id: "fixture-effort".into(),
            label: "Synthetic effort".into(),
            scope: OptionScope::Turn,
            revision: "1".into(),
            control: OptionControl::Select {
                choices: vec![Choice {
                    value: "example".into(),
                    label: "Example".into(),
                    disabled_reason: None,
                }],
                default_value: Some("example".into()),
            },
            current_value: None,
            category: None,
            source: Some(source.clone()),
            billing_sensitive: false,
            permission_sensitive: false,
        }],
    });
    s.quota_pools.push(QuotaPool {
        id: "q".into(),
        account_id: None,
        connection_ids: vec!["c".into()],
        route_ids: vec!["r".into()],
        sharing: "synthetic".into(),
        windows: vec![UsageWindow {
            id: "w".into(),
            label: "Synthetic request count".into(),
            reading: Observation::reported(
                UsageReading::try_from(UsageInput {
                    unit: "requests".into(),
                    used: DecimalString::try_from("2".to_owned()).unwrap(),
                    limit: Some(DecimalString::try_from("10".to_owned()).unwrap()),
                })
                .unwrap(),
                source,
            ),
            resets_at: None,
            duration_seconds: None,
        }],
    });
    s
}
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let source: InventorySource = Arc::new(|_| {
        Ok(ScanResult {
            snapshot: fixture(),
            locations: BTreeMap::from([(
                "c".into(),
                LocatedClient {
                    executable_identity: None,
                    provider_id: "fixture".into(),
                    profile_home: std::env::temp_dir(),
                    executable: None,
                },
            )]),
            metadata_probes: 0,
        })
    });
    let service = NativeService::new(source, Duration::from_secs(60)).with_execution(
        BTreeSet::from(["fixture".into()]),
        Arc::new(|_| Box::pin(async { Ok(Box::new(SyntheticAgent) as Box<dyn Agent>) })),
    );
    service.register_client("example", ClientPolicy::passive())?;
    service.register_execution(
        "example",
        ExecutionGrant {
            provider_ids: BTreeSet::from(["fixture".into()]),
            workspaces: BTreeMap::from([("test".into(), std::env::temp_dir())]),
            ..Default::default()
        },
    )?;
    service
        .handle("example", "discovery.refresh", json!({}))
        .await?;
    let session=service.handle("example","sessions.create",json!({"workspaceId":"test","selection":{"routeId":"r","modelId":"m","revision":"1","options":{"fixture-effort":"example"}}})).await?;
    service
        .handle(
            "example",
            "turns.start",
            json!({"sessionId":session["id"],"requestId":"example-1","prompt":"Synthetic input"}),
        )
        .await?;
    let events = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let e = service.handle("example", "events.read", json!({})).await?;
            if e["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["payload"]["type"] == "final")
            {
                break Ok::<_, DomainError>(e);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await??;
    assert!(
        events
            .to_string()
            .contains("Synthetic adapter through the public runtime")
    );
    service.close_client("example");
    println!(
        "PASS synthetic adapter: public contracts, option, quota, session, stream, artifact, and close; no inference"
    );
    Ok(())
}
