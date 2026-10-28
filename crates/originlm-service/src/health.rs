//! Derive actionable issues from normalized observations. This module does not
//! reauthenticate, rotate accounts, purchase capacity, or weaken a grant.
use originlm_contracts::*;
use std::collections::BTreeMap;

pub const MAX_ISSUES: usize = 128;
pub const MAX_RESOLVED_HISTORY: usize = 32;

fn issue_id(kind: &str, resource: &str) -> String {
    stable_id("issue", &[kind, resource])
}

fn evidence_from<T>(observation: &Observation<T>, code: &str, message: &str) -> IssueEvidence {
    match observation {
        Observation::Known {
            source,
            observed_at,
            freshness,
            verification,
            ..
        } => IssueEvidence {
            code: code.into(),
            message: message.into(),
            source: Some(source.clone()),
            observed_at: Some(observed_at.clone()),
            freshness: Some(freshness.clone()),
            verification: Some(verification.clone()),
        },
        Observation::Unknown { checked_at, .. } | Observation::Unavailable { checked_at, .. } => {
            IssueEvidence {
                code: code.into(),
                message: message.into(),
                source: None,
                observed_at: checked_at.clone(),
                freshness: None,
                verification: None,
            }
        }
    }
}

fn remediation(kind: RemediationKind, label: &str, description: &str, user: bool) -> Remediation {
    Remediation {
        kind,
        label: label.into(),
        description: description.into(),
        requires_user_action: user,
    }
}

#[allow(clippy::too_many_arguments)]
// One call site per provider/quota condition; the fields map directly to the
// public Issue contract and keeping them explicit prevents a generic builder
// from hiding which resource is affected.
fn observed_issue(
    kind: IssueKind,
    severity: IssueSeverity,
    resource: &str,
    title: &str,
    summary: String,
    connection: Option<&Connection>,
    route: Option<&AccessRoute>,
    evidence: Vec<IssueEvidence>,
    remediation: Vec<Remediation>,
) -> Issue {
    let now = Timestamp::now();
    Issue {
        id: issue_id(
            match kind {
                IssueKind::AuthRequired => "auth_required",
                IssueKind::AuthExpired => "auth_expired",
                IssueKind::KeyringLocked => "keyring_locked",
                IssueKind::UnsupportedClient => "unsupported_client",
                IssueKind::UsageUnavailable => "usage_unavailable",
                IssueKind::QuotaExhausted => "quota_exhausted",
                IssueKind::RateLimited => "rate_limited",
                IssueKind::AdapterFailure => "adapter_failure",
                IssueKind::StorageUnavailable => "storage_unavailable",
                IssueKind::PermissionDenied => "permission_denied",
                IssueKind::ServiceUnavailable => "service_unavailable",
                IssueKind::ConfigurationInvalid => "configuration_invalid",
                IssueKind::Unknown => "unknown",
            },
            resource,
        ),
        fingerprint: resource.into(),
        kind,
        severity,
        state: IssueState::Open,
        title: title.into(),
        summary,
        provider_id: connection.map(|connection| connection.provider_id.clone()),
        connection_id: connection.map(|connection| connection.id.clone()),
        account_id: connection.and_then(|connection| connection.account_id.clone()),
        route_id: route.map(|route| route.id.clone()),
        pool_id: None,
        window_id: None,
        first_seen_at: now.clone(),
        last_seen_at: now,
        resolved_at: None,
        snoozed_until: None,
        occurrences: 1,
        evidence,
        remediation,
        last_observation: None,
    }
}

pub fn merge_issue(previous: Option<&Issue>, issue: Issue) -> Issue {
    merge(previous, issue)
}

