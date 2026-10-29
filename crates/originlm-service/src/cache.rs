use crate::InventorySource;
use crate::health::{derive_issues, merge_issue, storage_issue, summarize};
use originlm_contracts::{
    ConnectionMetadata, DataMode, DomainError, ErrorCode, InventoryChange, InventoryEventPage,
    Issue, IssueState, NotificationPreferences, ResourceChange, ResourceChangeKind, Result,
    Snapshot, Timestamp,
};
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_store::{DurableMetadata, StateStore, StoreError};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Notify, watch};
use tokio::time::Instant;

type RefreshResult = Option<Result<()>>;
const METADATA_PROBE_CACHE_TTL: Duration = Duration::from_secs(60);
const METADATA_FRESHNESS_TTL: Duration = Duration::from_secs(15 * 60);
/// Authoritative usage/reset observations are polled on their own, faster
/// cadence. This never advances a reset; it only re-reads provider evidence.
pub(crate) const USAGE_FRESHNESS_TTL: Duration = Duration::from_secs(60);
const JOURNAL_LIMIT: usize = 2048;
const JOURNAL_PAGE: usize = 256;

#[derive(Clone)]
struct MetadataEntry {
    observed: Instant,
    /// Last authoritative metadata probe that may contain usage/reset data.
    /// Kept separately so usage can be refreshed at its own cadence.
    usage_observed: Instant,
    next_reset_check: Option<Instant>,
    report: ConnectionMetadata,
    /// A local filesystem event says this must be re-read. The old value stays
    /// visible as stale until a probe succeeds.
    invalidated: bool,
}

#[derive(Clone)]
struct MetadataFailure {
    until: Instant,
    attempts: u32,
    error: DomainError,
}

#[derive(Clone)]
struct CacheState {
    snapshot: Snapshot,
    metadata: BTreeMap<String, MetadataEntry>,
    metadata_failures: BTreeMap<String, MetadataFailure>,
    issues: BTreeMap<String, Issue>,
    revision: u64,
    epoch: String,
    cursor: String,
    journal: VecDeque<InventoryChange>,
    store: Option<Arc<StateStore>>,
    storage_error: Option<DomainError>,
    notification_preferences: NotificationPreferences,
    notification_history: BTreeMap<String, u64>,
    // Native-only: never serialize filesystem locations into public responses.
    _locations: BTreeMap<String, LocatedClient>,
    updated: Option<Instant>,
    failure: Option<(Instant, DomainError)>,
    scan_attempts: u32,
    flight: Option<watch::Receiver<RefreshResult>>,
    flight_cancel: Option<Arc<AtomicBool>>,
    flight_cancel_wake: Option<Arc<Notify>>,
    flight_background: bool,
    flight_generation: Option<u64>,
    flight_host_cancelled: bool,
    background_generation: u64,
    inventory_enabled: bool,
    scan_generation: u64,
    connection_generations: BTreeMap<String, u64>,
    dirty: BTreeSet<String>,
    hydrated: bool,
}

pub(crate) struct ProbeTarget {
    pub connection_id: String,
    pub located: LocatedClient,
    pub route_id: String,
    pub base: ProbeBase,
}

pub(crate) struct ProbeBase {
    scan: u64,
    connection: u64,
    background: Option<u64>,
}

impl ProbeTarget {
    pub(crate) fn for_background(mut self, generation: u64) -> Self {
        self.base.background = Some(generation);
        self
    }
}

impl MetadataEntry {
    fn merge(&self, snapshot: &mut Snapshot) {
        crate::metadata::merge_report(
            snapshot,
            &self.report,
            self.invalidated || metadata_is_stale(self.observed.elapsed()),
        );
        if self.usage_observed.elapsed() >= USAGE_FRESHNESS_TTL
            || self.next_reset_check.is_some_and(|at| at <= Instant::now())
        {
            for pool in snapshot
                .quota_pools
                .iter_mut()
                .filter(|p| p.connection_ids.contains(&self.report.connection_id))
            {
                for window in &mut pool.windows {
                    window.reading.mark_stale();
                }
            }
        }
    }
}

fn reset_check(report: &ConnectionMetadata, now: chrono::DateTime<chrono::Utc>) -> Option<Instant> {
    // Convert a future provider timestamp once to a monotonic deadline. A
    // response still reporting a past reset does not cause a hot polling loop.
    report
        .quota_pools
        .iter()
        .flat_map(|p| &p.windows)
        .filter_map(|w| {
            let at = chrono::DateTime::parse_from_rfc3339(w.resets_at.as_ref()?.as_str()).ok()?;
            let delay = (at.with_timezone(&chrono::Utc) - now).to_std().ok()?;
            Some(Instant::now() + delay.max(Duration::from_secs(5)))
        })
        .min()
}

pub(crate) struct InventoryCache {
    state: Mutex<CacheState>,
    source: InventorySource,
    ttl: Duration,
    wake: Notify,
    wall_clock: Arc<dyn Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync>,
}

