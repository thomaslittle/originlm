//! Synthetic sources, paused monotonic time, real service/cache/journal.
#[allow(dead_code)]
#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_service::{MetadataSource, NativeService};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn scan() -> ScanResult {
    let mut snapshot = common::fixture();
    snapshot.models.clear();
    snapshot.quota_pools.clear();
    snapshot.accounts.clear();
    for c in &mut snapshot.connections {
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
                    provider_id: c.provider_id.clone(),
                    profile_home: std::path::PathBuf::from("synthetic").join(&c.id),
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

fn report(id: &str, route: &str, usage: bool, used: &str) -> ConnectionMetadata {
    let mut s = common::fixture();
    let pools = if usage {
        vec![QuotaPool {
            id: format!("pool-{id}"),
            account_id: None,
            connection_ids: vec![id.into()],
            route_ids: vec![route.into()],
            sharing: "unverified".into(),
            windows: vec![UsageWindow {
                id: "window".into(),
                label: "Synthetic allowance".into(),
                reading: Observation::reported(
                    UsageReading::try_from(UsageInput {
                        unit: "requests".into(),
                        used: used.to_owned().try_into().unwrap(),
                        limit: Some("100".to_owned().try_into().unwrap()),
                    })
                    .unwrap(),
                    EvidenceSource::new(EvidenceKind::Synthetic, "test", "usage"),
                ),
                resets_at: None,
                duration_seconds: None,
            }],
        }]
    } else {
        vec![]
    };
    ConnectionMetadata {
        connection_id: id.into(),
        route_id: route.into(),
        version: Observation::unknown("TEST"),
        auth: Observation::reported(
            AuthState::Authenticated,
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "auth"),
        ),
        plan: Observation::unknown("NOT_REPORTED"),
        billing: Observation::reported(
            BillingMode::Subscription,
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "billing"),
        ),
        account: None,
        models: s.models.drain(..).filter(|m| m.route_id == route).collect(),
        providers: vec![],
        additional_routes: vec![],
        quota_pools: pools,
        included_usage_allowed: Observation::unknown("NOT_REPORTED"),
        diagnostics: vec![],
    }
}

async fn settle() {
    // Keep the test runnable so Tokio never auto-advances to a pending deadline.
    for _ in 0..200 {
        tokio::task::yield_now().await;
    }
}
async fn advance(seconds: u64) {
    for _ in 0..seconds.div_ceil(5) {
        tokio::time::advance(Duration::from_secs(5)).await;
        // Tokio's timer wheel rounds deadlines up to the next millisecond.
        tokio::time::advance(Duration::from_millis(1)).await;
        settle().await;
    }
}
async fn ready(source: MetadataSource) -> Arc<NativeService> {
    let s = Arc::new(
        NativeService::new(Arc::new(|_| Ok(scan())), Duration::ZERO).with_metadata(source),
    );
    s.enable_autonomous_refresh(BTreeSet::from(["test".into()]));
    s.register_client("app", ClientPolicy::passive()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    tokio::time::pause();
    assert!(s.start_background_tasks());
    settle().await;
    advance(5).await;
    s
}
async fn snapshot(s: &NativeService) -> Value {
    s.handle("app", "snapshot.get", json!({})).await.unwrap()
}
async fn wait_snapshot(s: &NativeService, ready: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..10_000 {
        let value = snapshot(s).await;
        if ready(&value) {
            return value;
        }
        tokio::task::yield_now().await;
    }
    panic!("blocking inventory worker did not finish within the bounded runnable budget");
}
fn named<'a>(value: &'a Value, field: &str, id: &str) -> &'a Value {
    value[field]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("missing {field}/{id}: {value}"))
}

