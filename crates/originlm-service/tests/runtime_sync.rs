#[allow(dead_code)]
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_service::{MetadataSource, NativeService, NotificationSink};
use serde_json::json;
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn inventory(root: std::path::PathBuf) -> ScanResult {
    let mut snapshot = common::fixture();
    snapshot.models.clear();
    snapshot.accounts.clear();
    snapshot.quota_pools.clear();
    for connection in &mut snapshot.connections {
        connection.provider_id = "codex".into();
        connection.account_id = None;
        connection.auth = Observation::unknown("UNPROBED");
    }
    let locations = snapshot
        .connections
        .iter()
        .map(|connection| {
            (
                connection.id.clone(),
                LocatedClient {
                    executable_identity: None,
                    provider_id: "codex".into(),
                    profile_home: root.join(&connection.id),
                    executable: None,
                    scope: None,
                },
            )
        })
        .collect();
    ScanResult {
        snapshot,
        locations,
        metadata_probes: 0,
    }
}

fn report(id: &str, route: &str, auth: AuthState) -> ConnectionMetadata {
    let mut snapshot = common::fixture();
    snapshot.models.retain(|model| model.route_id == route);
    for pool in &mut snapshot.quota_pools {
        pool.connection_ids = vec![id.into()];
        pool.route_ids = vec![route.into()];
    }
    ConnectionMetadata {
        connection_id: id.into(),
        route_id: route.into(),
        version: Observation::unknown("TEST"),
        auth: Observation::reported(
            auth,
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture"),
        ),
        plan: Observation::reported(
            Plan {
                native_id: "synthetic-plan".into(),
                label: "Synthetic plan".into(),
            },
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture"),
        ),
        billing: Observation::reported(
            BillingMode::Subscription,
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture"),
        ),
        account: snapshot
            .accounts
            .into_iter()
            .find(|account| account.id == format!("account-{id}")),
        models: snapshot.models,
        additional_routes: vec![],
        providers: vec![],
        quota_pools: snapshot.quota_pools,
        included_usage_allowed: Observation::unknown("NOT_REPORTED"),
        diagnostics: vec![],
    }
}

fn policy() -> ClientPolicy {
    ClientPolicy {
        probe_connection_ids: BTreeSet::from(["personal".into()]),
        ..ClientPolicy::passive()
    }
}

fn service_builder(
    root: std::path::PathBuf,
    calls: Arc<AtomicUsize>,
    delay: Duration,
    auth: AuthState,
) -> NativeService {
    let source: MetadataSource = Arc::new(move |_, id, route| {
        let calls = calls.clone();
        let auth = auth.clone();
        Box::pin(async move {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(delay).await;
            Ok(report(&id, &route, auth))
        })
    });
    NativeService::new(
        Arc::new(move |_| {
            let scan = inventory(root.clone());
            Ok(scan)
        }),
        Duration::from_secs(0),
    )
    .with_metadata(source)
}

fn service(
    root: std::path::PathBuf,
    calls: Arc<AtomicUsize>,
    delay: Duration,
    auth: AuthState,
) -> Arc<NativeService> {
    Arc::new(service_builder(root, calls, delay, auth))
}

fn roots() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    for name in ["personal", "work"] {
        std::fs::create_dir(temp.path().join(name)).unwrap();
    }
    let path = temp.path().to_path_buf();
    (temp, path)
}