fn unix_ms(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn instance_epoch() -> String {
    let mut bytes = [0u8; 8];
    let _ = getrandom::fill(&mut bytes);
    format!("local-{:x}", u64::from_be_bytes(bytes))
}

fn metadata_is_stale(age: Duration) -> bool {
    age >= METADATA_FRESHNESS_TTL
}

fn unavailable() -> DomainError {
    DomainError::new(
        ErrorCode::ProviderUnavailable,
        "Native inventory is unavailable",
    )
}

fn store_error(error: StoreError) -> DomainError {
    error.domain()
}

fn storage_failure(error: DomainError) -> DomainError {
    let mut safe = DomainError::new(
        ErrorCode::StorageUnavailable,
        "Local state could not be committed; the previous snapshot remains visible",
    );
    safe.retryable = true;
    safe.retry_after_ms = error.retry_after_ms;
    safe
}

fn metadata_backoff(connection: &str, attempts: u32, error: &DomainError) -> Duration {
    if let Some(milliseconds) = error.retry_after_ms {
        return Duration::from_millis(milliseconds.min(86_400_000));
    }
    if !error.retryable {
        return Duration::from_secs(5);
    }
    let shift = attempts.saturating_sub(1).min(7);
    let base_ms = 250u64.saturating_mul(1u64 << shift).min(30_000);
    let mut hash = 0xcbf29ce484222325u64;
    for byte in connection.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= u64::from(attempts).wrapping_mul(0x9e3779b97f4a7c15);
    let jitter_window = (base_ms / 4).max(1);
    Duration::from_millis(base_ms.saturating_add(hash % jitter_window).min(30_000))
}

fn mark_observation_stale<T>(observation: &mut originlm_contracts::Observation<T>) {
    observation.mark_stale();
}

fn mark_snapshot_stale(snapshot: &mut Snapshot) {
    for connection in &mut snapshot.connections {
        mark_observation_stale(&mut connection.auth);
        mark_observation_stale(&mut connection.plan);
        if matches!(
            connection.status.as_str(),
            "metadata_ready" | "metadata_cached"
        ) {
            connection.status = "metadata_stale".into();
        }
    }
    for installation in &mut snapshot.installations {
        mark_observation_stale(&mut installation.version);
    }
    for route in &mut snapshot.routes {
        mark_observation_stale(&mut route.billing);
        for capability in &mut route.capabilities {
            mark_observation_stale(&mut capability.evidence);
        }
    }
    for model in &mut snapshot.models {
        for capability in &mut model.capabilities {
            mark_observation_stale(&mut capability.evidence);
        }
        mark_observation_stale(&mut model.context_window);
    }
    for pool in &mut snapshot.quota_pools {
        for window in &mut pool.windows {
            mark_observation_stale(&mut window.reading);
        }
    }
    snapshot.health.hydrated = true;
}

fn json_eq<T: serde::Serialize>(left: &T, right: &T) -> bool {
    serde_json::to_string(left).ok() == serde_json::to_string(right).ok()
}

fn push_changes<T: serde::Serialize>(
    changes: &mut Vec<ResourceChange>,
    resource: &str,
    before: &[T],
    after: &[T],
    id: impl Fn(&T) -> String,
) {
    let before: BTreeMap<String, &T> = before.iter().map(|item| (id(item), item)).collect();
    let after: BTreeMap<String, &T> = after.iter().map(|item| (id(item), item)).collect();
    for (id, item) in &after {
        match before.get(id) {
            None => changes.push(ResourceChange {
                resource: resource.into(),
                id: id.clone(),
                change: ResourceChangeKind::Added,
                reason: None,
            }),
            Some(previous) if !json_eq(previous, item) => changes.push(ResourceChange {
                resource: resource.into(),
                id: id.clone(),
                change: ResourceChangeKind::Updated,
                reason: None,
            }),
            Some(_) => {}
        }
    }
    for id in before.keys() {
        if !after.contains_key(id) {
            changes.push(ResourceChange {
                resource: resource.into(),
                id: id.clone(),
                change: ResourceChangeKind::Removed,
                reason: None,
            });
        }
    }
}

fn diff_snapshot(before: &Snapshot, after: &Snapshot) -> Vec<ResourceChange> {
    let mut changes = Vec::new();
    push_changes(
        &mut changes,
        "provider",
        &before.providers,
        &after.providers,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "installation",
        &before.installations,
        &after.installations,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "profile",
        &before.profiles,
        &after.profiles,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "account",
        &before.accounts,
        &after.accounts,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "connection",
        &before.connections,
        &after.connections,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "route",
        &before.routes,
        &after.routes,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "model",
        &before.models,
        &after.models,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "quota_pool",
        &before.quota_pools,
        &after.quota_pools,
        |item| item.id.clone(),
    );
    push_changes(
        &mut changes,
        "issue",
        &before.issues,
        &after.issues,
        |item| item.id.clone(),
    );
    if changes.len() > 64 {
        changes.truncate(63);
        changes.push(ResourceChange {
            resource: "inventory".into(),
            id: "*".into(),
            change: ResourceChangeKind::Updated,
            reason: Some("additional changes omitted from the journal record".into()),
        });
    }
    changes
}

fn cursor_epoch(cursor: &str) -> Option<(&str, u64)> {
    let (epoch, sequence) = cursor.split_once(':')?;
    Some((epoch, sequence.parse().ok()?))
}

impl InventoryCache {
    pub fn new(source: InventorySource, ttl: Duration) -> Self {
        Self {
            source,
            ttl,
            wake: Notify::new(),
            wall_clock: Arc::new(chrono::Utc::now),
            state: Mutex::new(CacheState {
                snapshot: Snapshot::empty(DataMode::Live),
                metadata: BTreeMap::new(),
                metadata_failures: BTreeMap::new(),
                issues: BTreeMap::new(),
                revision: 0,
                epoch: instance_epoch(),
                cursor: String::new(),
                journal: VecDeque::new(),
                store: None,
                storage_error: None,
                notification_preferences: NotificationPreferences::default(),
                notification_history: BTreeMap::new(),
                _locations: BTreeMap::new(),
                updated: None,
                failure: None,
                scan_attempts: 0,
                flight: None,
                flight_cancel: None,
                flight_cancel_wake: None,
                flight_background: false,
                flight_generation: None,
                flight_host_cancelled: false,
                background_generation: 0,
                inventory_enabled: false,
                scan_generation: 0,
                connection_generations: BTreeMap::new(),
                dirty: BTreeSet::new(),
                hydrated: false,
            }),
        }
    }

    /// Open the broker-owned SQLite store and hydrate the ready-to-serve
    /// snapshot before the first request. Hydrated fields are labeled stale and
    /// the background lane reverifies them.
    pub fn enable_store(&self, path: PathBuf) -> Result<()> {
        let (store, notice) = StateStore::open(path).map_err(store_error)?;
        let loaded = store.load().map_err(store_error)?;
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let had_snapshot = loaded.snapshot.is_some();
        if let Some(mut snapshot) = loaded.snapshot {
            mark_snapshot_stale(&mut snapshot);
            snapshot.issues = loaded.issues.clone();
            snapshot.health = summarize(&snapshot.issues, true);
            snapshot.revision = loaded.revision.to_string();
            snapshot.cursor = loaded.cursor.clone();
            state.snapshot = snapshot;
            state.hydrated = true;
        }
        let now = unix_ms(SystemTime::now());
        for entry in loaded.metadata {
            let age = Duration::from_millis(now.saturating_sub(entry.observed_at_ms));
            let observed = Instant::now().checked_sub(age).unwrap_or_else(Instant::now);
            // Hydrated auth is never treated as current without a fresh check.
            let report = entry.report;
            state.metadata.insert(
                report.connection_id.clone(),
                MetadataEntry {
                    observed,
                    usage_observed: observed,
                    next_reset_check: reset_check(&report, (self.wall_clock)()),
                    report,
                    invalidated: true,
                },
            );
        }
        for issue in loaded.issues {
            state.issues.insert(issue.id.clone(), issue);
        }
        if !had_snapshot && !state.issues.is_empty() {
            state.snapshot.issues = state.issues.values().cloned().collect();
            state.snapshot.health = summarize(&state.snapshot.issues, true);
        }
        state.notification_preferences = loaded.notifications;
        state.revision = loaded.revision;
        state.cursor = loaded.cursor;
        state.store = Some(Arc::new(store));
        if notice.is_some() {
            state.storage_error = Some(DomainError::new(
                ErrorCode::StorageUnavailable,
                "Damaged local state was preserved; the runtime rebuilt its working state",
            ));
        }
        state.updated = None;
        let had_issues = !state.issues.is_empty();
        drop(state);
        if had_snapshot || had_issues {
            // The durable view is already ready to serve. Do not invent a new
            // revision merely because the process restarted.
            return Ok(());
        }
        self.compose_and_commit("hydrate", None, false)?;
        Ok(())
    }

    pub(crate) fn status(&self) -> (u64, String, bool, Option<DomainError>) {
        self.state
            .lock()
            .map(|state| {
                (
                    state.revision,
                    state.cursor.clone(),
                    state.hydrated,
                    state.storage_error.clone(),
                )
            })
            .unwrap_or((0, String::new(), false, Some(unavailable())))
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        let mut snapshot = state.snapshot.clone();
        for entry in state.metadata.values() {
            entry.merge(&mut snapshot);
        }
        Ok(snapshot)
    }

    pub(crate) fn inventory_page(&self, after: &str) -> Result<InventoryEventPage> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        if let Some(store) = &state.store {
            return store.read_journal(after).map_err(store_error);
        }
        let Some((epoch, after_sequence)) = cursor_epoch(after) else {
            return Err(DomainError::new(
                ErrorCode::EventGap,
                "Invalid inventory cursor",
            ));
        };
        if epoch != state.epoch {
            return Err(DomainError::new(
                ErrorCode::EventGap,
                "Inventory cursor belongs to a previous runtime epoch",
            ));
        }
        let tail = state
            .journal
            .back()
            .map(|event| event.sequence.parse::<u64>().unwrap_or(0))
            .unwrap_or(0);
        if after_sequence > tail {
            return Err(DomainError::new(
                ErrorCode::EventGap,
                "Inventory cursor is ahead of the journal",
            ));
        }
        if let Some(oldest) = state.journal.front() {
            let oldest = oldest.sequence.parse::<u64>().unwrap_or(0);
            if after_sequence.saturating_add(1) < oldest {
                return Err(DomainError::new(
                    ErrorCode::EventGap,
                    "Inventory cursor is no longer replayable",
                ));
            }
        }
        let events: Vec<_> = state
            .journal
            .iter()
            .filter(|event| event.sequence.parse::<u64>().unwrap_or(0) > after_sequence)
            .take(JOURNAL_PAGE)
            .cloned()
            .collect();
        let cursor = events
            .last()
            .map(|event| event.sequence.clone())
            .unwrap_or_else(|| after_sequence.to_string());
        Ok(InventoryEventPage {
            events,
            cursor: format!("{}:{cursor}", state.epoch),
            tail_cursor: format!("{}:{tail}", state.epoch),
            has_more: cursor.parse::<u64>().unwrap_or(0) < tail,
            revision: state.revision.to_string(),
        })
    }

    pub(crate) fn probe_input(&self, connection: &str) -> Result<ProbeTarget> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        let located =
            state._locations.get(connection).cloned().ok_or_else(|| {
                DomainError::new(ErrorCode::NotFound, "Connection is unavailable")
            })?;
        let route = state
            .snapshot
            .routes
            .iter()
            .find(|route| route.connection_id == connection)
            .ok_or_else(unavailable)?;
        Ok(ProbeTarget {
            connection_id: connection.to_owned(),
            located,
            route_id: route.id.clone(),
            base: ProbeBase {
                background: None,
                scan: state.scan_generation,
                connection: *state.connection_generations.get(connection).unwrap_or(&0),
            },
        })
    }

    pub(crate) fn has_fresh_metadata(&self, connection: &str) -> Result<bool> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        Ok(state.metadata.get(connection).is_some_and(|entry| {
            !entry.invalidated && entry.observed.elapsed() < METADATA_PROBE_CACHE_TTL
        }))
    }

    pub(crate) fn target_is_current(&self, target: &ProbeTarget) -> Result<bool> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        Ok(state.scan_generation == target.base.scan
            && target
                .base
                .background
                .is_none_or(|g| g == state.background_generation)
            && *state
                .connection_generations
                .get(&target.connection_id)
                .unwrap_or(&0)
                == target.base.connection)
    }

    /// Publish timer-driven freshness transitions once, without changing any
    /// provider observation time, reading, auth state, or reset timestamp.
    pub(crate) fn expire_observations(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let mut next = state.snapshot.clone();
        for entry in state.metadata.values() {
            entry.merge(&mut next);
        }
        let changes = diff_snapshot(&state.snapshot, &next);
        if changes.is_empty() {
            return Ok(());
        }
        let issues = self.compose_issues_locked(&state, &next);
        next.health = summarize(&issues, state.hydrated);
        next.issues = issues;
        self.commit_locked(&mut state, &mut next, None, "freshness_expired", changes, 0)
    }

    pub(crate) fn metadata_failure(&self, connection: &str) -> Result<Option<DomainError>> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        let now = Instant::now();
        Ok(state.metadata_failures.get(connection).and_then(|failure| {
            (failure.until > now).then(|| {
                let mut error = failure.error.clone();
                if error.retryable {
                    let remaining = failure.until.saturating_duration_since(now);
                    error.retry_after_ms = Some(
                        u64::try_from(remaining.as_millis())
                            .unwrap_or(u64::MAX)
                            .max(1),
                    );
                }
                error
            })
        }))
    }

    pub(crate) fn publish_metadata(
        &self,
        base: &ProbeBase,
        report: ConnectionMetadata,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if base
            .background
            .is_some_and(|g| g != state.background_generation)
            || state.scan_generation != base.scan
            || *state
                .connection_generations
                .get(&report.connection_id)
                .unwrap_or(&0)
                != base.connection
        {
            return Err(DomainError::new(
                ErrorCode::SelectionStale,
                "Inventory changed during the metadata probe",
            ));
        }
        let mut next = state.snapshot.clone();
        crate::metadata::merge_report(&mut next, &report, false);
        next.observed_at = Timestamp::now();
        state.metadata_failures.remove(&report.connection_id);
        let observed_at_ms = unix_ms(SystemTime::now());
        let now = Instant::now();
        let mut entry = MetadataEntry {
            observed: now,
            usage_observed: now,
            next_reset_check: reset_check(&report, (self.wall_clock)()),
            report,
            invalidated: false,
        };
        let changes = {
            let mut preview = next.clone();
            let issue_set = self.compose_issues_locked(&state, &preview);
            preview.issues = issue_set.clone();
            preview.health = summarize(&issue_set, state.hydrated);
            let mut changes = diff_snapshot(&state.snapshot, &preview);
            if changes.is_empty() {
                changes.push(ResourceChange {
                    resource: "connection".into(),
                    id: entry.report.connection_id.clone(),
                    change: ResourceChangeKind::Updated,
                    reason: Some("metadata observation refreshed".into()),
                });
            }
            next = preview;
            changes
        };
        self.commit_locked(
            &mut state,
            &mut next,
            Some(&mut entry),
            "metadata",
            changes,
            observed_at_ms,
        )?;
        *state
            .connection_generations
            .entry(entry.report.connection_id.clone())
            .or_default() += 1;
        state.dirty.remove(&entry.report.connection_id);
        state
            .metadata
            .insert(entry.report.connection_id.clone(), entry);
        Ok(())
    }

    pub(crate) fn fail_metadata(
        &self,
        base: &ProbeBase,
        connection: &str,
        error: DomainError,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if base
            .background
            .is_some_and(|g| g != state.background_generation)
            || state.scan_generation != base.scan
            || *state.connection_generations.get(connection).unwrap_or(&0) != base.connection
        {
            return Err(DomainError::new(
                ErrorCode::SelectionStale,
                "Inventory changed during the metadata probe",
            ));
        }
        let attempts = state
            .metadata_failures
            .get(connection)
            .map(|failure| failure.attempts.saturating_add(1))
            .unwrap_or(1)
            .min(32);
        let delay = metadata_backoff(connection, attempts, &error);
        state.metadata_failures.insert(
            connection.into(),
            MetadataFailure {
                until: Instant::now() + delay,
                attempts,
                error: error.clone(),
            },
        );
        if let Some(entry) = state.metadata.get_mut(connection) {
            entry.invalidated = true;
        }
        state.dirty.insert(connection.into());
        *state
            .connection_generations
            .entry(connection.into())
            .or_default() += 1;
        let mut next = state.snapshot.clone();
        // The last-known observation remains visible as stale; failure never
        // rewrites it as fresh or pretends the account signed out.
        for entry in state.metadata.values() {
            entry.merge(&mut next);
        }
        next.observed_at = Timestamp::now();
        let issues = self.compose_issues_locked(&state, &next);
        next.issues = issues.clone();
        next.health = summarize(&issues, state.hydrated);
        let changes = diff_snapshot(&state.snapshot, &next);
        if !changes.is_empty() {
            self.commit_locked(&mut state, &mut next, None, "metadata_failure", changes, 0)?;
        }
        drop(state);
        self.wake.notify_waiters();
        Ok(())
    }

    pub(crate) fn watch_roots(&self) -> Result<BTreeMap<String, PathBuf>> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        Ok(state
            ._locations
            .iter()
            .map(|(id, client)| (id.clone(), client.profile_home.clone()))
            .collect())
    }

    /// Invalidate stale facts and immediately publish the stale transition.
    /// The background lane performs the actual re-read with debounce/backoff.
    pub(crate) fn invalidate_metadata(&self, connections: &[String]) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let mut changed = Vec::new();
        for id in connections {
            if let Some(entry) = state.metadata.get_mut(id)
                && !entry.invalidated
            {
                entry.invalidated = true;
                changed.push(id.clone());
            }
            state.dirty.insert(id.clone());
            *state.connection_generations.entry(id.clone()).or_default() += 1;
        }
        if changed.is_empty() {
            drop(state);
            self.wake.notify_waiters();
            return Ok(());
        }
        let mut next = state.snapshot.clone();
        for entry in state.metadata.values() {
            entry.merge(&mut next);
        }
        next.observed_at = Timestamp::now();
        let issues = self.compose_issues_locked(&state, &next);
        next.issues = issues.clone();
        next.health = summarize(&issues, state.hydrated);
        let changes: Vec<_> = changed
            .iter()
            .map(|id| ResourceChange {
                resource: "connection".into(),
                id: id.clone(),
                change: ResourceChangeKind::Updated,
                reason: Some("local metadata changed; refresh scheduled".into()),
            })
            .collect();
        self.commit_locked(&mut state, &mut next, None, "invalidate", changes, 0)?;
        drop(state);
        self.wake.notify_waiters();
        Ok(())
    }

    pub(crate) fn due_probes(
        &self,
        allowed_providers: &BTreeSet<String>,
        limit: usize,
        period: Duration,
        after: &str,
    ) -> Result<Vec<ProbeTarget>> {
        self.due_probes_with(allowed_providers, limit, false, period, after)
    }

    /// Usage/reset observations are refreshed on their own cadence. They use
    /// the same authoritative metadata probe and never synthesize a reset.
    pub(crate) fn usage_due_probes(
        &self,
        allowed_providers: &BTreeSet<String>,
        limit: usize,
        period: Duration,
        after: &str,
    ) -> Result<Vec<ProbeTarget>> {
        self.due_probes_with(allowed_providers, limit, true, period, after)
    }

    fn due_probes_with(
        &self,
        allowed_providers: &BTreeSet<String>,
        limit: usize,
        usage_lane: bool,
        period: Duration,
        after: &str,
    ) -> Result<Vec<ProbeTarget>> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let now = Instant::now();
        let mut connections = state.snapshot.connections.clone();
        connections.sort_by(|a, b| a.id.cmp(&b.id));
        let offset = connections.partition_point(|c| c.id.as_str() <= after);
        connections.rotate_left(offset);
        let mut due = Vec::new();
        for connection in connections {
            if !allowed_providers.contains(&connection.provider_id) {
                continue;
            }
            if state
                .metadata_failures
                .get(&connection.id)
                .is_some_and(|failure| failure.until > now)
            {
                continue;
            }
            let needs = match state.metadata.get(&connection.id) {
                None => !usage_lane,
                Some(entry) => {
                    if usage_lane {
                        (entry
                            .report
                            .quota_pools
                            .iter()
                            .any(|p| !p.windows.is_empty())
                            || entry.report.included_usage_allowed.value().is_some())
                            && (entry.usage_observed.elapsed() >= period
                                || entry.next_reset_check.is_some_and(|at| at <= now))
                    } else {
                        entry.invalidated
                            || state.dirty.contains(&connection.id)
                            || entry.observed.elapsed() >= period
                    }
                }
            };
            if !needs {
                continue;
            }
            let Some(located) = state._locations.get(&connection.id).cloned() else {
                continue;
            };
            let Some(route) = state
                .snapshot
                .routes
                .iter()
                .find(|route| route.connection_id == connection.id)
                .cloned()
            else {
                continue;
            };
            due.push(ProbeTarget {
                base: ProbeBase {
                    background: None,
                    scan: state.scan_generation,
                    connection: *state
                        .connection_generations
                        .get(&connection.id)
                        .unwrap_or(&0),
                },
                connection_id: connection.id,
                located,
                route_id: route.id,
            });
            if due.len() >= limit {
                break;
            }
        }
        Ok(due)
    }

    pub(crate) fn set_notification_preferences(
        &self,
        preferences: NotificationPreferences,
    ) -> Result<()> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        if let Some(store) = &state.store {
            store.set_preferences(&preferences).map_err(store_error)?;
        }
        drop(state);
        self.state
            .lock()
            .map_err(|_| unavailable())?
            .notification_preferences = preferences;
        Ok(())
    }

    pub(crate) fn poll_notifications(
        &self,
        issues: &[Issue],
    ) -> Result<Vec<originlm_contracts::NotificationDecision>> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let now_ms = unix_ms(SystemTime::now());
        let decisions = crate::notifications::poll(
            issues,
            &state.notification_preferences,
            state.store.as_deref(),
            &state.notification_history,
            crate::notifications::local_minute(),
            now_ms,
        );
        for decision in &decisions {
            if decision.should_notify {
                state
                    .notification_history
                    .insert(decision.dedupe_key.clone(), now_ms);
            }
        }
        Ok(decisions)
    }

    pub(crate) fn update_issue(
        &self,
        issue_id: &str,
        action: impl FnOnce(&mut Issue) -> Result<()>,
    ) -> Result<Issue> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let mut issue = state
            .issues
            .get(issue_id)
            .cloned()
            .ok_or_else(|| DomainError::new(ErrorCode::NotFound, "Issue is unavailable"))?;
        action(&mut issue)?;
        state.issues.insert(issue.id.clone(), issue.clone());
        let mut next = state.snapshot.clone();
        next.issues = state.issues.values().cloned().collect();
        next.health = summarize(&next.issues, state.hydrated);
        let changes = vec![ResourceChange {
            resource: "issue".into(),
            id: issue.id.clone(),
            change: ResourceChangeKind::Updated,
            reason: Some(format!("{:?}", issue.state)),
        }];
        self.commit_locked(&mut state, &mut next, None, "issue_update", changes, 0)?;
        Ok(issue)
    }

    pub async fn refresh(self: &Arc<Self>) -> Result<()> {
        self.refresh_with_owner(None).await
    }

    pub(crate) async fn refresh_background(self: &Arc<Self>, generation: u64) -> Result<()> {
        self.refresh_with_owner(Some(generation)).await
    }

    pub(crate) fn cancel_background_refresh(&self) -> u64 {
        if let Ok(mut state) = self.state.lock() {
            state.background_generation = state.background_generation.wrapping_add(1);
            if state.flight_background {
                state.flight_host_cancelled = true;
                if let Some(cancel) = &state.flight_cancel {
                    cancel.store(true, Ordering::SeqCst);
                    if let Some(wake) = &state.flight_cancel_wake {
                        wake.notify_one();
                    }
                }
            }
            state.background_generation
        } else {
            0
        }
    }

    pub(crate) fn set_inventory_refresh(&self, enabled: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.inventory_enabled = enabled;
            if !enabled && state.flight_background {
                state.flight_host_cancelled = true;
                if let Some(cancel) = &state.flight_cancel {
                    cancel.store(true, Ordering::SeqCst);
                    if let Some(wake) = &state.flight_cancel_wake {
                        wake.notify_one();
                    }
                }
            }
        }
    }

    async fn refresh_with_owner(self: &Arc<Self>, background: Option<u64>) -> Result<()> {
        let mut receiver = {
            let mut state = self.state.lock().map_err(|_| unavailable())?;
            if background
                .is_some_and(|g| g != state.background_generation || !state.inventory_enabled)
            {
                return Err(DomainError::new(
                    ErrorCode::Cancelled,
                    "Background work stopped",
                ));
            }
            if state.updated.is_some_and(|at| at.elapsed() < self.ttl) {
                return Ok(());
            }
            if background.is_none()
                && !state
                    .flight_cancel
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::SeqCst))
            {
                state.flight_background = false;
            }
            if let Some(receiver) = &state.flight {
                receiver.clone()
            } else {
                if let Some((at, error)) = &state.failure {
                    let delay = metadata_backoff("inventory", state.scan_attempts, error);
                    if at.elapsed() < delay {
                        let mut safe = error.clone();
                        safe.retry_after_ms = Some(
                            u64::try_from(delay.saturating_sub(at.elapsed()).as_millis())
                                .unwrap_or(u64::MAX)
                                .max(1),
                        );
                        return Err(safe);
                    }
                }
                let (sender, receiver) = watch::channel(None);
                state.flight = Some(receiver.clone());
                let cancel = Arc::new(AtomicBool::new(false));
                let cancel_wake = Arc::new(Notify::new());
                state.flight_cancel_wake = Some(cancel_wake.clone());
                state.flight_cancel = Some(cancel.clone());
                state.flight_background = background.is_some();
                state.flight_generation = background;
                state.flight_host_cancelled = false;
                let cache = self.clone();
                tokio::spawn(async move {
                    cache.run_refresh(sender, cancel, cancel_wake).await;
                });
                receiver
            }
        };
        loop {
            let result = receiver.borrow().clone();
            if let Some(result) = result {
                return result;
            }
            receiver.changed().await.map_err(|_| unavailable())?;
        }
    }

    async fn run_refresh(
        self: Arc<Self>,
        sender: watch::Sender<RefreshResult>,
        cancelled: Arc<AtomicBool>,
        cancel_wake: Arc<Notify>,
    ) {
        let flag = cancelled.clone();
        let source = self.source.clone();
        let mut worker = tokio::task::spawn_blocking(move || source(&flag));
        let outcome = tokio::select! {
            biased;
            _ = cancel_wake.notified() => {
                let error = DomainError::new(ErrorCode::Cancelled, "Inventory reconciliation cancelled");
                sender.send_replace(Some(Err(error.clone())));
                let _ = worker.await;
                Err(error)
            }
            result = tokio::time::timeout(Duration::from_secs(5), &mut worker) => match result {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(unavailable()),
            Err(_) => {
                cancelled.store(true, Ordering::SeqCst);
                let error =
                    DomainError::new(ErrorCode::Timeout, "Native inventory deadline exceeded");
                sender.send_replace(Some(Err(error.clone())));
                let _ = worker.await;
                Err(error)
            }
        }
        };
        let outcome = if cancelled.load(Ordering::SeqCst)
            && !outcome
                .as_ref()
                .is_err_and(|e| e.code == ErrorCode::Timeout)
        {
            Err(DomainError::new(
                ErrorCode::Cancelled,
                "Inventory reconciliation cancelled",
            ))
        } else {
            outcome
        };
        let result = self.publish_scan(outcome);
        sender.send_replace(Some(result));
    }

    fn publish_scan(&self, outcome: Result<ScanResult>) -> Result<()> {
        let mut current = self.state.lock().map_err(|_| unavailable())?;
        // Ownership and cancellation are revalidated under the commit lock.
        // A foreground join made before cancellation transfers ownership; a
        // late join cannot revive an already cancelled background operation.
        let stopped = current.flight_host_cancelled
            || (current.flight_background
                && (current.flight_generation != Some(current.background_generation)
                    || !current.inventory_enabled))
            || (current
                .flight_cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::SeqCst))
                && !outcome
                    .as_ref()
                    .is_err_and(|error| error.code == ErrorCode::Timeout));
        current.flight = None;
        current.flight_cancel = None;
        current.flight_cancel_wake = None;
        current.flight_background = false;
        current.flight_generation = None;
        current.flight_host_cancelled = false;
        if stopped {
            return Err(DomainError::new(
                ErrorCode::Cancelled,
                "Inventory reconciliation cancelled",
            ));
        }
        // Scan reconciliation is infrequent. Stage its maps, native locations
        // and generations together with the snapshot; a rejected SQLite
        // transaction must leave the previous operational cache intact.
        let mut state = current.clone();
        state.storage_error = None;
        match outcome {
            Ok(result) => {
                state.failure = None;
                state.scan_generation += 1;
                let mut next = result.snapshot;
                state.metadata.retain(|id, _| {
                    next.connections
                        .iter()
                        .any(|connection| connection.id == *id)
                });
                state.metadata_failures.retain(|id, _| {
                    next.connections
                        .iter()
                        .any(|connection| connection.id == *id)
                });
                state.dirty.retain(|id| {
                    next.connections
                        .iter()
                        .any(|connection| connection.id == *id)
                });
                for entry in state.metadata.values() {
                    entry.merge(&mut next);
                }
                next.observed_at = Timestamp::now();
                let issues = self.compose_issues_locked(&state, &next);
                next.issues = issues.clone();
                next.health = summarize(&issues, state.hydrated);
                let changes = diff_snapshot(&state.snapshot, &next);
                state._locations = result.locations;
                let live: BTreeSet<_> = next.connections.iter().map(|c| c.id.clone()).collect();
                state
                    .connection_generations
                    .retain(|id, _| live.contains(id));
                let committed = self.commit_locked(&mut state, &mut next, None, "scan", changes, 0);
                match committed {
                    Ok(()) => {
                        for connection in &next.connections {
                            if !state.metadata.contains_key(&connection.id) {
                                state.dirty.insert(connection.id.clone());
                            }
                        }
                        state.snapshot = next;
                        state.updated = Some(Instant::now());
                        state.failure = None;
                        state.scan_attempts = 0;
                        *current = state;
                        drop(current);
                        self.wake.notify_waiters();
                        Ok(())
                    }
                    Err(error) => {
                        current.failure = Some((Instant::now(), error.clone()));
                        current.storage_error = Some(error.clone());
                        Err(error)
                    }
                }
            }
            Err(error) => {
                if error.code == ErrorCode::Cancelled {
                    return Err(error);
                }
                // Never echo an arbitrary adapter/OS error, which may include private paths.
                let mut safe = DomainError::new(error.code, "Native inventory is unavailable");
                safe.retryable = error.retryable;
                safe.retry_after_ms = error.retry_after_ms;
                state.scan_attempts = state.scan_attempts.saturating_add(1).min(32);
                state.failure = Some((Instant::now(), safe.clone()));
                let mut next = state.snapshot.clone();
                mark_snapshot_stale(&mut next);
                for entry in state.metadata.values_mut() {
                    entry.invalidated = true;
                }
                let issues = self.compose_issues_locked(&state, &next);
                next.health = summarize(&issues, state.hydrated);
                next.issues = issues;
                let changes = diff_snapshot(&state.snapshot, &next);
                if !changes.is_empty()
                    && let Err(error) = self.commit_locked(
                        &mut state,
                        &mut next,
                        None,
                        "inventory_failure",
                        changes,
                        0,
                    )
                {
                    current.failure = Some((Instant::now(), error.clone()));
                    current.storage_error = Some(error.clone());
                    return Err(error);
                }
                *current = state;
                Err(safe)
            }
        }
    }

    fn compose_issues_locked(&self, state: &CacheState, snapshot: &Snapshot) -> Vec<Issue> {
        let mut issues = derive_issues(snapshot, &state.issues, state.hydrated);
        if let Some((_, error)) = &state.failure {
            // A source-wide inventory error is visible without revealing private paths.
            let issue = crate::health::inventory_issue(error);
            issues.retain(|old| old.id != issue.id);
            issues.push(merge_issue(state.issues.get(&issue.id), issue));
        }
        for (connection, failure) in &state.metadata_failures {
            let now = Timestamp::now();
            let extra = Issue {
                id: originlm_contracts::stable_id("issue", &["adapter_failure", connection]),
                fingerprint: connection.clone(),
                kind: originlm_contracts::IssueKind::AdapterFailure,
                severity: originlm_contracts::IssueSeverity::Warning,
                state: IssueState::Open,
                title: "Provider metadata could not be refreshed".into(),
                summary: "Last-known data remains visible as stale. The runtime retries with backoff.".into(),
                provider_id: snapshot
                    .connections
                    .iter()
                    .find(|candidate| &candidate.id == connection)
                    .map(|candidate| candidate.provider_id.clone()),
                connection_id: Some(connection.clone()),
                account_id: None,
                route_id: None,
                pool_id: None,
                window_id: None,
                first_seen_at: now.clone(),
                last_seen_at: now,
                resolved_at: None,
                snoozed_until: None,
                occurrences: failure.attempts.max(1),
                evidence: vec![originlm_contracts::IssueEvidence {
                    code: format!("{:?}", failure.error.code),
                    message: failure.error.message.clone(),
                    source: Some(originlm_contracts::EvidenceSource::new(
                        originlm_contracts::EvidenceKind::Derived,
                        "originlm-service",
                        "metadata refresh",
                    )),
                    observed_at: Some(Timestamp::now()),
                    freshness: Some(originlm_contracts::Freshness::Fresh),
                    verification: Some(originlm_contracts::Verification::Observed),
                }],
                remediation: vec![originlm_contracts::Remediation {
                    kind: originlm_contracts::RemediationKind::RetryLater,
                    label: "Retry later".into(),
                    description: "The runtime retries with bounded backoff. Use Refresh for an explicit retry.".into(),
                    requires_user_action: false,
                }],
                last_observation: None,
            };
            if let Some(existing) = state.issues.get(&extra.id) {
                let merged = merge_issue(Some(existing), extra);
                if let Some(slot) = issues.iter_mut().find(|issue| issue.id == merged.id) {
                    *slot = merged;
                } else {
                    issues.push(merged);
                }
            } else {
                issues.push(extra);
            }
        }
        if let Some(error) = &state.storage_error {
            let issue = storage_issue(error, true);
            if let Some(existing) = state.issues.get(&issue.id) {
                let merged = merge_issue(Some(existing), issue);
                if let Some(slot) = issues
                    .iter_mut()
                    .find(|candidate| candidate.id == merged.id)
                {
                    *slot = merged;
                } else {
                    issues.push(merged);
                }
            } else {
                issues.push(issue);
            }
        }
        issues.sort_by(|left, right| {
            right
                .severity
                .cmp(&left.severity)
                .then_with(|| right.last_seen_at.as_str().cmp(left.last_seen_at.as_str()))
        });
        issues.truncate(crate::health::MAX_ISSUES);
        issues
    }

    fn compose_and_commit(
        &self,
        reason: &str,
        metadata: Option<DurableMetadata>,
        force_change: bool,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        let mut next = state.snapshot.clone();
        for entry in state.metadata.values() {
            crate::metadata::merge_report(
                &mut next,
                &entry.report,
                entry.invalidated || metadata_is_stale(entry.observed.elapsed()),
            );
        }
        next.observed_at = Timestamp::now();
        let issues = self.compose_issues_locked(&state, &next);
        next.issues = issues.clone();
        next.health = summarize(&issues, state.hydrated);
        let mut changes = diff_snapshot(&state.snapshot, &next);
        if changes.is_empty() && force_change {
            changes.push(ResourceChange {
                resource: "inventory".into(),
                id: "*".into(),
                change: ResourceChangeKind::Updated,
                reason: Some(reason.into()),
            });
        }
        self.commit_locked(
            &mut state,
            &mut next,
            None,
            reason,
            changes,
            metadata.as_ref().map(|m| m.observed_at_ms).unwrap_or(0),
        )?;
        if let Some(metadata) = metadata
            && let Some(entry) = state.metadata.get_mut(&metadata.report.connection_id)
        {
            entry.report = metadata.report;
        }
        Ok(())
    }

    fn commit_locked(
        &self,
        state: &mut CacheState,
        next: &mut Snapshot,
        metadata: Option<&mut MetadataEntry>,
        reason: &str,
        changes: Vec<ResourceChange>,
        observed_at_ms: u64,
    ) -> Result<()> {
        let issues_changed = !json_eq(&state.snapshot.issues, &next.issues);
        if let Some(store) = &state.store {
            let durable = metadata.as_ref().map(|entry| DurableMetadata {
                observed_at_ms: if observed_at_ms == 0 {
                    unix_ms(SystemTime::now())
                } else {
                    observed_at_ms
                },
                report: entry.report.clone(),
            });
            match store.commit_snapshot(next, durable, reason, changes, issues_changed) {
                Ok(committed) => {
                    state.revision = committed.revision.parse().unwrap_or(state.revision + 1);
                    state.cursor = committed.cursor.clone();
                    state.journal.push_back(committed.change);
                    while state.journal.len() > JOURNAL_LIMIT {
                        state.journal.pop_front();
                    }
                }
                Err(error) => {
                    let safe = storage_failure(error.domain());
                    state.storage_error = Some(safe.clone());
                    return Err(safe);
                }
            }
        } else {
            let revision = state.revision.saturating_add(1);
            let sequence = revision;
            let cursor = format!("{}:{sequence}", state.epoch);
            next.revision = revision.to_string();
            next.cursor = cursor.clone();
            let change = InventoryChange {
                sequence: sequence.to_string(),
                cursor,
                revision: revision.to_string(),
                previous_revision: state.revision.to_string(),
                observed_at: next.observed_at.clone(),
                reason: reason.into(),
                changes,
                issues_changed,
            };
            state.revision = revision;
            state.cursor = change.cursor.clone();
            state.journal.push_back(change);
            while state.journal.len() > JOURNAL_LIMIT {
                state.journal.pop_front();
            }
        }
        state.snapshot = next.clone();
        for issue in &next.issues {
            state.issues.insert(issue.id.clone(), issue.clone());
        }
        state
            .issues
            .retain(|id, _| next.issues.iter().any(|issue| &issue.id == id));
        Ok(())
    }
}