#[tokio::test]
async fn supported_usage_refreshes_before_slow_metadata_without_reset_inference() {
    // Removing usage scheduling leaves the old reading visible after 60s.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let seen = seen.clone();
        Box::pin(async move {
            seen.lock().unwrap().push(id.clone());
            let used = seen.lock().unwrap().iter().filter(|c| **c == id).count();
            Ok(report(
                &id,
                &route,
                id == "personal",
                if used == 1 { "80" } else { "81" },
            ))
        })
    }))
    .await;
    let before = snapshot(&s).await;
    assert_eq!(
        named(&before, "quotaPools", "pool-personal")["windows"][0]["reading"]["value"]["used"],
        "80"
    );
    advance(65).await;
    let after = snapshot(&s).await;
    assert_eq!(
        named(&after, "quotaPools", "pool-personal")["windows"][0]["reading"]["value"]["used"],
        "81"
    );
    assert_ne!(before["revision"], after["revision"]);
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|id| id.as_str() == "work")
            .count(),
        1,
        "unknown usage must not create a fast provider polling loop"
    );
    let events = s
        .handle(
            "app",
            "events.read",
            json!({"scope":"inventory", "after":before["cursor"]}),
        )
        .await
        .unwrap();
    assert!(!events["events"].as_array().unwrap().is_empty());
    s.stop_background_tasks();
}

#[tokio::test]
async fn a_slow_provider_does_not_block_a_healthy_connection_or_duplicate_probes() {
    // Serial probing makes the healthy work account wait for personal's timeout.
    let running = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let (r, p, c) = (running.clone(), peak.clone(), calls.clone());
    let s = ready(Arc::new(move |_, id, route| {
        let (r, p, c) = (r.clone(), p.clone(), c.clone());
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            let count = r.fetch_add(1, Ordering::SeqCst) + 1;
            p.fetch_max(count, Ordering::SeqCst);
            struct Guard(Arc<AtomicUsize>);
            impl Drop for Guard {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _guard = Guard(r);
            if id == "personal" {
                tokio::time::sleep(Duration::from_secs(20)).await;
            }
            Ok(report(&id, &route, false, "0"))
        })
    }))
    .await;
    advance(5).await;
    assert_eq!(
        named(&snapshot(&s).await, "connections", "work")["status"],
        "metadata_ready"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(peak.load(Ordering::SeqCst) <= 2);
    advance(20).await;
    assert_eq!(
        named(&snapshot(&s).await, "connections", "personal")["status"],
        "metadata_ready",
        "a peer's commit must not discard this independent report"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn usage_backoff_retains_stale_data_and_does_not_stale_a_healthy_peer() {
    // Ignoring Retry-After storms the source; blanket stale marking corrupts the peer.
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let c = c.clone();
        Box::pin(async move {
            if id == "personal" && c.fetch_add(1, Ordering::SeqCst) > 0 {
                let mut error =
                    DomainError::new(ErrorCode::RateLimited, "private-provider-text").retryable();
                error.retry_after_ms = Some(120_000);
                return Err(error);
            }
            Ok(report(&id, &route, true, "80"))
        })
    }))
    .await;
    advance(65).await;
    let after = snapshot(&s).await;
    let reading = &named(&after, "quotaPools", "pool-personal")["windows"][0]["reading"];
    assert_eq!(reading["value"]["used"], "80");
    assert_eq!(reading["freshness"], "stale");
    assert_eq!(
        named(&after, "connections", "work")["auth"]["freshness"],
        "fresh"
    );
    assert!(!after.to_string().contains("private-provider-text"));
    let after_attempt = calls.load(Ordering::SeqCst);
    assert!(
        after_attempt >= 2,
        "the failing usage probe must be attempted"
    );
    advance(100).await;
    assert!(
        calls.load(Ordering::SeqCst) <= after_attempt + 1,
        "Retry-After must still bound retries after the faster usage cadence"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn idle_usage_refreshes_inside_the_freshness_ttl_and_stop_cancels_owned_probes() {
    // Idle monitors must not wait for the old 300s cadence while readings expire
    // after 60s; stopping still cancels owned probes.
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(report(&id, &route, true, "80"))
        })
    }))
    .await;
    s.close_client("app");
    advance(65).await;
    let first = calls.load(Ordering::SeqCst);
    assert!(
        first >= 4,
        "idle usage must refresh before the 60s freshness threshold"
    );
    advance(240).await;
    assert!(
        calls.load(Ordering::SeqCst) > first,
        "idle clients still get bounded reconciliation"
    );
    s.stop_background_tasks();
    let stopped = calls.load(Ordering::SeqCst);
    advance(600).await;
    assert_eq!(calls.load(Ordering::SeqCst), stopped);
}

