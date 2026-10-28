use crate::ClientPolicy;
use originlm_contracts::*;
use std::collections::BTreeSet;

pub fn project_snapshot(source: &Snapshot, policy: &ClientPolicy) -> Snapshot {
    let mut snapshot = source.clone();
    snapshot.connections.retain(|c| policy.allows(&c.id));
    let visible_connections: BTreeSet<_> =
        snapshot.connections.iter().map(|c| c.id.clone()).collect();
    snapshot.issues.retain(|issue| {
        issue
            .connection_id
            .as_ref()
            .is_none_or(|id| visible_connections.contains(id))
    });
    let open: Vec<&Issue> = snapshot
        .issues
        .iter()
        .filter(|issue| issue.state != IssueState::Resolved)
        .collect();
    snapshot.health.open_issues = u32::try_from(open.len()).unwrap_or(u32::MAX);
    snapshot.health.worst_severity = open.iter().map(|issue| issue.severity).max();
    snapshot.health.status = match snapshot.health.worst_severity {
        Some(IssueSeverity::Critical)
        | Some(IssueSeverity::Error)
        | Some(IssueSeverity::Warning) => HealthStatus::Degraded,
        Some(IssueSeverity::Info) | None => HealthStatus::Healthy,
    };
    let connections: BTreeSet<_> = snapshot.connections.iter().map(|c| c.id.clone()).collect();
    let profiles: BTreeSet<_> = snapshot
        .connections
        .iter()
        .map(|c| c.profile_id.clone())
        .collect();
    let installations: BTreeSet<_> = snapshot
        .connections
        .iter()
        .map(|c| c.installation_id.clone())
        .collect();
    let accounts: BTreeSet<_> = snapshot
        .connections
        .iter()
        .filter_map(|c| c.account_id.clone())
        .collect();
    let mut providers: BTreeSet<_> = snapshot
        .connections
        .iter()
        .map(|c| c.provider_id.clone())
        .collect();
    snapshot
        .routes
        .retain(|r| connections.contains(&r.connection_id));
    let routes: BTreeSet<_> = snapshot.routes.iter().map(|r| r.id.clone()).collect();
    snapshot.models.retain(|m| routes.contains(&m.route_id));
    snapshot.profiles.retain(|p| profiles.contains(&p.id));
    snapshot
        .installations
        .retain(|i| installations.contains(&i.id));
    snapshot.accounts.retain(|a| accounts.contains(&a.id));
    providers.extend(snapshot.routes.iter().map(|r| r.provider_id.clone()));
    snapshot.providers.retain(|p| providers.contains(&p.id));
    for pool in &mut snapshot.quota_pools {
        pool.connection_ids.retain(|id| connections.contains(id));
        pool.route_ids.retain(|id| routes.contains(id));
        if pool
            .account_id
            .as_ref()
            .is_some_and(|id| !accounts.contains(id))
        {
            pool.account_id = None;
        }
    }
    snapshot
        .quota_pools
        .retain(|p| !p.connection_ids.is_empty() && !p.route_ids.is_empty());
    if !policy.reveal_identity {
        for account in &mut snapshot.accounts {
            account.display_label = Observation::unknown("IDENTITY_PERMISSION_REQUIRED");
            account.identity_source = Observation::unknown("IDENTITY_PERMISSION_REQUIRED");
        }
    }
    // Global diagnostic strings may reference a provider invisible to this client.
    snapshot.diagnostics.clear();
    snapshot.cursor = source.cursor.clone();
    snapshot.revision = stable_id(
        "projection",
        &[
            &source.revision,
            &serde_json::to_string(&(
                &snapshot.connections,
                &snapshot.routes,
                &snapshot.models,
                &snapshot.quota_pools,
                &snapshot.issues,
                &snapshot.health,
            ))
            .unwrap_or_default(),
        ],
    );
    snapshot
}