#[cfg(test)]
mod backoff_tests {
    use super::metadata_backoff;
    use originlm_contracts::{DomainError, ErrorCode};
    use std::time::Duration;

    #[test]
    fn retry_jitter_never_exceeds_the_thirty_second_cap() {
        let error = DomainError::new(ErrorCode::ProviderUnavailable, "safe").retryable();
        assert!(metadata_backoff("cap", 32, &error) <= Duration::from_secs(30));
    }
}

#[cfg(test)]
mod metadata_freshness_tests {
    use super::metadata_is_stale;
    use std::time::Duration;

    #[test]
    fn a_recent_authentication_check_stays_fresh_while_the_user_checks_other_profiles() {
        assert!(!metadata_is_stale(Duration::from_secs(61)));
        assert!(!metadata_is_stale(Duration::from_secs(15 * 60 - 1)));
        assert!(metadata_is_stale(Duration::from_secs(15 * 60)));
    }
}

#[cfg(test)]
mod refresh_generation_tests {
    use super::*;
    use originlm_contracts::*;
    #[allow(dead_code)]
    mod common {
        include!("../../originlm-core/tests/common/mod.rs");
    }

    fn seeded() -> InventoryCache {
        let cache = InventoryCache::new(
            Arc::new(|_| unreachable!("no external source")),
            Duration::ZERO,
        );
        cache
            .publish_scan(Ok(ScanResult {
                snapshot: common::fixture(),
                locations: BTreeMap::from([(
                    "personal".into(),
                    LocatedClient {
                        executable_identity: None,
                        provider_id: "test".into(),
                        profile_home: PathBuf::from("synthetic"),
                        executable: None,
                    },
                )]),
                metadata_probes: 0,
            }))
            .unwrap();
        cache
    }

