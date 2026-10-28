#[allow(dead_code)]
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_service::{MetadataSource, NativeService};
use serde_json::json;
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
fn inventory() -> ScanResult {
    let mut snapshot = common::fixture();
    snapshot.models.clear();
    snapshot.accounts.clear();
    snapshot.quota_pools.clear();
    for c in &mut snapshot.connections {
        c.provider_id = "codex".into();
        c.account_id = None;
        c.auth = Observation::unknown("UNPROBED");
    }
    let locations = snapshot
        .connections
        .iter()
        .map(|c| {
            (
                c.id.clone(),
                LocatedClient {
                    executable_identity: None,
                    provider_id: "codex".into(),
                    profile_home: std::env::temp_dir().join(&c.id),
                    executable: None,
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
fn report(id: &str, route: &str) -> ConnectionMetadata {
    let mut snapshot = common::fixture();
    let connection = snapshot
        .connections
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .clone();
    snapshot.models.retain(|m| m.route_id == route);
    for pool in &mut snapshot.quota_pools {
        pool.connection_ids = vec![id.into()];
        pool.route_ids = vec![route.into()];
    }
    ConnectionMetadata {
        connection_id: id.into(),
        route_id: route.into(),
        version: Observation::unknown("TEST"),
        auth: connection.auth,
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
            .find(|a| a.id == format!("account-{id}")),
        models: snapshot.models,
        additional_routes: vec![],
        providers: vec![],
        quota_pools: snapshot.quota_pools,
        included_usage_allowed: Observation::unknown("NOT_REPORTED"),
        diagnostics: Vec::new(),
    }
}
fn service(count: Arc<AtomicUsize>, delay: Duration) -> Arc<NativeService> {
    let source: MetadataSource = Arc::new(move |_, id, route| {
        let count = count.clone();
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(delay).await;
            Ok(report(&id, &route))
        })
    });
    Arc::new(
        NativeService::new(Arc::new(|_| Ok(inventory())), Duration::from_secs(60))
            .with_metadata(source),
    )
}
fn probe_policy() -> ClientPolicy {
    ClientPolicy {
        probe_connection_ids: BTreeSet::from(["personal".into()]),
        ..ClientPolicy::passive()
    }
}
#[tokio::test]
async fn discovery_never_runs_client_probe_and_missing_grant_is_denied() {
    let calls = Arc::new(AtomicUsize::new(0));
    let s = service(calls.clone(), Duration::ZERO);
    s.register_client("app", ClientPolicy::passive()).unwrap();
    s.handle("app", "runtime.handshake", json!({}))
        .await
        .unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert_eq!(
        s.handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"})
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::LocalAccessDenied
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn approved_metadata_is_scoped_and_identity_stays_masked() {
    let s = service(Arc::new(AtomicUsize::new(0)), Duration::ZERO);
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let result = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "updated");
    let snapshot = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    assert_eq!(snapshot["models"].as_array().unwrap().len(), 1);
    assert!(!snapshot.to_string().contains("personal@example.test"));
    assert_eq!(
        snapshot["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "personal")
            .unwrap()["plan"]["value"]["nativeId"],
        "synthetic-plan"
    );
    assert_eq!(snapshot["accounts"][0]["displayLabel"]["state"], "unknown");
}
#[tokio::test]
async fn concurrent_probes_share_cached_work_without_repeated_client_spawns() {
    let calls = Arc::new(AtomicUsize::new(0));
    let s = service(calls.clone(), Duration::from_millis(30));
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for _ in 0..12 {
        let s = s.clone();
        tasks.push(tokio::spawn(async move {
            s.handle(
                "app",
                "connections.probe",
                json!({"connectionId":"personal"}),
            )
            .await
            .unwrap()
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn revoked_during_probe_cannot_publish_new_metadata() {
    let calls = Arc::new(AtomicUsize::new(0));
    let s = service(calls.clone(), Duration::from_millis(50));
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let other = s.clone();
    let task = tokio::spawn(async move {
        other
            .handle(
                "app",
                "connections.probe",
                json!({"connectionId":"personal"}),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("metadata source must be reached");
    s.handle("app", "grants.revoke", json!({})).await.unwrap();
    assert_eq!(
        task.await.unwrap().unwrap_err().code,
        ErrorCode::LocalAccessDenied
    );
    assert!(
        s.handle("app", "snapshot.get", json!({})).await.unwrap()["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn one_connection_probe_cannot_overwrite_another_account() {
    let source: MetadataSource =
        Arc::new(|_, _, _| Box::pin(async { Ok(report("work", "route-work")) }));
    let s = NativeService::new(Arc::new(|_| Ok(inventory())), Duration::from_secs(60))
        .with_metadata(source);
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    assert!(
        s.handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"})
        )
        .await
        .is_err()
    );
    assert!(
        s.handle("app", "models.list", json!({}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn provider_scope_must_be_host_set_and_unknown_provider_does_not_spawn() {
    let calls = Arc::new(AtomicUsize::new(0));
    let s = service(calls.clone(), Duration::ZERO);
    s.register_client(
        "app",
        ClientPolicy {
            probe_provider_ids: BTreeSet::from(["codex".into()]),
            ..ClientPolicy::passive()
        },
    )
    .unwrap();
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
    assert!(
        s.handle(
            "app",
            "connections.probe",
            json!({"connectionId":"work","providerId":"codex"})
        )
        .await
        .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn repeated_reads_do_not_invent_new_observation_times_or_revisions() {
    let s = service(Arc::new(AtomicUsize::new(0)), Duration::ZERO);
    s.register_client("app", probe_policy()).unwrap();
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
    let first = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let second = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    assert_eq!(first, second);
}

#[tokio::test]
async fn failed_metadata_probes_back_off_instead_of_respawning_for_each_caller() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let source: MetadataSource = Arc::new(move |_, _, _| {
        let calls = calls.clone();
        Box::pin(async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(DomainError::new(
                ErrorCode::ProviderUnavailable,
                "PRIVATE-ADAPTER-ERROR",
            ))
        })
    });
    let s = NativeService::new(Arc::new(|_| Ok(inventory())), Duration::from_secs(60))
        .with_metadata(source);
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    for _ in 0..3 {
        let error = s
            .handle(
                "app",
                "connections.probe",
                json!({"connectionId":"personal"}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ProviderUnavailable);
        assert!(!error.message.contains("PRIVATE"));
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn provider_retry_after_is_sanitized_preserved_and_expires() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let source: MetadataSource = Arc::new(move |_, _, _| {
        let calls = calls.clone();
        Box::pin(async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(
                DomainError::new(ErrorCode::RateLimited, "PRIVATE-RATE-LIMIT")
                    .with_retry_after_ms(150),
            )
        })
    });
    let s = NativeService::new(Arc::new(|_| Ok(inventory())), Duration::from_secs(60))
        .with_metadata(source);
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let first = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    assert_eq!(first.code, ErrorCode::RateLimited);
    assert!(first.retryable);
    assert!(
        first
            .retry_after_ms
            .is_some_and(|ms| (100..=400).contains(&ms))
    );
    assert!(!first.message.contains("PRIVATE"));
    let cached = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(
        cached
            .retry_after_ms
            .is_some_and(|ms| ms <= first.retry_after_ms.unwrap())
    );
    tokio::time::sleep(Duration::from_millis(180)).await;
    let _ = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn retryable_metadata_failures_use_increasing_bounded_backoff() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let source: MetadataSource = Arc::new(move |_, _, _| {
        let calls = calls.clone();
        Box::pin(async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(DomainError::new(ErrorCode::ProviderUnavailable, "PRIVATE").retryable())
        })
    });
    let s = NativeService::new(Arc::new(|_| Ok(inventory())), Duration::from_secs(60))
        .with_metadata(source);
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let first = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    let first_delay = first.retry_after_ms.unwrap();
    assert!((200..=400).contains(&first_delay));
    tokio::time::sleep(Duration::from_millis(400)).await;
    let second = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let second_delay = second.retry_after_ms.unwrap();
    assert!((400..=800).contains(&second_delay));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = s
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap_err();
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

fn watched_service(
    root: std::path::PathBuf,
    calls: Arc<AtomicUsize>,
    delay: Duration,
) -> Arc<NativeService> {
    let source: MetadataSource = Arc::new(move |_, id, route| {
        let calls = calls.clone();
        Box::pin(async move {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(delay).await;
            Ok(report(&id, &route))
        })
    });
    Arc::new(
        NativeService::new(
            Arc::new(move |_| {
                let mut scan = inventory();
                for (id, location) in &mut scan.locations {
                    location.profile_home = root.join(id);
                }
                Ok(scan)
            }),
            Duration::ZERO,
        )
        .with_metadata(source),
    )
}
#[tokio::test]
async fn explicit_watch_invalidates_changed_metadata_without_running_a_probe() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["personal", "work"] {
        std::fs::create_dir(temp.path().join(name)).unwrap();
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let s = watched_service(temp.path().into(), calls.clone(), Duration::ZERO);
    s.register_client("app", probe_policy()).unwrap();
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
    s.handle("app", "discovery.watch", json!({"enabled":true}))
        .await
        .unwrap();
    std::fs::write(temp.path().join("personal/auth.json"), "secret never read").unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let snapshot = s.handle("app", "snapshot.get", json!({})).await.unwrap();
            let stale = snapshot["connections"]
                .as_array()
                .unwrap()
                .iter()
                .find(|connection| connection["id"] == "personal")
                .is_some_and(|connection| connection["status"] == "metadata_stale");
            if stale {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // Invalidation publishes staleness immediately; it does not invent a
    // logout, and this test did not start the autonomous lane.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.handle("app", "snapshot.get", json!({})).await.unwrap()["models"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn watching_needs_discovery_permission_and_can_be_stopped() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["personal", "work"] {
        std::fs::create_dir(temp.path().join(name)).unwrap();
    }
    let s = watched_service(
        temp.path().into(),
        Arc::new(AtomicUsize::new(0)),
        Duration::ZERO,
    );
    s.register_client("app", probe_policy()).unwrap();
    s.register_client("denied", ClientPolicy::denied()).unwrap();
    assert!(
        s.handle("denied", "discovery.watch", json!({"enabled":true}))
            .await
            .is_err()
    );
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle("app", "discovery.watch", json!({"enabled":true}))
        .await
        .unwrap();
    s.handle("app", "discovery.watch", json!({"enabled":false}))
        .await
        .unwrap();
    s.handle(
        "app",
        "connections.probe",
        json!({"connectionId":"personal"}),
    )
    .await
    .unwrap();
    std::fs::write(
        temp.path().join("personal/auth.json"),
        "not observed after stop",
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !s.handle("app", "snapshot.get", json!({})).await.unwrap()["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn observations_survive_a_restart_as_stale_while_revalidate_data_without_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.db");
    let first = service(Arc::new(AtomicUsize::new(0)), Duration::ZERO);
    first.enable_metadata_cache(path.clone());
    first.register_client("app", probe_policy()).unwrap();
    first
        .handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    first
        .handle(
            "app",
            "connections.probe",
            json!({"connectionId":"personal"}),
        )
        .await
        .unwrap();
    let mut written = std::fs::read(&path).unwrap();
    if let Ok(wal) = std::fs::read(directory.path().join("state.db-wal")) {
        written.extend_from_slice(&wal);
    }
    assert!(
        written
            .windows(b"synthetic-plan".len())
            .any(|window| window == b"synthetic-plan")
    );
    assert!(
        !written
            .windows(b"personal@example.test".len())
            .any(|window| window == b"personal@example.test"),
        "account identity must not be written to disk"
    );
    // A new process: no probe runs, yet the earlier observation is there right
    // after hydration/discovery and is labeled stale rather than current.
    let probes = Arc::new(AtomicUsize::new(0));
    let second = service(probes.clone(), Duration::ZERO);
    second.enable_metadata_cache(path);
    second.register_client("app", probe_policy()).unwrap();
    second
        .handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let snapshot = second
        .handle("app", "snapshot.get", json!({}))
        .await
        .unwrap();
    assert_eq!(probes.load(Ordering::SeqCst), 0);
    assert_eq!(snapshot["models"].as_array().unwrap().len(), 1);
    assert_eq!(
        snapshot["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "personal")
            .unwrap()["status"],
        "metadata_stale"
    );
    assert_eq!(
        snapshot["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "personal")
            .unwrap()["plan"]["value"]["nativeId"],
        "synthetic-plan"
    );
}
#[tokio::test]
async fn a_corrupt_or_foreign_cache_file_is_preserved_and_rebuilt() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.db");
    std::fs::write(&path, b"{ not sqlite").unwrap();
    let s = service(Arc::new(AtomicUsize::new(0)), Duration::ZERO);
    s.enable_metadata_cache(path.clone());
    s.register_client("app", probe_policy()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let snapshot = s.handle("app", "snapshot.get", json!({})).await.unwrap();
    assert_eq!(snapshot["models"].as_array().unwrap().len(), 0);
    assert!(
        directory
            .path()
            .read_dir()
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().contains("corrupt"))
    );
}