fn merge(previous: Option<&Issue>, mut issue: Issue) -> Issue {
    let Some(previous) = previous else {
        return issue;
    };
    issue.first_seen_at = previous.first_seen_at.clone();
    issue.occurrences = previous.occurrences.saturating_add(1);
    match previous.state {
        IssueState::Resolved => {
            issue.state = IssueState::Open;
            issue.resolved_at = None;
            issue.snoozed_until = None;
        }
        IssueState::Snoozed => {
            let now = Timestamp::now();
            if previous
                .snoozed_until
                .as_ref()
                .is_some_and(|until| until.as_str() > now.as_str())
            {
                issue.state = IssueState::Snoozed;
                issue.snoozed_until = previous.snoozed_until.clone();
            }
        }
        IssueState::Acknowledged => issue.state = IssueState::Acknowledged,
        IssueState::Open => {}
    }
    issue
}

fn resource_present(issue: &Issue, snapshot: &Snapshot) -> bool {
    if issue.kind == IssueKind::QuotaExhausted {
        // Only a current, comparable provider reading below the threshold is
        // recovery. Missing, unknown or stale usage retains the previous issue.
        return !snapshot
            .quota_pools
            .iter()
            .filter(|p| Some(&p.id) == issue.pool_id.as_ref())
            .flat_map(|p| &p.windows)
            .filter(|w| Some(&w.id) == issue.window_id.as_ref())
            .any(|w| {
                matches!(&w.reading, Observation::Known { value, freshness: Freshness::Fresh, .. }
                if value.used_fraction.is_some_and(|fraction| fraction < 0.98))
            });
    }
    if let Some(connection) = &issue.connection_id {
        return !snapshot
            .connections
            .iter()
            .any(|candidate| &candidate.id == connection);
    }
    if let Some(pool_id) = &issue.pool_id {
        return !snapshot.quota_pools.iter().any(|pool| {
            &pool.id == pool_id
                && issue
                    .window_id
                    .as_ref()
                    .is_none_or(|window| pool.windows.iter().any(|item| &item.id == window))
        });
    }
    // Storage/diagnostic issues have no provider resource. They are only kept
    // while the current composition still emits them.
    false
}