    #[test]
    fn a_failure_cannot_publish_after_a_generation_change_even_if_a_prior_check_passed() {
        let cache = seeded();
        let target = cache.probe_input("personal").unwrap();
        assert!(cache.target_is_current(&target).unwrap());
        cache.invalidate_metadata(&["personal".into()]).unwrap();
        assert_eq!(
            cache
                .fail_metadata(
                    &target.base,
                    "personal",
                    DomainError::new(ErrorCode::Timeout, "synthetic")
                )
                .unwrap_err()
                .code,
            ErrorCode::SelectionStale
        );
        assert!(cache.metadata_failure("personal").unwrap().is_none());
        let target = cache.probe_input("personal").unwrap();
        cache
            .publish_scan(Ok(ScanResult {
                snapshot: common::fixture(),
                locations: BTreeMap::new(),
                metadata_probes: 0,
            }))
            .unwrap();
        assert_eq!(
            cache
                .fail_metadata(
                    &target.base,
                    "personal",
                    DomainError::new(ErrorCode::Timeout, "synthetic")
                )
                .unwrap_err()
                .code,
            ErrorCode::SelectionStale
        );
    }

    #[test]
    fn rejected_scan_commit_preserves_locations_metadata_generations_and_dirty_work() {
        let cache = seeded();
        let fixture = common::fixture();
        let report = ConnectionMetadata {
            connection_id: "personal".into(),
            route_id: "route-personal".into(),
            version: Observation::unknown("TEST"),
            auth: fixture.connections[0].auth.clone(),
            plan: Observation::unknown("TEST"),
            billing: fixture.routes[0].billing.clone(),
            account: None,
            models: vec![],
            providers: vec![],
            additional_routes: vec![],
            quota_pools: vec![],
            included_usage_allowed: Observation::unknown("TEST"),
            diagnostics: vec![],
        };
        cache
            .publish_metadata(&cache.probe_input("personal").unwrap().base, report)
            .unwrap();
        cache
            .fail_metadata(
                &cache.probe_input("personal").unwrap().base,
                "personal",
                DomainError::new(ErrorCode::Timeout, "synthetic").retryable(),
            )
            .unwrap();
        let target = cache.probe_input("personal").unwrap();
        let before = cache.snapshot().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.db");
        drop(StateStore::open(&path).unwrap());
        cache.state.lock().unwrap().store =
            Some(Arc::new(StateStore::open_read_only(&path).unwrap()));
        assert_eq!(
            cache
                .publish_scan(Ok(ScanResult {
                    snapshot: Snapshot::empty(DataMode::Synthetic),
                    locations: BTreeMap::new(),
                    metadata_probes: 0
                }))
                .unwrap_err()
                .code,
            ErrorCode::StorageUnavailable
        );
        assert!(
            cache.probe_input("personal").is_ok(),
            "the old visible connection must keep its native location after a rejected commit"
        );
        assert!(
            cache.target_is_current(&target).unwrap(),
            "an uncommitted scan must not advance the scan generation"
        );
        let state = cache.state.lock().unwrap();
        assert!(state.metadata.contains_key("personal"));
        assert!(state.metadata_failures.contains_key("personal"));
        assert!(state.dirty.contains("personal"));
        assert_eq!(state.snapshot.revision, before.revision);
        assert_eq!(state.snapshot.cursor, before.cursor);
    }

