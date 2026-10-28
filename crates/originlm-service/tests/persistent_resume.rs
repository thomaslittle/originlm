//! Durable resume tokens: survive helper restarts, revalidate identity, never
//! silently start a fresh conversation.
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
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
const NATIVE: &str = "native-secret-session";
#[derive(Debug, Clone, PartialEq)]
struct Open {
    persistent: bool,
    native_resume: Option<String>,
    revision: String,
    hosts: usize,
}
type Opens = Arc<Mutex<Vec<Open>>>;
struct PersistAgent(bool);
impl Agent for PersistAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        context: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            context.emit(EventPayload::TextDelta {
                text: "fixture reply".into(),
            })
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn native_resumption(&self) -> bool {
        self.0
    }
    fn suspend(&mut self) -> BoxFuture<'_, String> {
        Box::pin(async { Ok(NATIVE.into()) })
    }
}
struct Helper {
    service: Arc<NativeService>,
    opens: Opens,
    inventory: Arc<Mutex<Snapshot>>,
}
/// One helper process: a fresh service over the given state directory.
fn helper(state: Option<&Path>, workspace: &Path, resumable: bool) -> Helper {
    let opens: Opens = Arc::default();
    let seen = opens.clone();
    let inventory = Arc::new(Mutex::new(common::fixture()));
    let scan = inventory.clone();
    let mut service = NativeService::new(
        Arc::new(move |_| {
            let snapshot = scan.lock().unwrap().clone();
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
            seen.lock().unwrap().push(Open {
                persistent: input.persistent,
                native_resume: input.native_resume.clone(),
                revision: input.model.revision.clone(),
                hosts: input.host_mcp_servers.len(),
            });
            Box::pin(async move {
                if input
                    .native_resume
                    .as_deref()
                    .is_some_and(|id| id != NATIVE)
                {
                    return Err(DomainError::new(ErrorCode::NotFound, "missing"));
                }
                Ok(Box::new(PersistAgent(resumable)) as Box<dyn Agent>)
            })
        }),
    );
    if let Some(state) = state {
        service =
            service.with_persistent_resume(state, BTreeSet::from(["test".into()]), BTreeSet::new());
    }
    let service = Arc::new(service);
    for id in ["conn-1", "other-app"] {
        service
            .register_client(id, ClientPolicy::passive())
            .unwrap();
        service
            .register_execution(
                id,
                ExecutionGrant {
                    connection_ids: BTreeSet::from(["personal".into()]),
                    workspaces: BTreeMap::from([("scratch".into(), workspace.to_path_buf())]),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    service.set_app_identity("conn-1", "example-app").unwrap();
    service
        .set_app_identity("other-app", "someone-else")
        .unwrap();
    Helper {
        service,
        opens,
        inventory,
    }
}
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap();
    (dir, path)
}
async fn call(h: &Helper, client: &str, method: &str, params: Value) -> Result<Value> {
    h.service.handle(client, method, params).await
}
async fn completed_session(h: &Helper) -> Value {
    call(h, "conn-1", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let session = call(
        h,
        "conn-1",
        "sessions.create",
        json!({"selection":common::selection(),"workspaceId":"scratch","persistent":true}),
    )
    .await
    .unwrap();
    call(
        h,
        "conn-1",
        "turns.start",
        json!({"sessionId":session["id"],"prompt":"hi","requestId":"1"}),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let current = call(
                h,
                "conn-1",
                "sessions.get",
                json!({"sessionId":session["id"]}),
            )
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
    session
}
async fn suspend(h: &Helper) -> String {
    let session = completed_session(h).await;
    let suspended = call(
        h,
        "conn-1",
        "sessions.suspend",
        json!({"sessionId":session["id"]}),
    )
    .await
    .unwrap();
    suspended["resumeToken"].as_str().unwrap().to_owned()
}
async fn resume(h: &Helper, client: &str, token: &str) -> Result<Value> {
    call(h, client, "discovery.refresh", json!({}))
        .await
        .unwrap();
    call(h, client, "sessions.resume", json!({"resumeToken":token})).await
}

#[tokio::test]
async fn token_survives_a_helper_restart_and_reattaches_the_same_conversation() {
    let state = tempfile::tempdir().unwrap();
    let (_w, path) = workspace();
    let first = helper(Some(state.path()), &path, true);
    let token = suspend(&first).await;
    assert!(token.starts_with("lai1_"));
    for hidden in [
        NATIVE,
        "account-personal",
        path.to_str().unwrap(),
        "scratch",
    ] {
        assert!(!token.contains(hidden), "token leaks {hidden}");
    }
    drop(first);
    // A restarted helper: new service, new connection ids, same state dir.
    let second = helper(Some(state.path()), &path, true);
    let resumed = resume(&second, "conn-1", &token).await.unwrap();
    assert_eq!(resumed["nativeResumption"], true);
    assert_eq!(
        second.opens.lock().unwrap().last().unwrap(),
        &Open {
            persistent: true,
            native_resume: Some(NATIVE.into()),
            revision: "1".into(),
            hosts: 0
        }
    );
    // One live reattachment per conversation.
    assert_eq!(
        resume(&second, "conn-1", &token).await.unwrap_err().code,
        ErrorCode::Conflict
    );
    // The token stays host-storable: after closing, it resumes again.
    call(
        &second,
        "conn-1",
        "sessions.close",
        json!({"sessionId":resumed["id"]}),
    )
    .await
    .unwrap();
    resume(&second, "conn-1", &token).await.unwrap();
}
#[tokio::test]
async fn handshake_and_routes_advertise_persistent_resume() {
    let state = tempfile::tempdir().unwrap();
    let (_w, path) = workspace();
    let h = helper(Some(state.path()), &path, true);
    let handshake = call(&h, "conn-1", "runtime.handshake", json!({}))
        .await
        .unwrap();
    assert!(
        handshake["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("session.resume.persistent"))
    );
    call(&h, "conn-1", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let snapshot = call(&h, "conn-1", "snapshot.get", json!({})).await.unwrap();
    let route = snapshot["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "route-personal")
        .unwrap();
    let cap = route["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "session.resume.persistent")
        .expect("route capability");
    assert_eq!(cap["support"], "supported");
    // No key file until a token is issued.
    assert!(!state.path().join("resume.key").exists());
    // Without a state directory the capability is absent.
    let plain = helper(None, &path, true);
    let handshake = call(&plain, "conn-1", "runtime.handshake", json!({}))
        .await
        .unwrap();
    assert!(
        !handshake["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("session.resume.persistent"))
    );
}
#[tokio::test]
async fn persistent_requires_a_state_dir_and_a_resumable_route() {
    let (_w, path) = workspace();
    let create = json!({"selection":common::selection(),"workspaceId":"scratch","persistent":true});
    let plain = helper(None, &path, true);
    call(&plain, "conn-1", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert_eq!(
        call(&plain, "conn-1", "sessions.create", create.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let state = tempfile::tempdir().unwrap();
    let unresumable = helper(Some(state.path()), &path, false);
    call(&unresumable, "conn-1", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert_eq!(
        call(&unresumable, "conn-1", "sessions.create", create)
            .await
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[tokio::test]
async fn model_revision_changes_are_tolerated_for_the_same_model_id() {
    let state = tempfile::tempdir().unwrap();
    let (_w, path) = workspace();
    let token = suspend(&helper(Some(state.path()), &path, true)).await;
    let second = helper(Some(state.path()), &path, true);
    for model in &mut second.inventory.lock().unwrap().models {
        model.revision = "2".into();
    }
    resume(&second, "conn-1", &token).await.unwrap();
    assert_eq!(second.opens.lock().unwrap().last().unwrap().revision, "2");
}
#[tokio::test]
async fn identity_changes_refuse_with_resume_unavailable() {
    let state = tempfile::tempdir().unwrap();
    let (_w, path) = workspace();
    let token = suspend(&helper(Some(state.path()), &path, true)).await;
    type Change = Box<dyn Fn(&mut Snapshot)>;
    let changes: Vec<(&str, Change)> = vec![
        (
            "account",
            Box::new(|s: &mut Snapshot| {
                for c in &mut s.connections {
                    c.account_id = Some("account-someone-else".into());
                }
            }),
        ),
        (
            "model",
            Box::new(|s: &mut Snapshot| s.models.retain(|m| m.id != "model-personal")),
        ),
        (
            "route",
            Box::new(|s: &mut Snapshot| s.routes.retain(|r| r.id != "route-personal")),
        ),
    ];
    for (label, change) in changes {
        let h = helper(Some(state.path()), &path, true);
        change(&mut h.inventory.lock().unwrap());
        let error = resume(&h, "conn-1", &token).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::ResumeUnavailable, "{label}");
        assert!(!error.message.contains(NATIVE));
        assert!(
            h.opens.lock().unwrap().is_empty(),
            "{label}: no fresh session"
        );
    }
    // Workspace grant now points elsewhere.
    let (_other, other_path) = workspace();
    let h = helper(Some(state.path()), &other_path, true);
    assert_eq!(
        resume(&h, "conn-1", &token).await.unwrap_err().code,
        ErrorCode::ResumeUnavailable
    );
    // Another application on the same helper.
    let h = helper(Some(state.path()), &path, true);
    assert_eq!(
        resume(&h, "other-app", &token).await.unwrap_err().code,
        ErrorCode::ResumeUnavailable
    );
    // Another installation's key.
    let foreign = tempfile::tempdir().unwrap();
    let h = helper(Some(foreign.path()), &path, true);
    assert_eq!(
        resume(&h, "conn-1", &token).await.unwrap_err().code,
        ErrorCode::ResumeUnavailable
    );
    // Durable resume disabled entirely.
    let h = helper(None, &path, true);
    assert_eq!(
        resume(&h, "conn-1", &token).await.unwrap_err().code,
        ErrorCode::ResumeUnavailable
    );
}
#[tokio::test]
async fn missing_native_history_is_resume_unavailable_not_a_new_chat() {
    let state = tempfile::tempdir().unwrap();
    let (_w, path) = workspace();
    let first = helper(Some(state.path()), &path, true);
    // A conversation whose native id the adapter no longer knows.
    let token = {
        let session = completed_session(&first).await;
        call(
            &first,
            "conn-1",
            "sessions.suspend",
            json!({"sessionId":session["id"]}),
        )
        .await
        .unwrap()["resumeToken"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let second = helper(Some(state.path()), &path, true);
    // Swap the adapter's knowledge: every reattach is "not found".
    let broken = Arc::new(
        NativeService::new(
            {
                let inventory = second.inventory.clone();
                Arc::new(move |_| {
                    let snapshot = inventory.lock().unwrap().clone();
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
                })
            },
            Duration::ZERO,
        )
        .with_execution(
            BTreeSet::from(["test".into()]),
            Arc::new(|_| {
                Box::pin(async {
                    Err(DomainError::new(ErrorCode::NotFound, "native history gone"))
                })
            }),
        )
        .with_persistent_resume(
            state.path(),
            BTreeSet::from(["test".into()]),
            BTreeSet::new(),
        ),
    );
    broken
        .register_client("conn-9", ClientPolicy::passive())
        .unwrap();
    broken.set_app_identity("conn-9", "example-app").unwrap();
    broken
        .register_execution(
            "conn-9",
            ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("scratch".into(), path.clone())]),
                ..Default::default()
            },
        )
        .unwrap();
    broken
        .handle("conn-9", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let error = broken
        .handle("conn-9", "sessions.resume", json!({"resumeToken":token}))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResumeUnavailable);
}
#[tokio::test]
async fn host_servers_are_resupplied_on_resume_never_stored() {
    let state = tempfile::tempdir().unwrap();
    let (_w, path) = workspace();
    let secret = "host-bearer-token-that-must-not-be-sealed-0123";
    let first = helper(Some(state.path()), &path, true);
    call(&first, "conn-1", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let server = json!({"name":"design","url":"http://127.0.0.1:4000/mcp","bearerToken":secret,"allowedTools":["echo"]});
    let session = call(
        &first,
        "conn-1",
        "sessions.create",
        json!({"selection":common::selection(),"workspaceId":"scratch","persistent":true,"hostMcpServers":[server.clone()]}),
    )
    .await
    .unwrap();
    call(
        &first,
        "conn-1",
        "turns.start",
        json!({"sessionId":session["id"],"prompt":"hi","requestId":"1"}),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let token = call(
        &first,
        "conn-1",
        "sessions.suspend",
        json!({"sessionId":session["id"]}),
    )
    .await
    .unwrap()["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let second = helper(Some(state.path()), &path, true);
    call(&second, "conn-1", "discovery.refresh", json!({}))
        .await
        .unwrap();
    call(
        &second,
        "conn-1",
        "sessions.resume",
        json!({"resumeToken":token,"hostMcpServers":[server]}),
    )
    .await
    .unwrap();
    assert_eq!(second.opens.lock().unwrap().last().unwrap().hosts, 1);
    let key = std::fs::read(state.path().join("resume.key")).unwrap();
    assert_eq!(key.len(), 32);
}