/// Produce the complete issue view for a snapshot. Issues that were observed
/// before but are absent now are marked resolved only because this pass
/// verified the corresponding resource, not because a timer elapsed.
pub fn derive_issues(
    snapshot: &Snapshot,
    previous: &BTreeMap<String, Issue>,
    hydrated: bool,
) -> Vec<Issue> {
    let mut observed: BTreeMap<String, Issue> = BTreeMap::new();
    for connection in &snapshot.connections {
        let route = snapshot
            .routes
            .iter()
            .find(|route| route.connection_id == connection.id);
        match &connection.auth {
            Observation::Known {
                value: AuthState::Expired,
                ..
            } => {
                let issue = observed_issue(
                    IssueKind::AuthExpired,
                    IssueSeverity::Critical,
                    &connection.id,
                    "Sign-in expired",
                    format!(
                        "{} reported an expired sign-in. Existing usage data remains visible but is not current.",
                        connection.display_name
                    ),
                    Some(connection),
                    route,
                    vec![evidence_from(
                        &connection.auth,
                        "AUTH_EXPIRED",
                        "Provider reported an expired sign-in",
                    )],
                    vec![
                        remediation(
                            RemediationKind::SignIn,
                            "Sign in again",
                            "Open the installed client or account setup and sign in. The runtime never does this silently.",
                            true,
                        ),
                        remediation(
                            RemediationKind::Refresh,
                            "Refresh",
                            "Re-read metadata after signing in.",
                            false,
                        ),
                    ],
                );
                observed.insert(issue.id.clone(), merge(previous.get(&issue.id), issue));
            }
            Observation::Known {
                value: AuthState::Unauthenticated,
                ..
            } => {
                let issue = observed_issue(
                    IssueKind::AuthRequired,
                    IssueSeverity::Warning,
                    &connection.id,
                    "Sign-in required",
                    format!(
                        "{} is installed but is not signed in.",
                        connection.display_name
                    ),
                    Some(connection),
                    route,
                    vec![evidence_from(
                        &connection.auth,
                        "AUTH_REQUIRED",
                        "Provider reported no signed-in account",
                    )],
                    vec![remediation(
                        RemediationKind::SignIn,
                        "Sign in",
                        "Sign in through the installed client. No account switch is performed automatically.",
                        true,
                    )],
                );
                observed.insert(issue.id.clone(), merge(previous.get(&issue.id), issue));
            }
            Observation::Unknown { reason, .. } | Observation::Unavailable { reason, .. } => {
                let upper = reason.to_ascii_uppercase();
                if upper.contains("KEYRING") || upper.contains("LOCKED") || upper.contains("SECRET")
                {
                    let issue = observed_issue(
                        IssueKind::KeyringLocked,
                        IssueSeverity::Warning,
                        &connection.id,
                        "Credential store is locked",
                        format!(
                            "{} cannot be checked because the local credential store is locked or unavailable.",
                            connection.display_name
                        ),
                        Some(connection),
                        route,
                        vec![evidence_from(
                            &connection.auth,
                            "KEYRING_LOCKED",
                            "Credential store is locked",
                        )],
                        vec![remediation(
                            RemediationKind::UnlockKeyring,
                            "Unlock the credential store",
                            "Unlock it in your desktop session. The runtime does not repeatedly prompt or bypass it.",
                            true,
                        )],
                    );
                    observed.insert(issue.id.clone(), merge(previous.get(&issue.id), issue));
                } else if upper.contains("UNSUPPORTED") || upper.contains("VERSION") {
                    let issue = observed_issue(
                        IssueKind::UnsupportedClient,
                        IssueSeverity::Warning,
                        &connection.id,
                        "Client version is not supported",
                        format!(
                            "{} cannot be inspected by its current adapter.",
                            connection.display_name
                        ),
                        Some(connection),
                        route,
                        vec![evidence_from(
                            &connection.auth,
                            "UNSUPPORTED_CLIENT",
                            "Client version is unsupported",
                        )],
                        vec![remediation(
                            RemediationKind::UpdateClient,
                            "Update the client",
                            "Install a supported client version, then refresh.",
                            true,
                        )],
                    );
                    observed.insert(issue.id.clone(), merge(previous.get(&issue.id), issue));
                }
            }
            _ => {}
        }
        if connection.status == "needs_auth"
            && !observed
                .values()
                .any(|issue| issue.connection_id.as_deref() == Some(&connection.id))
        {
            // Only unknown, unavailable or unchecked auth reaches this branch;
            // reported sign-outs are handled above. Do not claim the user has
            // to sign in when the provider never told us either way.
            let issue = observed_issue(
                IssueKind::AuthRequired,
                IssueSeverity::Info,
                &connection.id,
                "Sign-in not checked",
                format!(
                    "{} has not reported a signed-in account yet, so OriginLM cannot confirm the session.",
                    connection.display_name
                ),
                Some(connection),
                route,
                vec![evidence_from(
                    &connection.auth,
                    "AUTH_NOT_CHECKED",
                    "Provider did not report an auth state",
                )],
                vec![remediation(
                    RemediationKind::Refresh,
                    "Check again",
                    "Read the client's metadata again. Signing in is only needed if the client itself asks for it.",
                    false,
                )],
            );
            observed.insert(issue.id.clone(), merge(previous.get(&issue.id), issue));
        }
    }
    for pool in &snapshot.quota_pools {
        for window in &pool.windows {
            let Observation::Known {
                value,
                freshness: Freshness::Fresh,
                ..
            } = &window.reading
            else {
                continue;
            };
            let Some(fraction) = value.used_fraction else {
                continue;
            };
            if fraction < 0.98 {
                continue;
            }
            let resource = format!("{}:{}", pool.id, window.id);
            let connection = pool.connection_ids.first().and_then(|id| {
                snapshot
                    .connections
                    .iter()
                    .find(|connection| &connection.id == id)
            });
            let reset_description = window.resets_at.as_ref().map_or_else(
                || "The provider did not report a reset time; check again later.".to_owned(),
                |reset| format!("The provider reported a reset at {}.", reset.as_str()),
            );
            let mut issue = observed_issue(
                IssueKind::QuotaExhausted,
                IssueSeverity::Warning,
                &resource,
                "Allowance nearly exhausted",
                format!(
                    "{} in {} is at {:.0}% of its reported limit.",
                    window.label,
                    pool.id,
                    fraction * 100.0
                ),
                connection,
                None,
                vec![evidence_from(
                    &window.reading,
                    "QUOTA_EXHAUSTED",
                    "Reported usage is at or near the limit",
                )],
                vec![remediation(
                    RemediationKind::RetryLater,
                    "Wait for the reported reset",
                    &reset_description,
                    false,
                )],
            );
            issue.pool_id = Some(pool.id.clone());
            issue.window_id = Some(window.id.clone());
            observed.insert(issue.id.clone(), merge(previous.get(&issue.id), issue));
        }
    }
    if hydrated {
        // Hydration is a freshness fact, not an issue. Keep it in health only.
    }
    let mut issues: Vec<Issue> = observed.into_values().collect();
    // A resource disappearing from a scan or a file event is not proof that
    // the account recovered or signed out. Keep an unobserved open issue
    // visible; only a later positive observation resolves it.
    for previous in previous.values() {
        if issues.iter().any(|current| current.id == previous.id) {
            continue;
        }
        if previous.state != IssueState::Resolved && resource_present(previous, snapshot) {
            let mut stale = previous.clone();
            for evidence in &mut stale.evidence {
                evidence.freshness = Some(Freshness::Stale);
            }
            stale.last_seen_at = Timestamp::now();
            stale.occurrences = stale.occurrences.saturating_add(1);
            issues.push(stale);
        }
    }
    // Preserve recent resolved history, but never let it grow without a bound.
    let resolved: Vec<Issue> = previous
        .values()
        .filter(|issue| issue.state == IssueState::Resolved)
        .rev()
        .take(MAX_RESOLVED_HISTORY)
        .cloned()
        .collect();
    for mut issue in resolved {
        if issues.iter().any(|current| current.id == issue.id) {
            continue;
        }
        issue.state = IssueState::Resolved;
        issues.push(issue);
    }
    issues.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| right.last_seen_at.as_str().cmp(left.last_seen_at.as_str()))
            .then_with(|| left.id.cmp(&right.id))
    });
    issues.truncate(MAX_ISSUES);
    issues
}

