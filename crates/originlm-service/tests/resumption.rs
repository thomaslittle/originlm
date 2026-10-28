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
    sync::{Arc, Mutex},
    time::Duration,
};
type ResumeOpens = Arc<Mutex<Vec<Option<String>>>>;
struct ResumeAgent;
impl Agent for ResumeAgent {
    fn turn<'a>(
        &'a mut self,
        prompt: String,
        _: BTreeMap<String, Value>,
        mut context: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if prompt == "complete" {
                return context.emit(EventPayload::TextDelta {
                    text: "fixture reply".into(),
                });
            }
            context.cancelled().await;
            Err(DomainError::new(ErrorCode::Cancelled, "fixture cancelled"))
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn native_resumption(&self) -> bool {
        true
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async { Ok("native-secret-session".into()) })
    }
}
fn setup_with_inventory(
    workspace: std::path::PathBuf,
) -> (Arc<NativeService>, ResumeOpens, Arc<Mutex<Snapshot>>) {
    let opens = Arc::new(Mutex::new(vec![]));
    let seen = opens.clone();
    let inventory = Arc::new(Mutex::new(common::fixture()));
    let scan_inventory = inventory.clone();
    let s = Arc::new(
        NativeService::new(
            Arc::new(move |_| {
                let snapshot = scan_inventory
                    .lock()
                    .map_err(|_| DomainError::new(ErrorCode::ProviderUnavailable, "fixture"))?
                    .clone();
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
            Duration::ZERO,
        )
        .with_execution(
            BTreeSet::from(["test".into()]),
            Arc::new(move |input| {
                seen.lock().unwrap().push(input.native_resume.clone());
                Box::pin(async move {
                    if input
                        .native_resume
                        .as_deref()
                        .is_some_and(|id| id != "native-secret-session")
                    {
                        return Err(DomainError::new(
                            ErrorCode::NotFound,
                            "native session missing",
                        ));
                    }
                    Ok(Box::new(ResumeAgent) as Box<dyn Agent>)
                })
            }),
        ),
    );
    for id in ["a", "b"] {
        s.register_client(id, ClientPolicy::passive()).unwrap();
        s.register_execution(
            id,
            ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("scratch".into(), workspace.clone())]),
                ..Default::default()
            },
        )
        .unwrap();
    }
    (s, opens, inventory)
}
fn setup() -> (Arc<NativeService>, ResumeOpens) {
    let (service, opens, _) = setup_with_inventory(std::env::temp_dir());
    (service, opens)
}
fn create() -> Value {
    json!({"selection":common::selection(),"workspaceId":"scratch"})
}
#[tokio::test]
async fn completed_conversation_can_suspend_and_resume_without_a_fresh_session() {
    let (s, opens) = setup();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let session = s.handle("a", "sessions.create", create()).await.unwrap();
    s.handle(
        "a",
        "turns.start",
        json!({"sessionId":session["id"],"prompt":"complete","requestId":"completed"}),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let current = s
                .handle("a", "sessions.get", json!({"sessionId":session["id"]}))
                .await
                .unwrap();
            if current["state"] == "completed" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let suspended = s
        .handle("a", "sessions.suspend", json!({"sessionId":session["id"]}))
        .await
        .unwrap();
    let resumed = s
        .handle(
            "a",
            "sessions.resume",
            json!({"resumeToken":suspended["resumeToken"]}),
        )
        .await
        .unwrap();
    assert_eq!(resumed["nativeResumption"], true);
    assert_eq!(
        *opens.lock().unwrap(),
        vec![None, Some("native-secret-session".into())]
    );
}
#[tokio::test]
async fn resume_tokens_are_opaque_owner_scoped_and_one_time() {
    let (s, opens) = setup();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let session = s.handle("a", "sessions.create", create()).await.unwrap();
    assert_eq!(session["nativeResumption"], true);
    let suspended = s
        .handle("a", "sessions.suspend", json!({"sessionId":session["id"]}))
        .await
        .unwrap();
    let token = suspended["resumeToken"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    assert!(!suspended.to_string().contains("native-secret-session"));
    for method in ["diagnostics.report", "events.read"] {
        let payload = s.handle("a", method, json!({})).await.unwrap().to_string();
        assert!(
            !payload.contains(token),
            "{method} must not reveal the token"
        );
        assert!(
            !payload.contains("native-secret-session"),
            "{method} must not reveal the provider session id"
        );
    }
    assert!(
        s.handle("a", "sessions.get", json!({"sessionId":session["id"]}))
            .await
            .is_err()
    );
    assert_eq!(
        s.handle("b", "sessions.resume", json!({"resumeToken":token}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    let resumed = s
        .handle("a", "sessions.resume", json!({"resumeToken":token}))
        .await
        .unwrap();
    assert_ne!(resumed["id"], session["id"]);
    assert_eq!(resumed["nativeResumption"], true);
    assert_eq!(
        s.handle("a", "sessions.resume", json!({"resumeToken":token}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        *opens.lock().unwrap(),
        vec![None, Some("native-secret-session".into())]
    );
}
#[tokio::test]
async fn revocation_invalidates_unconsumed_resume_tokens() {
    let (s, _) = setup();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let session = s.handle("a", "sessions.create", create()).await.unwrap();
    let token = s
        .handle("a", "sessions.suspend", json!({"sessionId":session["id"]}))
        .await
        .unwrap()["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();
    s.handle("a", "grants.revoke", json!({})).await.unwrap();
    assert_eq!(
        s.handle("a", "sessions.resume", json!({"resumeToken":token}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn active_and_closed_sessions_cannot_be_suspended() {
    let (s, _) = setup();
    s.handle("a", "discovery.refresh", json!({})).await.unwrap();
    let active = s.handle("a", "sessions.create", create()).await.unwrap();
    let started = s
        .handle(
            "a",
            "turns.start",
            json!({"sessionId":active["id"],"prompt":"hold","requestId":"active"}),
        )
        .await
        .unwrap();
    assert_eq!(
        s.handle("a", "sessions.suspend", json!({"sessionId":active["id"]}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    s.handle("a", "turns.cancel", json!({"turnId":started["turnId"]}))
        .await
        .unwrap();

    let closed = s.handle("a", "sessions.create", create()).await.unwrap();
    s.handle("a", "sessions.close", json!({"sessionId":closed["id"]}))
        .await
        .unwrap();
    assert!(
        s.handle("a", "sessions.suspend", json!({"sessionId":closed["id"]}))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn resume_tokens_are_invalid_after_runtime_restart() {
    let (first_runtime, _) = setup();
    first_runtime
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let session = first_runtime
        .handle("a", "sessions.create", create())
        .await
        .unwrap();
    let token = first_runtime
        .handle("a", "sessions.suspend", json!({"sessionId":session["id"]}))
        .await
        .unwrap()["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();

    let (restarted, _) = setup();
    restarted
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert_eq!(
        restarted
            .handle("a", "sessions.resume", json!({"resumeToken":token}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn resume_revalidates_account_model_route_billing_and_workspace_grant() {
    for change in ["account", "model", "route", "billing", "workspace"] {
        let workspace = tempfile::tempdir().unwrap();
        let workspace_path = workspace.path().to_path_buf();
        let (service, opens, inventory) = setup_with_inventory(workspace_path.clone());
        service
            .handle("a", "discovery.refresh", json!({}))
            .await
            .unwrap();
        let session = service
            .handle("a", "sessions.create", create())
            .await
            .unwrap();
        let token = service
            .handle("a", "sessions.suspend", json!({"sessionId":session["id"]}))
            .await
            .unwrap()["resumeToken"]
            .as_str()
            .unwrap()
            .to_owned();

        match change {
            "account" => {
                inventory.lock().unwrap().connections[0].account_id = Some("changed".into());
            }
            "model" => inventory.lock().unwrap().models[0].revision = "2".into(),
            "route" => {
                inventory.lock().unwrap().routes[0].id = "changed-route".into();
                inventory.lock().unwrap().models[0].route_id = "changed-route".into();
            }
            "billing" => {
                inventory.lock().unwrap().routes[0].billing = Observation::reported(
                    BillingMode::ApiMetered,
                    EvidenceSource::new(EvidenceKind::Synthetic, "synthetic", "changed billing"),
                );
            }
            "workspace" => {
                let moved = workspace_path.with_extension("moved");
                std::fs::rename(&workspace_path, &moved).unwrap();
            }
            _ => unreachable!(),
        }
        service
            .handle("a", "discovery.refresh", json!({}))
            .await
            .unwrap();
        assert!(
            service
                .handle("a", "sessions.resume", json!({"resumeToken":token}))
                .await
                .is_err(),
            "{change} change must invalidate native resumption"
        );
        assert_eq!(
            opens.lock().unwrap().len(),
            1,
            "{change} must fail before reopening the provider"
        );
        if change == "workspace" {
            let moved = workspace_path.with_extension("moved");
            std::fs::remove_dir(moved).unwrap();
        }
    }
}