#[tokio::test]
async fn metadata_advances_the_revision_and_replays_exactly_once() {
    let (_temp, root) = roots();
    let s = service(
        root,
        Arc::new(AtomicUsize::new(0)),
        Duration::ZERO,
        AuthState::Authenticated,
    );
    s.register_client("app", policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let before = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    let before_cursor = before["cursor"].as_str().unwrap().to_owned();
    s.handle(
        "app",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    let after = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    assert_ne!(before["revision"], after["revision"]);
    assert_ne!(before_cursor, after["cursor"]);
    let page = s
        .handle(
            "app",
            "events.read",
            json!({"scope":"inventory","after":before_cursor}),
        )
        .await
        .unwrap();
    let changes = page["events"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["reason"], "metadata");
    assert_eq!(page["tailCursor"], after["cursor"]);
    let replay = s
        .handle(
            "app",
            "events.read",
            json!({"scope":"inventory","after":page["cursor"]}),
        )
        .await
        .unwrap();
    assert!(replay["events"].as_array().unwrap().is_empty());
    let gap = s
        .handle(
            "app",
            "events.read",
            json!({"scope":"inventory","after":"previous-epoch:0"}),
        )
        .await
        .unwrap_err();
    assert_eq!(gap.code, ErrorCode::EventGap);
}

#[tokio::test]
async fn an_older_probe_cannot_overwrite_a_newer_revision() {
    let (_temp, root) = roots();
    let calls = Arc::new(AtomicUsize::new(0));
    let s = service(
        root,
        calls.clone(),
        Duration::from_millis(200),
        AuthState::Authenticated,
    );
    s.register_client("app", policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let probe = {
        let s = s.clone();
        tokio::spawn(async move {
            s.handle(
                "app",
                "connections.probe",
                json!({"connectionId":"personal"}),
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    // A new scan commits a newer revision while the old metadata probe is in flight.
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let result = probe.await.unwrap();
    assert_eq!(result.unwrap_err().code, ErrorCode::SelectionStale);
    let snapshot = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    assert!(snapshot["models"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn expiry_creates_actionable_health_and_snooze_does_not_resolve_it() {
    let directory = tempfile::tempdir().unwrap();
    let (_temp, root) = roots();
    let s = service(
        root,
        Arc::new(AtomicUsize::new(0)),
        Duration::ZERO,
        AuthState::Expired,
    );
    s.enable_metadata_cache(directory.path().join("state.db"));
    s.register_client("app", policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "app",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    let health = s.handle("app", "health.get", json!({})).await.unwrap();
    assert_eq!(health["summary"]["status"], "degraded");
    assert!(
        health["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| { issue["kind"] == "auth_expired" && issue["severity"] == "critical" })
    );
    let issue_id = health["issues"][0]["id"].as_str().unwrap().to_owned();
    let snoozed = s
        .handle(
            "app",
            "issues.snooze",
            json!({"issueId":issue_id,"until":"2099-01-01T00:00:00.000Z"}),
        )
        .await
        .unwrap();
    assert_eq!(snoozed["issue"]["state"], "snoozed");
    let health = s.handle("app", "health.get", json!({})).await.unwrap();
    assert!(
        health["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["id"] == issue_id && issue["state"] == "snoozed")
    );
    // A snooze suppresses alerts but does not pretend the account recovered.
    let decisions = s
        .handle("app", "notifications.poll", json!({}))
        .await
        .unwrap();
    assert!(
        decisions.as_array().unwrap().iter().all(|decision| {
            decision["issueId"] != issue_id || decision["shouldNotify"] == false
        })
    );
}

#[derive(Default)]
struct RecordingSink(Mutex<Vec<NotificationDecision>>);

impl NotificationSink for RecordingSink {
    fn deliver(&self, decision: &NotificationDecision) -> std::result::Result<(), String> {
        self.0.lock().unwrap().push(decision.clone());
        Ok(())
    }
}

#[tokio::test]
async fn an_installed_notification_sink_receives_only_due_decisions() {
    let (_temp, root) = roots();
    let sink = Arc::new(RecordingSink::default());
    let s = Arc::new(
        service_builder(
            root,
            Arc::new(AtomicUsize::new(0)),
            Duration::ZERO,
            AuthState::Unauthenticated,
        )
        .with_notification_sink(sink.clone()),
    );
    s.register_client("one", policy()).unwrap();
    s.handle("one", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "one",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    s.handle(
        "one",
        "notifications.configure",
        json!({"enabled":true,"minimumSeverity":"warning","reminderAfterSeconds":3600,"privateContent":true}),
    )
    .await
    .unwrap();
    s.handle("one", "notifications.poll", json!({}))
        .await
        .unwrap();
    let delivered = sink.0.lock().unwrap().clone();
    assert!(!delivered.is_empty());
    assert!(delivered.iter().all(|decision| decision.should_notify));
    assert!(
        delivered
            .iter()
            .all(|decision| decision.title == "OriginLM needs attention")
    );

    // The coordinator still deduplicates: a second poll delivers nothing.
    let before = delivered.len();
    s.handle("one", "notifications.poll", json!({}))
        .await
        .unwrap();
    assert_eq!(sink.0.lock().unwrap().len(), before);
}

#[tokio::test]
async fn notifications_are_deduplicated_across_clients_and_respect_privacy() {
    let (_temp, root) = roots();
    let s = service(
        root,
        Arc::new(AtomicUsize::new(0)),
        Duration::ZERO,
        AuthState::Unauthenticated,
    );
    s.register_client("one", policy()).unwrap();
    s.register_client("two", policy()).unwrap();
    s.handle("one", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "one",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    let prefs = s
        .handle(
            "one",
            "notifications.configure",
            json!({"enabled":true,"minimumSeverity":"warning","reminderAfterSeconds":3600,"privateContent":true}),
        )
        .await
        .unwrap();
    assert_eq!(prefs["privateContent"], true);
    let first = s
        .handle("one", "notifications.poll", json!({}))
        .await
        .unwrap();
    let first_count = first
        .as_array()
        .unwrap()
        .iter()
        .filter(|decision| decision["shouldNotify"] == true)
        .count();
    assert!(first_count >= 1);
    assert!(
        first
            .as_array()
            .unwrap()
            .iter()
            .filter(|decision| decision["shouldNotify"] == true)
            .all(|decision| decision["title"] == "OriginLM needs attention")
    );
    let second = s
        .handle("two", "notifications.poll", json!({}))
        .await
        .unwrap();
    assert_eq!(
        second
            .as_array()
            .unwrap()
            .iter()
            .filter(|decision| decision["shouldNotify"] == true)
            .count(),
        0
    );
}

#[tokio::test]
async fn issue_and_snapshot_projection_hide_inaccessible_connections() {
    let (_temp, root) = roots();
    let s = service(
        root,
        Arc::new(AtomicUsize::new(0)),
        Duration::ZERO,
        AuthState::Expired,
    );
    let all = {
        let mut policy = policy();
        policy.connection_ids = None;
        policy
    };
    s.register_client("all", all).unwrap();
    s.register_client("one", policy()).unwrap();
    s.handle("all", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "all",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    let one = s.handle("one", "issues.list", json!({})).await.unwrap();
    assert!(
        one.as_array().unwrap().iter().all(|issue| {
            issue["connectionId"].is_null() || issue["connectionId"] == "personal"
        })
    );
    let denied = ClientPolicy::denied();
    s.register_client("denied", denied).unwrap();
    let hidden = s.handle("denied", "issues.list", json!({})).await.unwrap();
    assert!(hidden.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn invalidation_is_debounced_then_the_autonomous_lane_rechecks_it() {
    let (_temp, root) = roots();
    let calls = Arc::new(AtomicUsize::new(0));
    let s = service(
        root.clone(),
        calls.clone(),
        Duration::ZERO,
        AuthState::Authenticated,
    );
    s.enable_autonomous_refresh(BTreeSet::from(["codex".into()]));
    s.register_client("app", policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "app",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    s.handle("app", "discovery.watch", json!({"enabled":true}))
        .await
        .unwrap();
    assert!(s.start_background_tasks());
    std::fs::write(root.join("personal/auth.json"), "changed").unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let snapshot = s.handle("app", "snapshot.get", json!({})).await.unwrap();
            let ready = snapshot["connections"]
                .as_array()
                .unwrap()
                .iter()
                .find(|connection| connection["id"] == "personal")
                .is_some_and(|connection| connection["status"] == "metadata_ready");
            if ready && calls.load(Ordering::SeqCst) >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("the autonomous lane must re-read invalidated metadata");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    s.stop_background_tasks();
}