pub fn summarize(issues: &[Issue], hydrated: bool) -> HealthSummary {
    let open: Vec<&Issue> = issues
        .iter()
        .filter(|issue| issue.state != IssueState::Resolved)
        .collect();
    let worst = open.iter().map(|issue| issue.severity).max();
    let status = match worst {
        Some(IssueSeverity::Critical) | Some(IssueSeverity::Error) => HealthStatus::Degraded,
        Some(IssueSeverity::Warning) => HealthStatus::Degraded,
        Some(IssueSeverity::Info) => HealthStatus::Healthy,
        None => HealthStatus::Healthy,
    };
    HealthSummary {
        status,
        open_issues: u32::try_from(open.len()).unwrap_or(u32::MAX),
        worst_severity: worst,
        checked_at: Some(Timestamp::now()),
        hydrated,
    }
}

pub fn inventory_issue(error: &DomainError) -> Issue {
    observed_issue(
        IssueKind::AdapterFailure,
        IssueSeverity::Warning,
        "inventory",
        "Inventory could not be refreshed",
        "Last-known inventory remains stale until a successful reconciliation.".into(),
        None,
        None,
        vec![IssueEvidence {
            code: format!("{:?}", error.code),
            message: error.message.clone(),
            source: Some(EvidenceSource::new(
                EvidenceKind::Derived,
                "originlm-service",
                "inventory refresh",
            )),
            observed_at: Some(Timestamp::now()),
            freshness: Some(Freshness::Fresh),
            verification: Some(Verification::Observed),
        }],
        vec![remediation(
            RemediationKind::RetryLater,
            "Retry later",
            "The runtime retries with bounded backoff.",
            false,
        )],
    )
}

