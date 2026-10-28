//! One native service shared by every transport. Only the trusted host sets policies.
mod execution;
mod inputs;
mod inventory_events;
pub use execution::{
    ExecutionGrant, ExecutionInput, ExecutionSource, PERSISTENT_RESUME, PersistentResume,
};
mod cache;
mod catalog;
mod costs;
mod delivery;
pub use delivery::NotificationSink;
mod health;
mod metadata;
mod methods;
mod notifications;
mod params;
mod refresh;
mod resume_token;
use cache::InventoryCache;
pub use metadata::{MetadataFuture, MetadataSource};
use originlm_chatgpt_auth::AuthManager;
use originlm_contracts::{DomainError, ErrorCode, MAX_TURN_START_PARAMS_BYTES, Result, Snapshot};
use originlm_core::{ClientPolicy, project_snapshot};
use originlm_discovery::ScanResult;
use originlm_platform::watcher::DebouncedMetadataWatcher;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock, atomic::AtomicBool},
    time::Duration,
};

pub type InventorySource = Arc<dyn Fn(&AtomicBool) -> Result<ScanResult> + Send + Sync>;
pub type AppRevoker = Arc<dyn Fn(&str) -> bool + Send + Sync>;
pub struct NativeService {
    cost_prices: RwLock<costs::Prices>,
    clients: Arc<RwLock<BTreeMap<String, ClientPolicy>>>,
    adapters: Vec<originlm_contracts::AdapterDescriptor>,
    execution_state: execution::ExecutionState,
    cache: Arc<InventoryCache>,
    metadata_source: Option<MetadataSource>,
    metadata_locks: std::sync::Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<()>>>>,
    watchers: std::sync::Mutex<BTreeMap<String, DebouncedMetadataWatcher>>,
    /// Providers whose metadata the host authorized for the autonomous lane.
    refresh_providers: RwLock<BTreeSet<String>>,
    background: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    inventory_refresh: AtomicBool,
    client_interest: std::sync::Mutex<BTreeMap<String, tokio::time::Instant>>,
    metadata_slots: tokio::sync::Semaphore,
    /// Stable application identity per connection, for durable resume tokens.
    app_identities: RwLock<BTreeMap<String, String>>,
    /// Broker-owned hook that removes a reusable application grant when an
    /// administrator revokes the application. The service has no socket or
    /// credential access of its own.
    app_revoker: std::sync::Mutex<Option<AppRevoker>>,
    /// Host-installed notification delivery. Empty by default; the broker only
    /// computes decisions until a host explicitly opts in.
    notification_sink: std::sync::Mutex<Option<Arc<dyn delivery::NotificationSink>>>,
    chatgpt_auth: Option<Arc<AuthManager>>,
    /// Serializes managed sign-out with session publication and turn enqueue.
    chatgpt_lifecycle: tokio::sync::Mutex<()>,
}
impl NativeService {
    pub fn new(source: InventorySource, ttl: Duration) -> Self {
        Self {
            cost_prices: RwLock::new(costs::Prices::default()),
            clients: Arc::new(RwLock::new(BTreeMap::new())),
            adapters: Vec::new(),
            execution_state: Default::default(),
            metadata_source: None,
            metadata_locks: std::sync::Mutex::new(BTreeMap::new()),
            watchers: std::sync::Mutex::new(BTreeMap::new()),
            refresh_providers: RwLock::new(BTreeSet::new()),
            background: std::sync::Mutex::new(None),
            inventory_refresh: AtomicBool::new(false),
            client_interest: std::sync::Mutex::new(BTreeMap::new()),
            metadata_slots: tokio::sync::Semaphore::new(2),
            app_identities: RwLock::new(BTreeMap::new()),
            app_revoker: std::sync::Mutex::new(None),
            notification_sink: std::sync::Mutex::new(None),
            chatgpt_auth: None,
            chatgpt_lifecycle: tokio::sync::Mutex::new(()),
            cache: Arc::new(InventoryCache::new(source, ttl)),
        }
    }
    /// Open the broker-owned SQLite state and hydrate the cached snapshot
    /// before the server starts accepting clients.
    pub fn enable_state_store(&self, path: std::path::PathBuf) -> Result<()> {
        self.cache.enable_store(path)
    }
    /// Host-only: providers for the autonomous metadata lane. The lane uses the
    /// same grants the host already declared; it does not invent new ones.
    pub fn enable_autonomous_refresh(&self, providers: BTreeSet<String>) {
        if let Ok(mut current) = self.refresh_providers.write() {
            *current = providers;
        }
    }
    /// Host-only, independent of metadata grants. The installed source must
    /// implement passive discovery; enabling provider probes does not allow scans.
    pub fn enable_inventory_refresh(&self, enabled: bool) {
        self.inventory_refresh
            .store(enabled, std::sync::atomic::Ordering::SeqCst);
        self.cache.set_inventory_refresh(enabled);
    }
    /// Starts the separately paced inventory, metadata and usage lanes.
    /// Safe to call once; a second call returns false.
    pub fn start_background_tasks(self: &Arc<Self>) -> bool {
        let providers = self
            .refresh_providers
            .read()
            .map(|providers| providers.clone())
            .unwrap_or_default();
        if !self
            .inventory_refresh
            .load(std::sync::atomic::Ordering::SeqCst)
            && (providers.is_empty() || self.metadata_source.is_none())
        {
            return false;
        }
        let Ok(mut slot) = self.background.lock() else {
            return false;
        };
        if slot.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return false;
        }
        let service = self.clone();
        let generation = self.cache.cancel_background_refresh();
        let handle = tokio::spawn(async move { service.run_refresh_lanes(generation).await });
        *slot = Some(handle);
        true
    }
    pub fn stop_background_tasks(&self) {
        if let Ok(mut slot) = self.background.lock() {
            self.cache.cancel_background_refresh();
            if let Some(handle) = slot.take() {
                handle.abort();
            }
        }
    }
    /// Host-only: persist the last metadata observations at `path` (identity excluded) and reload
    /// them after a restart, so status is shown immediately and refreshed in the background.
    pub fn enable_metadata_cache(&self, path: std::path::PathBuf) {
        let _ = self.cache.enable_store(path);
    }
    pub fn with_adapters(mut self, adapters: Vec<originlm_contracts::AdapterDescriptor>) -> Self {
        self.adapters = adapters;
        self
    }
    /// Opt-in host-managed SIWC authentication. Callers still require an
    /// explicit managed-provider metadata grant for every auth RPC.
    pub fn with_chatgpt_auth(mut self, auth: Arc<AuthManager>) -> Self {
        self.chatgpt_auth = Some(auth);
        self
    }
    /// Host-only: install a delivery sink. Until this is called, the service
    /// only computes deduplicated decisions and never contacts the desktop.
    pub fn with_notification_sink(self, sink: Arc<dyn NotificationSink>) -> Self {
        self.set_notification_sink(sink);
        self
    }
    pub fn set_notification_sink(&self, sink: Arc<dyn NotificationSink>) {
        if let Ok(mut slot) = self.notification_sink.lock() {
            *slot = Some(sink);
        }
    }
    /// Host-only build-time opt-in for the platform desktop notifier.
    #[cfg(feature = "os-notifications")]
    pub fn with_os_notifications(self) -> Self {
        self.with_notification_sink(Arc::new(delivery::OsNotificationSink))
    }
    #[cfg(feature = "os-notifications")]
    pub fn install_os_notifications(self: &Arc<Self>) {
        self.set_notification_sink(Arc::new(delivery::OsNotificationSink));
    }
    pub(crate) fn deliver_notifications(
        &self,
        decisions: &[originlm_contracts::NotificationDecision],
    ) {
        let sink = self
            .notification_sink
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        let Some(sink) = sink else {
            return;
        };
        for decision in decisions.iter().filter(|decision| decision.should_notify) {
            // Delivery is best effort; dedupe state remains authoritative and
            // the next poll can report a reminder. Never retry a notification
            // inline, which could duplicate an alert.
            let _ = sink.deliver(decision);
        }
    }
    /// Host-only API. Duplicate registration cannot silently replace a grant.
    pub fn register_client(&self, id: &str, policy: ClientPolicy) -> Result<()> {
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err(params::invalid());
        }
        let mut clients = self.clients.write().map_err(|_| denied())?;
        if clients.contains_key(id) {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                "Client is already registered",
            ));
        }
        if clients.len() >= 64 {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                "Client capacity reached",
            ));
        }
        clients.insert(id.to_owned(), policy);
        Ok(())
    }
    /// Host-only: names the application behind a connection. Connection ids
    /// change on every pairing or restart; durable resume tokens are bound to
    /// this identity instead (default: the connection id itself).
    pub fn set_app_identity(&self, client: &str, app: &str) -> Result<()> {
        if app.is_empty() || app.len() > 128 || app.chars().any(char::is_control) {
            return Err(params::invalid());
        }
        self.policy(client)?;
        self.app_identities
            .write()
            .map_err(|_| denied())?
            .insert(client.to_owned(), app.to_owned());
        Ok(())
    }
    pub(crate) fn app_identity(&self, client: &str) -> String {
        self.app_identities
            .read()
            .ok()
            .and_then(|ids| ids.get(client).cloned())
            .unwrap_or_else(|| client.to_owned())
    }
    /// Host-only: install the broker's grant-revocation hook. Calling it again
    /// replaces the previous broker (tests start brokers sequentially).
    pub fn set_app_revoker(&self, revoker: AppRevoker) {
        if let Ok(mut slot) = self.app_revoker.lock() {
            *slot = Some(revoker);
        }
    }
    fn revoke_app_grant(&self, app: &str) -> bool {
        self.app_revoker
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .is_some_and(|revoker| revoker(app))
    }
    /// Host-only cleanup for every live connection owned by one application.
    /// A scoped grant has one app identity even when it opened several sockets.
    pub fn unregister_app(&self, app: &str) -> usize {
        let clients: Vec<String> = self
            .app_identities
            .read()
            .map(|ids| {
                ids.iter()
                    .filter(|(_, identity)| identity.as_str() == app)
                    .map(|(client, _)| client.clone())
                    .collect()
            })
            .unwrap_or_default();
        for client in &clients {
            self.unregister_client(client);
        }
        clients.len()
    }
    /// Host-only cleanup for an application connection; other clients remain live.
    pub fn unregister_client(&self, id: &str) {
        if let Ok(mut interest) = self.client_interest.lock() {
            interest.remove(id);
        }
        if let Ok(mut ids) = self.app_identities.write() {
            ids.remove(id);
        }
        self.execution_state.revoke(id);
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.remove(id);
        }
        if let Ok(mut clients) = self.clients.write() {
            clients.remove(id);
        }
    }
    /// Release only this host-registered application. Other clients and shared metadata survive.
    pub fn close_client(&self, id: &str) {
        if let Ok(mut interest) = self.client_interest.lock() {
            interest.remove(id);
        }
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.remove(id);
        }
        if let Ok(mut clients) = self.clients.write() {
            clients.remove(id);
        }
        self.execution_state.revoke(id);
    }
    fn policy(&self, id: &str) -> Result<ClientPolicy> {
        self.clients
            .read()
            .map_err(|_| denied())?
            .get(id)
            .cloned()
            .ok_or_else(denied)
    }
    fn update_visible_issue(
        &self,
        client: &str,
        params: &Value,
        action: impl FnOnce(&mut originlm_contracts::Issue) -> Result<()>,
    ) -> Result<originlm_contracts::Issue> {
        let id = params::text(params, "issueId")?.ok_or_else(params::invalid)?;
        // Filtering happens before mutation, so an inaccessible issue cannot be
        // probed for existence or changed through this endpoint.
        let visible = self
            .projected(client)?
            .issues
            .iter()
            .any(|issue| issue.id == id);
        if !visible {
            return Err(denied());
        }
        self.cache.update_issue(id, action)
    }
    fn projected(&self, id: &str) -> Result<Snapshot> {
        // Acquire policy after loading data; never reuse a policy from before an await.
        let snapshot = self.cache.snapshot()?;
        let mut snapshot = project_snapshot(&snapshot, &self.policy(id)?);
        execution::decorate(
            &mut snapshot,
            &self.execution_state.providers,
            &self.execution_state.connections,
            &self.execution_state.persistent,
        );
        Ok(snapshot)
    }
    fn set_watch(&self, client: &str, enabled: bool) -> Result<Value> {
        if !self.policy(client)?.allow_passive_discovery {
            return Err(denied());
        }
        let mut watchers = self.watchers.lock().map_err(|_| denied())?;
        if !enabled {
            watchers.remove(client);
            return Ok(serde_json::json!({"watching":false}));
        }
        if watchers.contains_key(client) {
            return Ok(serde_json::json!({"watching":true}));
        }
        let roots = self.cache.watch_roots()?;
        if roots.is_empty() {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                "Run passive discovery before enabling local metadata watching",
            ));
        }
        let cache = self.cache.clone();
        let watcher = DebouncedMetadataWatcher::start(
            roots,
            Duration::from_millis(750),
            Arc::new(move |ids| {
                let _ = cache.invalidate_metadata(&ids);
            }),
        )?;
        watchers.insert(client.to_owned(), watcher);
        Ok(serde_json::json!({"watching":true}))
    }
    pub async fn handle(&self, client: &str, method: &str, params: Value) -> Result<Value> {
        let _chatgpt_lifecycle =
            if self.chatgpt_auth.is_some() && method == "auth.chatgpt.disconnect" {
                Some(self.chatgpt_lifecycle.lock().await)
            } else {
                None
            };
        self.policy(client)?;
        // A registered but idle socket is not active UI interest. Only an
        // authenticated read or authorized activity renews the short lease.
        if matches!(
            method,
            "snapshot.get"
                | "connections.list"
                | "models.list"
                | "models.page"
                | "usage.read"
                | "health.get"
                | "events.read"
                | "connections.probe"
                | "discovery.refresh"
                | "turns.start"
                | "sessions.create"
        ) && let Ok(mut interest) = self.client_interest.lock()
        {
            interest.insert(client.to_owned(), tokio::time::Instant::now());
        }
        // Staged images and turn text have separate bounds and closed typed params.
        if method == "inputs.put" {
            params.as_object().ok_or_else(params::invalid)?;
        } else if method == "turns.start" {
            params::object_with_limit(&params, MAX_TURN_START_PARAMS_BYTES)?;
        } else {
            params::object(&params)?;
        }
        if method.len() > 128 || params.get("clientId").is_some() {
            return Err(params::invalid());
        }
        if method == "runtime.closeClient" {
            params::allow(&params, &[])?;
            self.close_client(client);
            return Ok(serde_json::json!({"closed":true}));
        }
        if method.starts_with("auth.chatgpt.") {
            if !self
                .policy(client)?
                .probe_provider_ids
                .contains("codex_managed")
            {
                return Err(denied());
            }
            let auth = self.chatgpt_auth.as_ref().ok_or_else(unsupported)?;
            let map_auth = |error: originlm_chatgpt_auth::AuthError| {
                DomainError::new(
                    originlm_contracts::ErrorCode::ProviderUnavailable,
                    error.message,
                )
            };
            let value = match method {
                "auth.chatgpt.status" => {
                    params::allow(&params, &[])?;
                    serde_json::to_value(auth.status().await.map_err(map_auth)?)
                }
                "auth.chatgpt.start" => {
                    params::allow(&params, &[])?;
                    serde_json::to_value(auth.start().await.map_err(map_auth)?)
                }
                "auth.chatgpt.cancel" => {
                    params::allow(&params, &["loginId"])?;
                    let id = params::text(&params, "loginId")?.ok_or_else(params::invalid)?;
                    auth.cancel(id).await.map_err(map_auth)?;
                    Ok(serde_json::json!({}))
                }
                "auth.chatgpt.disconnect" => {
                    params::allow(&params, &[])?;
                    // Clear durable credentials before sweeping sessions. The
                    // lifecycle lease prevents a concurrent create from
                    // publishing a new session after this sweep.
                    let mut close_result = Ok(());
                    let disconnect = auth
                        .disconnect_with(|| async {
                            close_result = self.close_managed_sessions().await;
                        })
                        .await;
                    close_result?;
                    // Remote revocation can fail after local sign-out. Never
                    // restore secrets or report the local disconnect as failed.
                    if let Err(error) = disconnect
                        && error.code != "REVOCATION_UNCONFIRMED"
                    {
                        return Err(map_auth(error));
                    }
                    Ok(serde_json::json!({}))
                }
                _ => return Err(unsupported()),
            };
            return value.map_err(|_| params::invalid());
        }
        if method == "discovery.refresh" {
            params::allow(&params, &["level"])?;
            if params::text(&params, "level")?.is_some_and(|level| level != "passive") {
                return Err(unsupported());
            }
            if !self.policy(client)?.allow_passive_discovery {
                return Err(denied());
            }
            self.cache.refresh().await?;
            if !self.policy(client)?.allow_passive_discovery {
                return Err(denied());
            }
            let snapshot = self.projected(client)?;
            return Ok(serde_json::json!({"revision":snapshot.revision,"mode":snapshot.mode}));
        }
        if method == "discovery.watch" {
            params::allow(&params, &["enabled"])?;
            return self.set_watch(client, params::boolean(&params, "enabled")?);
        }
        if method == "connections.probe" {
            return self.probe_connection(client, &params).await;
        }
        if method == "events.read" && params::text(&params, "scope")? == Some("inventory") {
            params::allow(&params, &["scope", "after"])?;
            let snapshot = self.projected(client)?;
            let after = params::text(&params, "after")?
                .map(str::to_owned)
                .unwrap_or_else(|| snapshot.cursor.clone());
            let page = self.cache.inventory_page(&after)?;
            let page = inventory_events::project(page, &snapshot, &self.policy(client)?);
            return serde_json::to_value(page).map_err(|_| params::invalid());
        }
        if method == "health.get" {
            params::allow(&params, &[])?;
            let snapshot = self.projected(client)?;
            return Ok(serde_json::json!({
                "summary": snapshot.health,
                "issues": snapshot.issues,
            }));
        }
        if method == "issues.list" {
            params::allow(&params, &[])?;
            return serde_json::to_value(self.projected(client)?.issues)
                .map_err(|_| params::invalid());
        }
        if method == "issues.acknowledge" {
            params::allow(&params, &["issueId", "until"])?;
            let issue = self.update_visible_issue(client, &params, |issue| {
                issue.state = originlm_contracts::IssueState::Acknowledged;
                issue.snoozed_until = None;
                Ok(())
            })?;
            return Ok(serde_json::json!({"issue": issue}));
        }
        if method == "issues.snooze" {
            params::allow(&params, &["issueId", "until"])?;
            let until = match params::text(&params, "until")? {
                Some(value) => originlm_contracts::Timestamp::try_from(value.to_owned())
                    .map_err(|_| params::invalid())?,
                None => {
                    let future = chrono::Utc::now() + chrono::Duration::hours(24);
                    originlm_contracts::Timestamp::try_from(
                        future.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    )
                    .map_err(|_| params::invalid())?
                }
            };
            let issue = self.update_visible_issue(client, &params, |issue| {
                issue.state = originlm_contracts::IssueState::Snoozed;
                issue.snoozed_until = Some(until.clone());
                Ok(())
            })?;
            return Ok(serde_json::json!({"issue": issue}));
        }
        if method == "notifications.poll" {
            params::allow(&params, &[])?;
            let snapshot = self.projected(client)?;
            let decisions = self.cache.poll_notifications(&snapshot.issues)?;
            self.deliver_notifications(&decisions);
            return serde_json::to_value(decisions).map_err(|_| params::invalid());
        }
        if method == "notifications.configure" {
            params::allow(
                &params,
                &[
                    "enabled",
                    "minimumSeverity",
                    "quietHoursStartMinute",
                    "quietHoursEndMinute",
                    "reminderAfterSeconds",
                    "privateContent",
                ],
            )?;
            let preferences: originlm_contracts::NotificationPreferences =
                params::decode(params.clone())?;
            if preferences
                .quiet_hours_start_minute
                .is_some_and(|minute| minute >= 1440)
                || preferences
                    .quiet_hours_end_minute
                    .is_some_and(|minute| minute >= 1440)
            {
                return Err(params::invalid());
            }
            self.cache
                .set_notification_preferences(preferences.clone())?;
            return serde_json::to_value(preferences).map_err(|_| params::invalid());
        }
        if method == "notifications.ack" {
            params::allow(&params, &["issueId", "dedupeKey"])?;
            let _ = params::text(&params, "issueId")?.ok_or_else(params::invalid)?;
            let _ = params::text(&params, "dedupeKey")?.ok_or_else(params::invalid)?;
            return Ok(serde_json::json!({"acknowledged": true}));
        }
        if method == "grants.revoke" {
            let value = self.dispatch(client, method, &params)?;
            self.execution_state.revoke(client);
            if let Ok(mut watchers) = self.watchers.lock() {
                watchers.remove(client);
            }
            return Ok(value);
        }
        if method.starts_with("sessions.")
            || method.starts_with("turns.")
            || method.starts_with("inputs.")
            || method == "approvals.respond"
            || method == "events.read"
            || method == "artifacts.read"
        {
            return self.execute_method(client, method, params).await;
        }
        self.dispatch(client, method, &params)
    }
}
fn denied() -> DomainError {
    DomainError::new(
        ErrorCode::LocalAccessDenied,
        "Local access is not authorized",
    )
}
fn unsupported() -> DomainError {
    DomainError::new(
        ErrorCode::CapabilityUnsupported,
        "Operation is not available in this runtime",
    )
}
