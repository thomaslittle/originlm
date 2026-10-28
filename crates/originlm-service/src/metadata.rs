use crate::{NativeService, denied, params, unsupported};
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
use serde_json::{Value, json};
use std::{collections::BTreeSet, future::Future, pin::Pin, sync::Arc, time::Duration};
pub type MetadataFuture = Pin<Box<dyn Future<Output = Result<ConnectionMetadata>> + Send>>;
pub type MetadataSource =
    Arc<dyn Fn(LocatedClient, String, String) -> MetadataFuture + Send + Sync>;
impl NativeService {
    pub fn with_metadata(mut self, source: MetadataSource) -> Self {
        self.metadata_source = Some(source);
        self
    }
    fn ensure_probe_permission(
        &self,
        client: &str,
        connection: &str,
        provider: &str,
    ) -> Result<()> {
        let policy = self.policy(client)?;
        if !policy.allows(connection)
            || !(policy.probe_connection_ids.contains(connection)
                || policy.probe_provider_ids.contains(provider))
        {
            return Err(denied());
        }
        Ok(())
    }
    /// Re-read a connection's live account/auth status right before a session
    /// opens, the way native clients judge login at session start. Metadata
    /// newer than the short probe cache is already live evidence; anything
    /// older (including evidence past the freshness TTL) is re-probed. Failures
    /// are recorded like any probe failure, so a logged-out client stops
    /// looking authenticated instead of riding a stale cache. The caller's
    /// execution grant authorizes this read: opening the session launches the
    /// same client anyway.
    pub(crate) async fn refresh_for_session(&self, client: &str, route_id: &str) {
        let Some(source) = self.metadata_source.as_ref() else {
            return;
        };
        let Ok(snapshot) = self.cache.snapshot() else {
            return;
        };
        let Some(id) = snapshot
            .routes
            .iter()
            .find(|route| route.id == route_id)
            .map(|route| route.connection_id.clone())
        else {
            return;
        };
        if !self.policy(client).is_ok_and(|policy| policy.allows(&id)) {
            return;
        }
        let lock = {
            let Ok(mut locks) = self.metadata_locks.lock() else {
                return;
            };
            locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            if locks.len() >= 64 {
                return;
            }
            locks
                .entry(id.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        let Ok(_slot) = self.metadata_slots.acquire().await else {
            return;
        };
        if self.cache.has_fresh_metadata(&id).unwrap_or(true)
            || self.cache.metadata_failure(&id).ok().flatten().is_some()
        {
            return;
        }
        let Ok(target) = self.cache.probe_input(&id) else {
            return;
        };
        let result = tokio::time::timeout(
            Duration::from_secs(25),
            source(target.located, id.clone(), target.route_id.clone()),
        )
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorCode::Timeout,
                "Metadata probe timed out",
            ))
        });
        match result
            .and_then(|report| validate_report(&report, &id, &target.route_id).map(|()| report))
        {
            Ok(report) => {
                let _ = self.cache.publish_metadata(&target.base, report);
            }
            Err(error) => {
                let mut safe =
                    DomainError::new(error.code.clone(), "Client metadata is unavailable");
                safe.retryable = error.retryable
                    || matches!(
                        error.code,
                        ErrorCode::RateLimited
                            | ErrorCode::ProviderUnavailable
                            | ErrorCode::Timeout
                    );
                safe.retry_after_ms = error.retry_after_ms;
                let _ = self.cache.fail_metadata(&target.base, &id, safe);
            }
        }
    }
    pub(crate) async fn probe_connection(&self, client: &str, p: &Value) -> Result<Value> {
        params::allow(p, &["connectionId"])?;
        let id = params::text(p, "connectionId")?.ok_or_else(params::invalid)?;
        let source = self.metadata_source.as_ref().ok_or_else(unsupported)?;
        // Scope check precedes path lookup so inaccessible IDs cannot be enumerated.
        if !self.policy(client)?.allows(id) {
            return Err(denied());
        }
        let target = self.cache.probe_input(id)?;
        self.ensure_probe_permission(client, id, &target.located.provider_id)?;
        let lock = {
            let mut locks = self.metadata_locks.lock().map_err(|_| denied())?;
            locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            if locks.len() >= 64 {
                return Err(DomainError::new(
                    ErrorCode::RateLimited,
                    "Metadata probe concurrency reached",
                ));
            }
            locks
                .entry(id.into())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        let _slot = self.metadata_slots.acquire().await.map_err(|_| denied())?;
        let target = self.cache.probe_input(id)?;
        self.ensure_probe_permission(client, id, &target.located.provider_id)?;
        if let Some(error) = self.cache.metadata_failure(id)? {
            return Err(error);
        }
        if self.cache.has_fresh_metadata(id)? {
            return Ok(json!({"status":"cached","connectionId":id}));
        }
        let result = tokio::time::timeout(
            Duration::from_secs(25),
            source(target.located.clone(), id.into(), target.route_id.clone()),
        )
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorCode::Timeout,
                "Metadata probe timed out",
            ))
        });
        self.ensure_probe_permission(client, id, &target.located.provider_id)?;
        let report = match result {
            Ok(report) => report,
            Err(error) => {
                let retryable = error.retryable
                    || matches!(
                        error.code,
                        ErrorCode::RateLimited
                            | ErrorCode::ProviderUnavailable
                            | ErrorCode::Timeout
                    );
                let mut safe = DomainError::new(error.code, "Client metadata is unavailable");
                safe.retryable = retryable;
                safe.retry_after_ms = error.retry_after_ms;
                self.cache.fail_metadata(&target.base, id, safe.clone())?;
                return Err(self.cache.metadata_failure(id)?.unwrap_or(safe));
            }
        };
        validate_report(&report, id, &target.route_id)?;
        self.cache.publish_metadata(&target.base, report)?;
        Ok(json!({"status":"updated","connectionId":id}))
    }

    /// One selected background operation. Selection is bounded by the supervisor;
    /// all callers share the per-connection lease and global probe budget.
    pub(crate) async fn refresh_due_target(&self, target: crate::cache::ProbeTarget) {
        let Some(source) = self.metadata_source.as_ref() else {
            return;
        };
        let lock = {
            let Ok(mut locks) = self.metadata_locks.lock() else {
                return;
            };
            locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            if locks.len() >= 64 {
                return;
            }
            locks
                .entry(target.connection_id.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        // An explicit probe/session owns this connection already; do not queue
        // another request behind it. The next tick re-evaluates its evidence.
        let Ok(_guard) = lock.try_lock() else {
            return;
        };
        let Ok(_slot) = self.metadata_slots.acquire().await else {
            return;
        };
        let authorized = || {
            self.refresh_providers
                .read()
                .is_ok_and(|p| p.contains(&target.located.provider_id))
        };
        if !authorized()
            || !self.cache.target_is_current(&target).unwrap_or(false)
            || self
                .cache
                .metadata_failure(&target.connection_id)
                .ok()
                .flatten()
                .is_some()
        {
            return;
        }
        let result = tokio::time::timeout(
            Duration::from_secs(25),
            source(
                target.located.clone(),
                target.connection_id.clone(),
                target.route_id.clone(),
            ),
        )
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorCode::Timeout,
                "Metadata probe timed out",
            ))
        });
        if !authorized() || !self.cache.target_is_current(&target).unwrap_or(false) {
            return;
        }
        match result.and_then(|report| {
            validate_report(&report, &target.connection_id, &target.route_id).map(|()| report)
        }) {
            Ok(report) => {
                let _ = self.cache.publish_metadata(&target.base, report);
            }
            Err(error) => {
                let mut safe =
                    DomainError::new(error.code.clone(), "Client metadata is unavailable");
                safe.retryable = error.retryable
                    || matches!(
                        error.code,
                        ErrorCode::RateLimited
                            | ErrorCode::ProviderUnavailable
                            | ErrorCode::Timeout
                    );
                safe.retry_after_ms = error.retry_after_ms;
                let _ = self
                    .cache
                    .fail_metadata(&target.base, &target.connection_id, safe);
            }
        }
    }
}
fn validate_report(report: &ConnectionMetadata, connection: &str, route: &str) -> Result<()> {
    if report.connection_id != connection
        || report.route_id != route
        || report.models.len() > 2048
        || report.quota_pools.len() > 128
        || report.additional_routes.len() > 128
        || report.providers.len() > 128
        || report.additional_routes.iter().any(|r| {
            r.connection_id != connection
                || r.id != stable_id("route", &[route, &r.provider_id])
                || r.provider_id.is_empty()
                || r.provider_id.len() > 128
        })
    {
        return Err(params::invalid());
    }
    if serde_json::to_vec(report)
        .map_err(|_| params::invalid())?
        .len()
        > 2_097_152
    {
        return Err(DomainError::new(
            ErrorCode::FrameTooLarge,
            "Metadata result exceeds the bound",
        ));
    }
    let mut route_ids = BTreeSet::new();
    for route in &report.additional_routes {
        if !route_ids.insert(&route.id) {
            return Err(params::invalid());
        }
    }
    let mut provider_ids = BTreeSet::new();
    for provider in &report.providers {
        if !provider_ids.insert(&provider.id)
            || provider.id.is_empty()
            || provider.id.len() > 128
            || provider.name.len() > 1024
            || provider.name.chars().any(char::is_control)
        {
            return Err(params::invalid());
        }
    }
    let mut ids = BTreeSet::new();
    for model in &report.models {
        if (model.route_id != route
            && !report.additional_routes.iter().any(|r| {
                r.id == model.route_id && model.id == route_model_key(&r.id, &model.native_id)
            }))
            || !ids.insert(&model.id)
            || model.options.len() > 128
        {
            return Err(params::invalid());
        }
    }
    for pool in &report.quota_pools {
        if pool.connection_ids.iter().any(|id| id != connection)
            || pool
                .route_ids
                .iter()
                .any(|id| id != route && !report.additional_routes.iter().any(|r| &r.id == id))
            || pool.windows.len() > 128
        {
            return Err(params::invalid());
        }
    }
    Ok(())
}