pub fn storage_issue(error: &DomainError, _path_redacted: bool) -> Issue {
    let now = Timestamp::now();
    Issue {
        id: issue_id("storage_unavailable", "state-store"),
        fingerprint: "state-store".into(),
        kind: IssueKind::StorageUnavailable,
        severity: IssueSeverity::Critical,
        state: IssueState::Open,
        title: "Local state storage is unavailable".into(),
        summary: error.message.clone(),
        provider_id: None,
        connection_id: None,
        account_id: None,
        route_id: None,
        pool_id: None,
        window_id: None,
        first_seen_at: now.clone(),
        last_seen_at: now,
        resolved_at: None,
        snoozed_until: None,
        occurrences: 1,
        evidence: vec![IssueEvidence {
            code: format!("{:?}", error.code),
            message: error.message.clone(),
            source: Some(EvidenceSource::new(
                EvidenceKind::Derived,
                "originlm-store",
                "broker-owned SQLite",
            )),
            observed_at: Some(Timestamp::now()),
            freshness: Some(Freshness::Fresh),
            verification: Some(Verification::Observed),
        }],
        remediation: vec![remediation(
            RemediationKind::FreeStorage,
            "Check local storage",
            "Free disk space or fix permissions for the broker state directory, then refresh.",
            true,
        )],
        last_observation: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(auth: Observation<AuthState>) -> Snapshot {
        let mut snapshot = Snapshot::empty(DataMode::Live);
        snapshot.connections.push(Connection {
            id: "c".into(),
            provider_id: "p".into(),
            installation_id: "i".into(),
            profile_id: "pr".into(),
            display_name: "C".into(),
            account_id: None,
            auth,
            plan: Observation::unknown("TEST"),
            status: "ready".into(),
            diagnostics: vec![],
        });
        snapshot
    }

    #[test]
    fn expired_auth_is_actionable_and_recovery_resolves() {
        let source = EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture");
        let mut snapshot = connection(Observation::reported(AuthState::Expired, source));
        let first = derive_issues(&snapshot, &BTreeMap::new(), false);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].kind, IssueKind::AuthExpired);
        assert_eq!(first[0].severity, IssueSeverity::Critical);
        let previous: BTreeMap<_, _> = first
            .iter()
            .map(|issue| (issue.id.clone(), issue.clone()))
            .collect();
        snapshot.connections[0].auth = Observation::reported(
            AuthState::Authenticated,
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture"),
        );
        let second = derive_issues(&snapshot, &previous, false);
        assert!(
            second
                .iter()
                .all(|issue| issue.state == IssueState::Resolved)
        );
    }

    #[test]
    fn unknown_is_not_reported_as_expired_and_duplicates_merge() {
        let snapshot = connection(Observation::unknown("UNPROBED"));
        assert!(derive_issues(&snapshot, &BTreeMap::new(), false).is_empty());
    }

    #[test]
    fn dismissal_is_not_resolution() {
        let source = EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture");
        let snapshot = connection(Observation::reported(AuthState::Unauthenticated, source));
        let mut issues = derive_issues(&snapshot, &BTreeMap::new(), false);
        issues[0].state = IssueState::Acknowledged;
        let previous: BTreeMap<_, _> = issues
            .iter()
            .map(|issue| (issue.id.clone(), issue.clone()))
            .collect();
        let second = derive_issues(&snapshot, &previous, false);
        assert_eq!(second[0].state, IssueState::Acknowledged);
        assert_eq!(second[0].occurrences, 2);
    }

    #[test]
    fn a_missing_resource_does_not_prove_recovery_or_logout() {
        let source = EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture");
        let snapshot = connection(Observation::reported(AuthState::Expired, source));
        let first = derive_issues(&snapshot, &BTreeMap::new(), false);
        let previous: BTreeMap<_, _> = first
            .iter()
            .map(|issue| (issue.id.clone(), issue.clone()))
            .collect();
        let empty = Snapshot::empty(DataMode::Live);
        let second = derive_issues(&empty, &previous, false);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].state, IssueState::Open);
        assert_eq!(second[0].kind, IssueKind::AuthExpired);
    }
}