    #[test]
    fn cancellation_after_the_outer_check_rejects_inventory_success_and_failure_at_commit() {
        for error in [
            None,
            Some(ErrorCode::ProviderUnavailable),
            Some(ErrorCode::Timeout),
        ] {
            let cache = seeded();
            cache.set_inventory_refresh(true);
            let cancelled = Arc::new(AtomicBool::new(false));
            {
                let mut state = cache.state.lock().unwrap();
                state.flight_cancel = Some(cancelled.clone());
                state.flight_background = true;
            }
            let before = cache.snapshot().unwrap();
            assert!(!cancelled.load(Ordering::SeqCst)); // Outer runtime check passed.
            cache.cancel_background_refresh(); // Stop wins before the commit lock.
            let outcome = match error {
                Some(code) => Err(DomainError::new(code, "synthetic")),
                None => Ok(ScanResult {
                    snapshot: Snapshot::empty(DataMode::Synthetic),
                    locations: BTreeMap::new(),
                    metadata_probes: 0,
                }),
            };
            assert_eq!(
                cache.publish_scan(outcome).unwrap_err().code,
                ErrorCode::Cancelled
            );
            assert_eq!(cache.snapshot().unwrap(), before);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn committed_reset_deadline_survives_forward_and_backward_wall_clock_jumps() {
        let mut cache = seeded();
        let initial = chrono::DateTime::parse_from_rfc3339("2026-10-22T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let wall = Arc::new(Mutex::new(initial));
        let clock = wall.clone();
        cache.wall_clock = Arc::new(move || *clock.lock().unwrap());
        let fixture = common::fixture();
        let mut pool = fixture.quota_pools[0].clone();
        pool.id = "clock-pool".into();
        pool.connection_ids = vec!["personal".into()];
        pool.route_ids = vec!["route-personal".into()];
        pool.windows = vec![UsageWindow {
            id: "window".into(),
            label: "Synthetic".into(),
            reading: Observation::reported(
                UsageReading::try_from(UsageInput {
                    unit: "requests".into(),
                    used: "80".to_owned().try_into().unwrap(),
                    limit: Some("100".to_owned().try_into().unwrap()),
                })
                .unwrap(),
                EvidenceSource::new(EvidenceKind::Synthetic, "test", "usage"),
            ),
            resets_at: Some(Timestamp::try_from("2026-10-22T12:00:30Z".to_owned()).unwrap()),
            duration_seconds: None,
        }];
        let report = ConnectionMetadata {
            connection_id: "personal".into(),
            route_id: "route-personal".into(),
            version: Observation::unknown("TEST"),
            auth: fixture.connections[0].auth.clone(),
            plan: Observation::unknown("NOT_REPORTED"),
            billing: fixture.routes[0].billing.clone(),
            account: None,
            models: vec![],
            additional_routes: vec![],
            providers: vec![],
            quota_pools: vec![pool],
            included_usage_allowed: Observation::unknown("NOT_REPORTED"),
            diagnostics: vec![],
        };
        cache
            .publish_metadata(&cache.probe_input("personal").unwrap().base, report)
            .unwrap();
        let providers = BTreeSet::from(["test".into()]);
        *wall.lock().unwrap() = initial + chrono::Duration::days(3650);
        assert!(
            cache
                .usage_due_probes(&providers, 64, Duration::from_secs(3600), "")
                .unwrap()
                .is_empty()
        );
        *wall.lock().unwrap() = initial - chrono::Duration::days(3650);
        tokio::time::advance(Duration::from_secs(31)).await;
        assert_eq!(
            cache
                .usage_due_probes(&providers, 64, Duration::from_secs(3600), "")
                .unwrap()
                .len(),
            1
        );
        let snapshot = cache.snapshot().unwrap();
        let pool = snapshot
            .quota_pools
            .iter()
            .find(|p| p.id == "clock-pool")
            .unwrap();
        assert_eq!(pool.windows[0].reading.value().unwrap().used.as_str(), "80");
        assert!(pool.windows[0].reading.fresh_value().is_none());
    }

    #[test]
    fn disabling_and_reenabling_inventory_cannot_revive_a_cancelled_timeout() {
        let cache = seeded();
        cache.set_inventory_refresh(true);
        let before = cache.snapshot().unwrap();
        {
            let mut state = cache.state.lock().unwrap();
            state.flight_background = true;
            state.flight_generation = Some(state.background_generation);
            state.flight_cancel = Some(Arc::new(AtomicBool::new(true)));
        } // Deadline already elapsed.
        cache.set_inventory_refresh(false);
        cache.set_inventory_refresh(true);
        assert_eq!(
            cache
                .publish_scan(Err(DomainError::new(ErrorCode::Timeout, "synthetic")))
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(cache.snapshot().unwrap(), before);
    }

    #[test]
    fn an_uncancelled_owned_timeout_still_publishes_an_honest_failure() {
        let cache = seeded();
        cache.set_inventory_refresh(true);
        let before = cache.snapshot().unwrap();
        {
            let mut state = cache.state.lock().unwrap();
            state.flight_background = true;
            state.flight_generation = Some(state.background_generation);
            state.flight_cancel = Some(Arc::new(AtomicBool::new(true)));
        } // Source deadline, no host cancellation.
        assert_eq!(
            cache
                .publish_scan(Err(DomainError::new(ErrorCode::Timeout, "synthetic")))
                .unwrap_err()
                .code,
            ErrorCode::Timeout
        );
        let after = cache.snapshot().unwrap();
        assert_ne!(after.revision, before.revision);
        assert!(
            after
                .issues
                .iter()
                .any(|issue| issue.kind == IssueKind::AdapterFailure)
        );
    }

    #[tokio::test]
    async fn immediate_restart_retries_after_the_cancelled_old_inventory_flight_unwinds() {
        use crate::NativeService;
        use originlm_core::ClientPolicy;
        use serde_json::json;
        use std::sync::atomic::AtomicUsize;
        let calls = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));
        let (count, free) = (calls.clone(), release.clone());
        let service = Arc::new(NativeService::new(
            Arc::new(move |cancel| {
                let n = count.fetch_add(1, Ordering::SeqCst);
                if n == 1 {
                    let deadline = std::time::Instant::now() + Duration::from_secs(2);
                    while !free.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                        std::thread::yield_now();
                    }
                    assert!(cancel.load(Ordering::SeqCst));
                }
                let mut snapshot = common::fixture();
                snapshot.connections[0].display_name = format!("scan-{n}");
                Ok(ScanResult {
                    snapshot,
                    locations: BTreeMap::new(),
                    metadata_probes: 0,
                })
            }),
            Duration::ZERO,
        ));
        service
            .register_client("app", ClientPolicy::passive())
            .unwrap();
        service
            .handle("app", "discovery.refresh", json!({}))
            .await
            .unwrap();
        tokio::time::pause();
        service.enable_inventory_refresh(true);
        assert!(service.start_background_tasks());
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_millis(1)).await;
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while calls.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
            tokio::task::yield_now().await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        service.stop_background_tasks();
        assert!(service.start_background_tasks());
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_millis(1)).await;
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
        release.store(true, Ordering::SeqCst);
        // Do not advance a paused timeout while its blocking worker is still
        // delivering a result. Wait for publication/retirement under the same
        // cache mutex used by production, then advance the supervisor tick.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while service.cache.state.lock().unwrap().flight.is_some()
            && std::time::Instant::now() < deadline
        {
            tokio::task::yield_now().await;
        }
        assert!(service.cache.state.lock().unwrap().flight.is_none());
        tokio::time::advance(Duration::from_secs(6)).await;
        while calls.load(Ordering::SeqCst) < 3 && std::time::Instant::now() < deadline {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "the new supervisor must retry before the full inventory cadence"
        );
        while service.cache.state.lock().unwrap().flight.is_some()
            && std::time::Instant::now() < deadline
        {
            tokio::task::yield_now().await;
        }
        assert!(service.cache.state.lock().unwrap().flight.is_none());
        let after = service
            .handle("app", "snapshot.get", json!({}))
            .await
            .unwrap();
        assert_eq!(after["connections"][0]["displayName"], "scan-2");
        service.stop_background_tasks();
    }
}