pub(crate) fn merge_report(snapshot: &mut Snapshot, report: &ConnectionMetadata, stale: bool) {
    let Some(connection) = snapshot
        .connections
        .iter_mut()
        .find(|c| c.id == report.connection_id)
    else {
        return;
    };
    connection.auth = report.auth.clone();
    connection.plan = report.plan.clone();
    connection.account_id = report.account.as_ref().map(|a| a.id.clone());
    connection.diagnostics = report.diagnostics.clone();
    connection.status = if stale {
        "metadata_stale"
    } else if report.auth.fresh_value() == Some(&AuthState::Authenticated) {
        "metadata_ready"
    } else if report.account.is_some()
        && !matches!(
            report.auth,
            originlm_contracts::Observation::Known {
                value: AuthState::Unauthenticated | AuthState::Expired,
                ..
            }
        )
    {
        // A local account cache proves a session exists but not that it is
        // currently valid. That is not an auth failure, so it must not ask the
        // user to sign in again.
        "metadata_cached"
    } else {
        "needs_auth"
    }
    .into();
    if stale {
        connection.auth.mark_stale();
        connection.plan.mark_stale();
    }
    if let Some(installation) = snapshot
        .installations
        .iter_mut()
        .find(|i| i.id == connection.installation_id)
    {
        installation.version = report.version.clone();
        if stale {
            installation.version.mark_stale();
        }
    }
    if let Some(account) = &report.account {
        snapshot.accounts.retain(|a| a.id != account.id);
        snapshot.accounts.push(account.clone());
    }
    let live_accounts: BTreeSet<_> = snapshot
        .connections
        .iter()
        .filter_map(|c| c.account_id.as_deref())
        .collect();
    snapshot
        .accounts
        .retain(|account| live_accounts.contains(account.id.as_str()));
    if let Some(route) = snapshot.routes.iter_mut().find(|r| r.id == report.route_id) {
        route.billing = report.billing.clone();
        if stale {
            route.billing.mark_stale();
        }
        route.catalog_revision = stable_id(
            "catalog",
            &[&report
                .models
                .iter()
                .map(|m| m.revision.as_str())
                .collect::<Vec<_>>()
                .join("|")],
        );
        route.capabilities = vec![Capability {
            id: "generation.text".into(),
            support: Support::Unsupported,
            origin: CapabilityOrigin::Runtime,
            evidence: runtime_evidence(&report.auth, &report.billing),
            conditions: Vec::new(),
        }];
        let support = match report.included_usage_allowed.fresh_value() {
            Some(true) => Support::Supported,
            Some(false) => Support::Unsupported,
            None => Support::Unknown,
        };
        route.capabilities.push(Capability {
            id: "usage.included_allowed".into(),
            support,
            origin: CapabilityOrigin::Runtime,
            evidence: usage_evidence(&report.included_usage_allowed),
            conditions: Vec::new(),
        });
        if stale {
            for cap in &mut route.capabilities {
                cap.evidence.mark_stale();
            }
        }
    }
    let old_routes: BTreeSet<_> = snapshot
        .routes
        .iter()
        .filter(|r| r.connection_id == report.connection_id)
        .map(|r| r.id.clone())
        .collect();
    snapshot
        .models
        .retain(|m| !old_routes.contains(&m.route_id));
    snapshot
        .routes
        .retain(|r| r.connection_id != report.connection_id || r.id == report.route_id);
    for original in &report.additional_routes {
        let mut route = original.clone();
        route
            .capabilities
            .retain(|c| !(c.id == "generation.text" && c.origin == CapabilityOrigin::Runtime));
        route.capabilities.push(Capability {
            id: "generation.text".into(),
            support: Support::Unsupported,
            origin: CapabilityOrigin::Runtime,
            evidence: runtime_evidence(&report.auth, &report.billing),
            conditions: vec![],
        });
        if stale {
            route.billing.mark_stale();
            for cap in &mut route.capabilities {
                cap.evidence.mark_stale();
            }
        }
        snapshot.routes.push(route);
    }
    for provider in &report.providers {
        if !snapshot.providers.iter().any(|p| p.id == provider.id) {
            snapshot.providers.push(provider.clone());
        }
    }
    for original in &report.models {
        let mut model = original.clone();
        if stale {
            for cap in &mut model.capabilities {
                cap.evidence.mark_stale();
            }
            model.context_window.mark_stale();
        }
        snapshot.models.push(model);
    }
    for pool in &mut snapshot.quota_pools {
        pool.connection_ids.retain(|c| c != &report.connection_id);
        pool.route_ids.retain(|r| !old_routes.contains(r));
    }
    snapshot
        .quota_pools
        .retain(|p| !p.connection_ids.is_empty() && !p.route_ids.is_empty());
    for original in &report.quota_pools {
        let mut pool = original.clone();
        if stale {
            for window in &mut pool.windows {
                window.reading.mark_stale();
            }
        }
        snapshot.quota_pools.push(pool);
    }
}
fn usage_evidence(value: &Observation<bool>) -> Observation<String> {
    match value {
        Observation::Known {
            value,
            source,
            observed_at,
            freshness,
            verification,
            expires_at,
        } => Observation::Known {
            value: if *value {
                "Included usage allowed"
            } else {
                "Included usage blocked"
            }
            .into(),
            source: source.clone(),
            observed_at: observed_at.clone(),
            freshness: freshness.clone(),
            verification: verification.clone(),
            expires_at: expires_at.clone(),
        },
        Observation::Unknown { reason, checked_at } => Observation::Unknown {
            reason: reason.clone(),
            checked_at: checked_at.clone(),
        },
        Observation::Unavailable { reason, checked_at } => Observation::Unavailable {
            reason: reason.clone(),
            checked_at: checked_at.clone(),
        },
    }
}

fn runtime_evidence(
    auth: &Observation<AuthState>,
    billing: &Observation<BillingMode>,
) -> Observation<String> {
    let timestamp = match auth {
        Observation::Known { observed_at, .. } => Some(observed_at),
        _ => match billing {
            Observation::Known {
                value: BillingMode::Local,
                observed_at,
                ..
            } => Some(observed_at),
            _ => None,
        },
    };
    match timestamp {
        Some(observed_at) => Observation::Known {
            value: "Metadata-only adapter; native execution not implemented".into(),
            source: EvidenceSource::new(
                EvidenceKind::Derived,
                "runtime",
                "metadata-only integration",
            ),
            observed_at: observed_at.clone(),
            freshness: Freshness::Fresh,
            verification: Verification::Observed,
            expires_at: None,
        },
        _ => Observation::unknown("EXECUTION_NOT_IMPLEMENTED"),
    }
}
