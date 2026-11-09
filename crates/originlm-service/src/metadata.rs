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
    /// The metadata host declares its adapters' whole-operation budget. The
    /// service must not cancel a valid native handshake at a shorter generic
    /// deadline. Individual adapters retain their own tighter phase deadlines.
    pub fn with_metadata_timeout(mut self, timeout: Duration) -> Self {
        self.metadata_timeout = timeout.clamp(Duration::from_secs(1), Duration::from_secs(300));
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
            self.metadata_timeout,
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
                    DomainError::new(error.code.clone(), metadata_failure_message(&error.code));
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
        params::allow(p, &["connectionId", "force"])?;
        let force = params::boolean(p, "force")?;
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
        if force {
            // Explicit refresh invalidates success and failure caches under the
            // same connection lease as every other probe.
            self.cache.invalidate_metadata(&[id.into()])?;
        }
        let target = self.cache.probe_input(id)?;
        if let Some(error) = self.cache.metadata_failure(id)? {
            return Err(error);
        }
        if !force && self.cache.has_fresh_metadata(id)? {
            return Ok(json!({"status":"cached","connectionId":id}));
        }
        let result = tokio::time::timeout(
            self.metadata_timeout,
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
                let mut safe =
                    DomainError::new(error.code.clone(), metadata_failure_message(&error.code));
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
            self.metadata_timeout,
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
                    DomainError::new(error.code.clone(), metadata_failure_message(&error.code));
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
// Only fixed, actionable reasons cross the public boundary. Adapter messages
// may contain private paths, URLs or tokens.
fn metadata_failure_message(code: &ErrorCode) -> &'static str {
    match code {
        ErrorCode::Timeout => "Service unreachable: metadata read timed out",
        ErrorCode::ProviderUnavailable => {
            "Service unreachable: local provider service did not answer"
        }
        ErrorCode::CapabilityUnsupported => {
            "Discovery only: OriginLM has no metadata adapter for this provider"
        }
        ErrorCode::RuntimeMissing => "Provider executable or required runtime is missing",
        ErrorCode::IntegrationRestricted => {
            "Provider launch is restricted: profile or browser-blocking helper is unavailable"
        }
        ErrorCode::ProtocolIncompatible => "Provider returned an incompatible metadata response",
        ErrorCode::LocalAccessDenied => "Provider metadata access was denied",
        _ => "Client metadata is unavailable",
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

/// Prefer a native tier. When the provider only publishes route billing,
/// surface that category at account level without inventing a tier/entitlement.
fn account_plan(snapshot: &Snapshot, report: &ConnectionMetadata) -> Observation<Plan> {
    if report.plan.fresh_value().is_some() {
        return report.plan.clone();
    }
    let primary = snapshot
        .connections
        .iter()
        .find(|c| c.id == report.connection_id)
        .map(|c| c.provider_id.as_str())
        .unwrap_or("");
    let best = std::iter::once((primary, &report.billing))
        .chain(
            report
                .additional_routes
                .iter()
                .filter(|r| r.connection_id == report.connection_id)
                .map(|r| (r.provider_id.as_str(), &r.billing)),
        )
        .filter_map(|(provider, billing)| match billing {
            Observation::Known {
                value, freshness, ..
            } => {
                let (priority, category) = match value {
                    BillingMode::Subscription => (5, "Subscription"),
                    BillingMode::Mixed => (4, "Mixed billing"),
                    BillingMode::PrepaidCredits => (3, "Prepaid credits"),
                    BillingMode::ApiMetered => (2, "Metered API"),
                    BillingMode::Local => (1, "Local"),
                    BillingMode::Unknown => return None,
                };
                Some((
                    (u8::from(*freshness == Freshness::Fresh), priority),
                    provider,
                    category,
                    billing,
                ))
            }
            _ => None,
        })
        .max_by_key(|(priority, ..)| *priority);
    let Some((
        _,
        provider,
        category,
        Observation::Known {
            source,
            observed_at,
            freshness,
            expires_at,
            ..
        },
    )) = best
    else {
        return report.plan.clone();
    };
    if *freshness == Freshness::Stale && report.plan.value().is_some() {
        return report.plan.clone();
    }
    let name = report
        .providers
        .iter()
        .chain(snapshot.providers.iter())
        .find(|p| p.id == provider)
        .map(|p| p.name.as_str())
        .unwrap_or(provider);
    Observation::Known {
        value: Plan {
            native_id: format!("route-billing:{provider}:{category}"),
            label: format!("{name} {category}"),
        },
        source: source.clone(),
        observed_at: observed_at.clone(),
        freshness: freshness.clone(),
        verification: Verification::Inferred,
        expires_at: expires_at.clone(),
    }
}

pub(crate) fn merge_report(snapshot: &mut Snapshot, report: &ConnectionMetadata, stale: bool) {
    let plan = account_plan(snapshot, report);
    let route_plan = plan
        .value()
        .is_some_and(|p| p.native_id.starts_with("route-billing:"));
    let Some(connection) = snapshot
        .connections
        .iter_mut()
        .find(|c| c.id == report.connection_id)
    else {
        return;
    };
    connection.auth = report.auth.clone();
    connection.plan = plan;
    connection.account_id = report.account.as_ref().map(|a| a.id.clone());
    connection.diagnostics = report.diagnostics.clone();
    if route_plan
        && !connection
            .diagnostics
            .iter()
            .any(|d| d == "PLAN_FROM_ROUTE_BILLING")
    {
        connection
            .diagnostics
            .push("PLAN_FROM_ROUTE_BILLING".into());
    }
    if connection.plan.value().is_none()
        && !connection
            .diagnostics
            .iter()
            .any(|d| d == "PLAN_NOT_REPORTED_BY_PROVIDER")
    {
        connection
            .diagnostics
            .push("PLAN_NOT_REPORTED_BY_PROVIDER".into());
    }
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
    // A metadata refresh can succeed while the provider declines to return
    // usage (for example an expired sign-in that still lists models). In that
    // case keep the last allowance windows and mark them stale instead of
    // deleting the only evidence the user has. When usage is present, replace
    // the connection's pools as before.
    let usage_reported = !report.quota_pools.is_empty();
    // Routes this refresh keeps or re-adds. A stale pool keeps those ids; only
    // routes that no longer exist fold onto the primary route.
    let live_routes: BTreeSet<String> = snapshot
        .routes
        .iter()
        .filter(|route| route.connection_id == report.connection_id)
        .map(|route| route.id.clone())
        .collect();
    for pool in &mut snapshot.quota_pools {
        if !usage_reported {
            if pool.connection_ids.contains(&report.connection_id) {
                for window in &mut pool.windows {
                    window.reading.mark_stale();
                }
                pool.route_ids = pool
                    .route_ids
                    .iter()
                    .map(|route| {
                        if live_routes.contains(route) {
                            route.clone()
                        } else if old_routes.contains(route) {
                            report.route_id.clone()
                        } else {
                            route.clone()
                        }
                    })
                    .collect();
                pool.route_ids.sort();
                pool.route_ids.dedup();
            }
            continue;
        }
        pool.connection_ids.retain(|c| c != &report.connection_id);
        pool.route_ids.retain(|r| !old_routes.contains(r));
    }
    if usage_reported {
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

#[cfg(test)]
mod tests {
    use super::merge_report;
    use originlm_contracts::{
        AccessRoute, AuthState, BillingMode, Connection, ConnectionMetadata, DataMode,
        EvidenceKind, EvidenceSource, Freshness, Observation, Plan, QuotaPool, Snapshot,
        UsageInput, UsageReading, UsageWindow,
    };

    fn source() -> EvidenceSource {
        EvidenceSource::new(EvidenceKind::Synthetic, "test", "metadata")
    }

    #[test]
    fn a_refresh_without_usage_keeps_the_last_allowance_stale() {
        let mut snapshot = Snapshot::empty(DataMode::Live);
        snapshot.connections.push(Connection {
            id: "connection-1".into(),
            provider_id: "claude_code".into(),
            installation_id: "installation-1".into(),
            profile_id: "profile-1".into(),
            display_name: "Claude Code".into(),
            account_id: None,
            auth: Observation::unknown("TEST"),
            plan: Observation::unknown("TEST"),
            status: "detected".into(),
            diagnostics: vec![],
        });
        snapshot.routes.push(AccessRoute {
            id: "route-1".into(),
            connection_id: "connection-1".into(),
            provider_id: "claude_code".into(),
            display_name: "Claude Code".into(),
            billing: Observation::reported(BillingMode::Subscription, source()),
            capabilities: vec![],
            catalog_revision: "1".into(),
        });
        snapshot.quota_pools.push(QuotaPool {
            id: "pool-1".into(),
            account_id: None,
            connection_ids: vec!["connection-1".into()],
            route_ids: vec!["route-1".into()],
            sharing: "private".into(),
            windows: vec![UsageWindow {
                id: "five_hour".into(),
                label: "Five hour".into(),
                reading: Observation::reported(
                    UsageReading::try_from(UsageInput {
                        unit: "percent".into(),
                        used: "94".to_owned().try_into().unwrap(),
                        limit: Some("100".to_owned().try_into().unwrap()),
                    })
                    .unwrap(),
                    source(),
                ),
                resets_at: None,
                duration_seconds: None,
            }],
        });
        let report = ConnectionMetadata {
            connection_id: "connection-1".into(),
            route_id: "route-1".into(),
            version: Observation::unknown("TEST"),
            auth: Observation::reported(AuthState::Authenticated, source()),
            plan: Observation::reported(
                Plan {
                    native_id: "pro".into(),
                    label: "Pro".into(),
                },
                source(),
            ),
            billing: Observation::reported(BillingMode::Subscription, source()),
            account: None,
            models: vec![],
            additional_routes: vec![],
            providers: vec![],
            quota_pools: vec![],
            included_usage_allowed: Observation::unknown("NOT_REPORTED"),
            diagnostics: vec![],
        };

        merge_report(&mut snapshot, &report, false);

        assert_eq!(snapshot.quota_pools.len(), 1);
        assert!(matches!(
            snapshot.quota_pools[0].windows[0].reading,
            Observation::Known {
                freshness: Freshness::Stale,
                ..
            }
        ));
    }

    fn route(id: &str, provider: &str) -> AccessRoute {
        AccessRoute {
            id: id.into(),
            connection_id: "connection-1".into(),
            provider_id: provider.into(),
            display_name: id.into(),
            billing: Observation::reported(BillingMode::Subscription, source()),
            capabilities: vec![],
            catalog_revision: "1".into(),
        }
    }

    #[test]
    fn a_refresh_without_usage_keeps_allowances_on_routes_it_still_reports() {
        let mut snapshot = Snapshot::empty(DataMode::Live);
        snapshot.connections.push(Connection {
            id: "connection-1".into(),
            provider_id: "opencode".into(),
            installation_id: "installation-1".into(),
            profile_id: "profile-1".into(),
            display_name: "OpenCode".into(),
            account_id: None,
            auth: Observation::unknown("TEST"),
            plan: Observation::unknown("TEST"),
            status: "detected".into(),
            diagnostics: vec![],
        });
        snapshot.routes.push(route("route-primary", "opencode"));
        snapshot.routes.push(route("route-go", "opencode-go"));
        snapshot.quota_pools.push(QuotaPool {
            id: "pool-go".into(),
            account_id: None,
            connection_ids: vec!["connection-1".into()],
            route_ids: vec!["route-go".into()],
            sharing: "private".into(),
            windows: vec![UsageWindow {
                id: "weekly".into(),
                label: "Weekly".into(),
                reading: Observation::unknown("TEST"),
                resets_at: None,
                duration_seconds: None,
            }],
        });
        let report = ConnectionMetadata {
            connection_id: "connection-1".into(),
            route_id: "route-primary".into(),
            version: Observation::unknown("TEST"),
            auth: Observation::reported(AuthState::Authenticated, source()),
            plan: Observation::unknown("TEST"),
            billing: Observation::reported(BillingMode::Subscription, source()),
            account: None,
            models: vec![],
            additional_routes: vec![route("route-go", "opencode-go")],
            providers: vec![],
            quota_pools: vec![],
            included_usage_allowed: Observation::unknown("NOT_REPORTED"),
            diagnostics: vec![],
        };

        merge_report(&mut snapshot, &report, false);

        assert_eq!(
            snapshot.quota_pools[0].route_ids,
            vec!["route-go".to_string()],
            "a route this refresh re-adds keeps its allowance"
        );
    }
}