#[tokio::test]
async fn idle_usage_keeps_advancing_across_multiple_freshness_intervals() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let seen = seen.clone();
        Box::pin(async move {
            let n = {
                let mut calls = seen.lock().unwrap();
                calls.push(id.clone());
                calls.iter().filter(|call| **call == id).count()
            };
            Ok(report(&id, &route, true, &(79 + n).to_string()))
        })
    }))
    .await;
    s.close_client("app");
    advance(65).await;
    advance(65).await;
    advance(65).await;
    s.register_client("observer", ClientPolicy::passive())
        .unwrap();
    let after = s
        .handle("observer", "snapshot.get", json!({}))
        .await
        .unwrap();
    let reading = &named(&after, "quotaPools", "pool-personal")["windows"][0]["reading"];
    assert_eq!(
        reading["value"]["used"], "84",
        "three idle intervals should publish three newer observations"
    );
    assert_eq!(
        reading["freshness"], "fresh",
        "idle usage must not expire between polls"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn idle_usage_expiry_publishes_a_stale_event_without_inventing_zero() {
    // A refresh that has not returned yet must still expose the old reading as
    // stale rather than extending its freshness or inventing zero.
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let c = c.clone();
        Box::pin(async move {
            if c.fetch_add(1, Ordering::SeqCst) > 0 {
                tokio::time::sleep(Duration::from_secs(120)).await;
            }
            Ok(report(&id, &route, true, "80"))
        })
    }))
    .await;
    let before = snapshot(&s).await;
    s.close_client("app");
    advance(65).await;
    s.register_client("observer", ClientPolicy::passive())
        .unwrap();
    let after = s
        .handle("observer", "snapshot.get", json!({}))
        .await
        .unwrap();
    assert_eq!(
        named(&after, "quotaPools", "pool-personal")["windows"][0]["reading"]["value"]["used"],
        "80"
    );
    assert_eq!(
        named(&after, "quotaPools", "pool-personal")["windows"][0]["reading"]["freshness"],
        "stale"
    );
    assert_eq!(
        named(&after, "connections", "personal")["auth"]["freshness"],
        "fresh"
    );
    let events = s
        .handle(
            "observer",
            "events.read",
            json!({"scope":"inventory", "after":before["cursor"]}),
        )
        .await
        .unwrap();
    assert!(
        !events["events"].as_array().unwrap().is_empty(),
        "expiry must commit a journal change"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn metadata_uses_slow_cadence_and_interest_expires_on_an_idle_open_socket() {
    // Neither fixed 5s probing nor treating every registered socket as active is valid.
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(report(&id, &route, false, "0"))
        })
    }))
    .await;
    advance(910).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "an idle socket must use the reduced cadence"
    );
    snapshot(&s).await; // Real renewed read interest, no refresh/probe request.
    advance(5).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "active interest must recheck expired metadata"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn a_provider_reset_deadline_only_requests_an_authoritative_observation() {
    // Inferring a reset from the timestamp would incorrectly change 80 to zero.
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let reset =
        Timestamp::try_from((chrono::Utc::now() + chrono::Duration::seconds(15)).to_rfc3339())
            .unwrap();
    let s = ready(Arc::new(move |_, id, route| {
        let (c, reset) = (c.clone(), reset.clone());
        Box::pin(async move {
            let n = c.fetch_add(1, Ordering::SeqCst);
            let mut value = report(&id, &route, true, "80");
            if n < 2 {
                value.quota_pools[0].windows[0].resets_at = Some(reset);
            }
            Ok(value)
        })
    }))
    .await;
    advance(20).await;
    assert!(
        calls.load(Ordering::SeqCst) > 2,
        "a provider-reported reset should prompt a re-read before ordinary usage expiry"
    );
    let after = snapshot(&s).await;
    assert_eq!(
        named(&after, "quotaPools", "pool-personal")["windows"][0]["reading"]["value"]["used"],
        "80"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn inventory_reconciles_without_metadata_and_keeps_application_scope() {
    // Provider metadata permission must neither authorize nor be required for a passive scan.
    let generation = Arc::new(AtomicUsize::new(0));
    let count = Arc::new(AtomicUsize::new(0));
    let (g, c) = (generation.clone(), count.clone());
    let s = Arc::new(NativeService::new(
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            let mut value = scan();
            value.snapshot.connections[0].display_name =
                format!("generation-{}", g.load(Ordering::SeqCst));
            Ok(value)
        }),
        Duration::ZERO,
    ));
    assert!(!s.start_background_tasks());
    let mut policy = ClientPolicy::passive();
    policy.connection_ids = Some(BTreeSet::from(["personal".into()]));
    s.register_client("app", policy).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    tokio::time::pause();
    s.enable_inventory_refresh(true);
    // The inventory lane paces itself on wall-clock time (300s while a client is
    // interested), which a paused-clock test cannot advance. Change the reported
    // generation before the supervisor starts, so the first background scan must
    // observe the new state; then prove reads alone never add a scan.
    generation.store(1, Ordering::SeqCst);
    assert!(s.start_background_tasks());
    assert!(
        !s.start_background_tasks(),
        "a second supervisor would duplicate work"
    );
    settle().await;
    advance(5).await;
    let mut after = snapshot(&s).await;
    for _ in 0..50 {
        if named(&after, "connections", "personal")["displayName"] == "generation-1" {
            break;
        }
        tokio::task::yield_now().await;
        after = snapshot(&s).await;
    }
    // Read-only interest keeps the active cadence; reads themselves never scan.
    for _ in 0..6 {
        snapshot(&s).await;
        advance(1).await;
    }
    assert_eq!(after["connections"].as_array().unwrap().len(), 1);
    assert_eq!(
        named(&after, "connections", "personal")["displayName"],
        "generation-1"
    );
    assert!(
        after["models"].as_array().unwrap().is_empty(),
        "inventory does not invent provider metadata"
    );
    assert!(
        count.load(Ordering::SeqCst) <= 3,
        "UI reads must not create full rescans"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn failed_inventory_retains_stale_observations_and_a_typed_refresh_error() {
    let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = failed.clone();
    let s = Arc::new(NativeService::new(
        Arc::new(move |_| {
            if flag.load(Ordering::SeqCst) {
                Err(DomainError::new(ErrorCode::ProviderUnavailable, "secret-path").retryable())
            } else {
                Ok(scan())
            }
        }),
        Duration::ZERO,
    ));
    s.register_client("app", ClientPolicy::passive()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    failed.store(true, Ordering::SeqCst);
    s.enable_inventory_refresh(true);
    tokio::time::pause();
    assert!(s.start_background_tasks());
    settle().await;
    advance(5).await;
    let after = wait_snapshot(&s, |v| {
        v["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "adapter_failure")
    })
    .await;
    assert_eq!(
        named(&after, "routes", "route-personal")["billing"]["freshness"],
        "stale"
    );
    assert!(!after.to_string().contains("secret-path"));
    assert!(
        after["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "adapter_failure")
    );
    failed.store(false, Ordering::SeqCst);
    advance(5).await;
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    let recovered = snapshot(&s).await;
    assert!(
        recovered["issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "adapter_failure" || i["state"] == "resolved")
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn stopping_background_work_drops_owned_probes_and_does_not_publish_late_data() {
    let running = Arc::new(AtomicUsize::new(0));
    let r = running.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let r = r.clone();
        Box::pin(async move {
            struct Guard(Arc<AtomicUsize>);
            impl Drop for Guard {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            r.fetch_add(1, Ordering::SeqCst);
            let _guard = Guard(r);
            tokio::time::sleep(Duration::from_secs(20)).await;
            Ok(report(&id, &route, false, "0"))
        })
    }))
    .await;
    assert_eq!(running.load(Ordering::SeqCst), 2);
    s.stop_background_tasks();
    settle().await;
    assert_eq!(running.load(Ordering::SeqCst), 0);
    advance(60).await;
    assert!(snapshot(&s).await["models"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn withdrawing_host_probe_permission_discards_in_flight_reports() {
    let s = ready(Arc::new(|_, id, route| {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(15)).await;
            Ok(report(&id, &route, true, "80"))
        })
    }))
    .await;
    s.enable_autonomous_refresh(BTreeSet::new());
    advance(20).await;
    assert!(snapshot(&s).await["models"].as_array().unwrap().is_empty());
    s.stop_background_tasks();
}

#[tokio::test]
async fn inventory_retry_after_is_shared_by_background_and_explicit_refreshes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = Arc::new(NativeService::new(
        Arc::new(move |_| {
            if c.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(scan());
            }
            // Make blocking-thread delivery independent of the runnable test.
            std::thread::sleep(Duration::from_millis(50));
            let mut error = DomainError::new(ErrorCode::RateLimited, "private-path").retryable();
            error.retry_after_ms = Some(3_600_000);
            Err(error)
        }),
        Duration::ZERO,
    ));
    s.register_client("app", ClientPolicy::passive()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.enable_inventory_refresh(true);
    tokio::time::pause();
    assert!(s.start_background_tasks());
    settle().await;
    // Hold virtual time until the provider's failure is actually published.
    // Advancing first can expire the five-second worker deadline while the
    // blocking thread is still delivering RateLimited, testing timeout backoff
    // instead of the authoritative Retry-After this fixture intends to cover.
    // Release Tokio's rounded initial interval tick without expiring the scan.
    tokio::time::advance(Duration::from_millis(1)).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let observed = snapshot(&s).await;
        if observed["issues"].as_array().unwrap().iter().any(|issue| {
            issue["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|evidence| evidence["code"] == "RateLimited")
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the authoritative inventory failure was not published: {observed}"
        );
        tokio::task::yield_now().await;
    }
    advance(5).await;
    for _ in 0..7 {
        snapshot(&s).await;
        advance(55).await;
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "background inventory must respect authoritative Retry-After"
    );
    assert_eq!(
        s.handle("app", "discovery.refresh", json!({}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::RateLimited
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "an explicit request must not bypass backoff"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn cancelling_a_background_scan_signals_only_its_owned_source() {
    let calls = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (c, stop) = (calls.clone(), stopped.clone());
    let s = Arc::new(NativeService::new(
        Arc::new(move |cancel| {
            if c.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(scan());
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !cancel.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            stop.store(cancel.load(Ordering::SeqCst), Ordering::SeqCst);
            let mut late = scan();
            late.snapshot.connections[0].display_name = "late-must-not-publish".into();
            Ok(late)
        }),
        Duration::ZERO,
    ));
    s.register_client("app", ClientPolicy::passive()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.enable_inventory_refresh(true);
    tokio::time::pause();
    assert!(s.start_background_tasks());
    settle().await;
    tokio::time::advance(Duration::from_millis(1)).await;
    for _ in 0..1000 {
        settle().await;
        if calls.load(Ordering::SeqCst) == 2 {
            break;
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    s.stop_background_tasks();
    for _ in 0..1000 {
        settle().await;
        if stopped.load(Ordering::SeqCst) {
            break;
        }
    }
    assert!(
        stopped.load(Ordering::SeqCst),
        "owned blocking discovery must receive cooperative cancellation"
    );
    assert_ne!(
        named(&snapshot(&s).await, "connections", "personal")["displayName"],
        "late-must-not-publish"
    );
}

#[tokio::test]
async fn scoped_inventory_events_never_reveal_another_accounts_resource_ids() {
    let s = ready(Arc::new(|_, id, route| {
        Box::pin(async move { Ok(report(&id, &route, true, "80")) })
    }))
    .await;
    let mut policy = ClientPolicy::passive();
    policy.connection_ids = Some(BTreeSet::from(["personal".into()]));
    s.register_client("limited", policy).unwrap();
    let first = s
        .handle("limited", "snapshot.get", json!({}))
        .await
        .unwrap();
    advance(65).await;
    let page = s
        .handle(
            "limited",
            "events.read",
            json!({"scope":"inventory", "after":first["cursor"]}),
        )
        .await
        .unwrap();
    assert!(
        !page.to_string().contains("work"),
        "scoped journal payload leaked a peer's resource IDs: {page}"
    );
    assert!(!page["events"].as_array().unwrap().is_empty());
    s.stop_background_tasks();
}

#[tokio::test]
async fn timer_expiry_preserves_quota_warning_until_authoritative_recovery() {
    let s = ready(Arc::new(|_, id, route| {
        Box::pin(async move { Ok(report(&id, &route, true, "99")) })
    }))
    .await;
    assert!(
        snapshot(&s).await["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "quota_exhausted")
    );
    s.close_client("app");
    advance(65).await;
    s.register_client("app", ClientPolicy::passive()).unwrap();
    let after = snapshot(&s).await;
    assert!(
        after["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "quota_exhausted" && i["state"] != "resolved"),
        "expiry is not proof of quota recovery"
    );
    assert_eq!(after["health"]["status"], "degraded");
    s.stop_background_tasks();
}

#[tokio::test]
async fn healthy_work_beyond_sixty_four_failed_connections_and_usage_both_progress() {
    let mut value = scan();
    let base_c = value.snapshot.connections[0].clone();
    let base_r = value.snapshot.routes[0].clone();
    value.snapshot.connections.clear();
    value.snapshot.routes.clear();
    value.locations.clear();
    for id in (0..68)
        .map(|n| format!("prefix-{n:02}"))
        .chain(["z-healthy".into(), "z-usage".into()])
    {
        let mut c = base_c.clone();
        c.id = id.clone();
        let mut r = base_r.clone();
        r.id = format!("route-{id}");
        r.connection_id = id.clone();
        value.snapshot.connections.push(c);
        value.snapshot.routes.push(r);
        value.locations.insert(
            id.clone(),
            LocatedClient {
                executable_identity: None,
                provider_id: "test".into(),
                profile_home: std::path::PathBuf::from(id),
                executable: None,
                scope: None,
            },
        );
    }
    let scan_value = Arc::new(Mutex::new(value));
    let usage_calls = Arc::new(AtomicUsize::new(0));
    let calls = usage_calls.clone();
    let source: MetadataSource = Arc::new(move |_, id, route| {
        let calls = calls.clone();
        Box::pin(async move {
            if id.starts_with("prefix") {
                let mut e = DomainError::new(ErrorCode::RateLimited, "synthetic").retryable();
                e.retry_after_ms = Some(0);
                return Err(e);
            }
            let n = if id == "z-usage" {
                calls.fetch_add(1, Ordering::SeqCst)
            } else {
                0
            };
            Ok(report(
                &id,
                &route,
                id == "z-usage",
                if n == 0 { "80" } else { "81" },
            ))
        })
    });
    let s = Arc::new(
        NativeService::new(
            Arc::new(move |_| {
                let v = scan_value.lock().unwrap();
                Ok(ScanResult {
                    snapshot: v.snapshot.clone(),
                    locations: v.locations.clone(),
                    metadata_probes: 0,
                })
            }),
            Duration::ZERO,
        )
        .with_metadata(source),
    );
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids = BTreeSet::from(["test".into()]);
    s.register_client("app", policy).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    s.handle(
        "app",
        "connections.probe",
        json!({"connectionId":"z-usage"}),
    )
    .await
    .unwrap();
    s.enable_autonomous_refresh(BTreeSet::from(["test".into()]));
    tokio::time::pause();
    assert!(s.start_background_tasks());
    settle().await;
    advance(85).await;
    let early = snapshot(&s).await;
    assert_eq!(
        named(&early, "quotaPools", "pool-z-usage")["windows"][0]["reading"]["value"]["used"],
        "81",
        "metadata backlog must not starve due usage"
    );
    advance(160).await;
    assert_eq!(
        named(&snapshot(&s).await, "connections", "z-healthy")["status"],
        "metadata_ready",
        "a continuously failing prefix must not starve a healthy account beyond the page bound"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn sleep_resume_coalesces_missed_intervals_instead_of_replaying_poll_ticks() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let c = c.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(report(&id, &route, true, "80"))
        })
    }))
    .await;
    tokio::time::advance(Duration::from_secs(7200)).await;
    tokio::time::advance(Duration::from_millis(1)).await;
    settle().await;
    advance(5).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "resume should coalesce a two-hour gap into one refresh per scope"
    );
    assert_eq!(
        named(&snapshot(&s).await, "quotaPools", "pool-personal")["windows"][0]["reading"]["value"]
            ["used"],
        "80"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn duplicate_routes_in_one_verified_quota_scope_share_one_probe_per_tick() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let s = ready(Arc::new(move |_, id, route| {
        let seen = seen.clone();
        Box::pin(async move {
            seen.lock().unwrap().push(id.clone());
            let mut value = report(&id, &route, true, "80");
            let extra = stable_id("route", &[&route, "test"]);
            value.additional_routes.push(AccessRoute {
                id: extra.clone(),
                connection_id: id.clone(),
                provider_id: "test".into(),
                display_name: "Second verified route".into(),
                billing: value.billing.clone(),
                capabilities: vec![],
                catalog_revision: "1".into(),
            });
            value.quota_pools[0].route_ids.push(extra);
            value.quota_pools[0].sharing = "verified".into();
            let mut account = common::fixture()
                .accounts
                .into_iter()
                .find(|a| a.id == format!("account-{id}"))
                .unwrap();
            account.display_label = Observation::reported(
                "same@example.test".into(),
                EvidenceSource::new(EvidenceKind::Synthetic, "test", "display only"),
            );
            value.quota_pools[0].account_id = Some(account.id.clone());
            value.account = Some(account);
            Ok(value)
        })
    }))
    .await;
    advance(65).await;
    let view = snapshot(&s).await;
    assert_eq!(
        named(&view, "quotaPools", "pool-personal")["routeIds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|id| id.as_str() == "personal")
            .count(),
        2,
        "two verified routes for one account must share the initial and usage probes"
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|id| id.as_str() == "work")
            .count(),
        2,
        "similarly named windows in a distinct unverified account must remain independent"
    );
    s.stop_background_tasks();
}

#[tokio::test]
async fn explicit_probe_failure_from_before_a_scan_cannot_stale_the_new_generation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let source: MetadataSource = Arc::new(move |_, id, route| {
        let c = c.clone();
        Box::pin(async move {
            if c.fetch_add(1, Ordering::SeqCst) == 0 {
                return Ok(report(&id, &route, true, "80"));
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
            Err(DomainError::new(ErrorCode::Timeout, "private-path").retryable())
        })
    });
    let s = Arc::new(
        NativeService::new(Arc::new(|_| Ok(scan())), Duration::ZERO).with_metadata(source),
    );
    let mut p = ClientPolicy::passive();
    p.probe_provider_ids = BTreeSet::from(["test".into()]);
    s.register_client("app", p).unwrap();
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
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(61)).await;
    let worker = s.clone();
    let old = tokio::spawn(async move {
        worker
            .handle(
                "app",
                "connections.probe",
                json!({"connectionId":"personal"}),
            )
            .await
    });
    settle().await;
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    advance(20).await;
    assert_eq!(
        old.await.unwrap().unwrap_err().code,
        ErrorCode::SelectionStale
    );
    let after = snapshot(&s).await;
    assert_eq!(
        named(&after, "connections", "personal")["auth"]["freshness"],
        "fresh"
    );
    assert!(
        after["issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "adapter_failure")
    );
}

#[tokio::test]
async fn stopping_background_inventory_does_not_cancel_a_foreground_joiners_shared_scan() {
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (c, r, flag) = (calls.clone(), release.clone(), cancelled.clone());
    let s = Arc::new(NativeService::new(
        Arc::new(move |cancel| {
            let n = c.fetch_add(1, Ordering::SeqCst);
            if n == 1 {
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while !r.load(Ordering::SeqCst)
                    && !cancel.load(Ordering::SeqCst)
                    && std::time::Instant::now() < deadline
                {
                    std::thread::yield_now();
                }
                flag.store(cancel.load(Ordering::SeqCst), Ordering::SeqCst);
            }
            let mut v = scan();
            v.snapshot.connections[0].display_name = format!("scan-{n}");
            Ok(v)
        }),
        Duration::ZERO,
    ));
    s.register_client("app", ClientPolicy::passive()).unwrap();
    s.handle("app", "discovery.refresh", json!({}))
        .await
        .unwrap();
    tokio::time::pause();
    s.enable_inventory_refresh(true);
    assert!(s.start_background_tasks());
    settle().await;
    tokio::time::advance(Duration::from_millis(1)).await;
    for _ in 0..1000 {
        settle().await;
        if calls.load(Ordering::SeqCst) == 2 {
            break;
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let worker = s.clone();
    let foreground =
        tokio::spawn(async move { worker.handle("app", "discovery.refresh", json!({})).await });
    settle().await;
    s.stop_background_tasks();
    release.store(true, Ordering::SeqCst);
    tokio::time::resume();
    foreground.await.unwrap().unwrap();
    assert!(!cancelled.load(Ordering::SeqCst));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        named(&snapshot(&s).await, "connections", "personal")["displayName"],
        "scan-1"
    );
}
