//! Presentation-ready projections of broker data. Nothing here invents a
//! value: unknown, unavailable and unobserved stay distinct.

use crate::history::Sample;
use originlm_contracts::{
    AccessRoute, BillingMode, Connection, Freshness, Issue, IssueSeverity, Model, Observation,
    Snapshot, UsageReading,
};

#[derive(Clone)]
pub struct QuotaRow {
    pub pool_id: String,
    pub provider_id: String,
    pub window_id: String,
    pub pool_label: String,
    pub window_label: String,
    pub remaining: Option<f32>,
    pub resets_at: Option<String>,
    pub fresh: bool,
}

pub fn quota_rows(snapshot: &Snapshot) -> Vec<QuotaRow> {
    let mut rows = Vec::new();
    for pool in &snapshot.quota_pools {
        let first_connection = pool
            .connection_ids
            .iter()
            .find_map(|id| snapshot.connections.iter().find(|c| &c.id == id));
        let provider_id = first_connection
            .map(|connection| connection.provider_id.clone())
            .unwrap_or_default();
        let provider = first_connection
            .map(|connection| provider_name(snapshot, &connection.provider_id))
            .unwrap_or_else(|| "Allowance".to_string());
        for window in &pool.windows {
            let (remaining, fresh) = match &window.reading {
                Observation::Known {
                    value, freshness, ..
                } => (
                    remaining_fraction(value),
                    matches!(freshness, Freshness::Fresh),
                ),
                _ => (None, false),
            };
            rows.push(QuotaRow {
                pool_id: pool.id.clone(),
                provider_id: provider_id.clone(),
                window_id: window.id.clone(),
                pool_label: provider.clone(),
                window_label: window.label.clone(),
                remaining,
                resets_at: window.resets_at.as_ref().map(|t| t.as_str().to_string()),
                fresh,
            });
        }
    }
    rows
}

pub fn remaining_fraction(reading: &UsageReading) -> Option<f32> {
    if let Some(fraction) = reading.used_fraction {
        return Some((1.0 - fraction).clamp(0.0, 1.0) as f32);
    }
    match (&reading.remaining, &reading.limit) {
        (Some(remaining), Some(limit)) => {
            let remaining = remaining.as_str().parse::<f64>().ok()?;
            let limit = limit.as_str().parse::<f64>().ok()?;
            if limit > 0.0 {
                Some((remaining / limit).clamp(0.0, 1.0) as f32)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// A readable pool+window label without repeating the provider name when the
/// provider already prefixes the window label.
pub fn quota_label(row: &QuotaRow) -> String {
    let pool = row.pool_label.to_lowercase();
    let window = row.window_label.to_lowercase();
    if window.contains(&pool) || pool.contains(&window) {
        row.window_label.clone()
    } else {
        let window_label = row.window_label.replace('_', " ");
        format!(
            "{} {}",
            row.pool_label,
            crate::human::title_case(&window_label)
        )
    }
}

pub fn provider_name(snapshot: &Snapshot, provider_id: &str) -> String {
    snapshot
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| provider_id.to_string())
}

/// Provider-prefixed connection title without repeating the provider when the
/// broker already includes it in the display name.
pub fn connection_title(snapshot: &Snapshot, connection: &Connection) -> String {
    let provider = provider_name(snapshot, &connection.provider_id);
    if connection
        .display_name
        .to_lowercase()
        .starts_with(&provider.to_lowercase())
    {
        connection.display_name.clone()
    } else {
        format!("{}, {}", provider, connection.display_name)
    }
}

pub fn plan_label(connection: &Connection) -> String {
    match &connection.plan {
        Observation::Known { value, .. } => value.label.clone(),
        Observation::Unknown { .. } => "Plan unknown".into(),
        Observation::Unavailable { .. } => "plan unavailable".into(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    Verified,
    Cached,
    Required,
    Unknown,
}

#[derive(Clone, Copy)]
pub struct AuthView {
    pub label: &'static str,
    pub kind: AuthKind,
    pub stale: bool,
}

pub fn auth_view(connection: &Connection) -> AuthView {
    let stale = matches!(
        &connection.auth,
        Observation::Known {
            freshness: Freshness::Stale,
            ..
        }
    );
    let kind = match &connection.auth {
        Observation::Known {
            value: originlm_contracts::AuthState::Authenticated,
            ..
        } => {
            if stale {
                AuthKind::Cached
            } else {
                AuthKind::Verified
            }
        }
        Observation::Known {
            value:
                originlm_contracts::AuthState::Unauthenticated | originlm_contracts::AuthState::Expired,
            ..
        } => AuthKind::Required,
        Observation::Known {
            value: originlm_contracts::AuthState::Unchecked,
            ..
        }
        | Observation::Unknown { .. } => {
            if connection.account_id.is_some() {
                AuthKind::Cached
            } else {
                AuthKind::Unknown
            }
        }
        Observation::Unavailable { .. } => AuthKind::Unknown,
    };
    AuthView {
        label: match kind {
            AuthKind::Verified => "Authenticated",
            AuthKind::Cached => "Signed in, cache only",
            AuthKind::Required => "Sign-in required",
            AuthKind::Unknown => "Not checked",
        },
        kind,
        stale,
    }
}

/// The OpenCode plan a model comes from. OpenCode serves three catalogs on
/// one connection: the free models in Zen, the paid Zen catalog, and Go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelPlan {
    Free,
    Zen,
    Go,
    Local,
}

impl ModelPlan {
    pub fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Zen => "Zen",
            Self::Go => "Go",
            Self::Local => "Local",
        }
    }
}

pub fn model_plan(snapshot: &Snapshot, model: &Model) -> Option<ModelPlan> {
    let route = route_for_model(snapshot, model)?;
    Some(match route.provider_id.as_str() {
        "opencode-go" => ModelPlan::Go,
        "opencode" => {
            // OpenCode's free models are named as such and carry a `-free`
            // native id. Everything else on the Zen route is pay-as-you-go.
            let name = model.name.to_ascii_lowercase();
            let native = model.native_id.to_ascii_lowercase();
            if native.ends_with("-free") || name.contains("free") {
                ModelPlan::Free
            } else {
                ModelPlan::Zen
            }
        }
        "ollama" => ModelPlan::Local,
        _ => return None,
    })
}

pub fn route_for_model<'a>(snapshot: &'a Snapshot, model: &Model) -> Option<&'a AccessRoute> {
    snapshot.routes.iter().find(|r| r.id == model.route_id)
}

pub fn connection_for_route<'a>(
    snapshot: &'a Snapshot,
    route: &AccessRoute,
) -> Option<&'a Connection> {
    snapshot
        .connections
        .iter()
        .find(|c| c.id == route.connection_id)
}

pub fn billing(route: &AccessRoute) -> (String, bool) {
    match &route.billing {
        Observation::Known {
            value,
            freshness,
            verification,
            ..
        } => {
            let mut label = match value {
                BillingMode::Subscription => "subscription".to_string(),
                BillingMode::ApiMetered => "metered API".to_string(),
                BillingMode::PrepaidCredits => "prepaid credits".to_string(),
                BillingMode::Local => "local".to_string(),
                BillingMode::Mixed => "mixed".to_string(),
                BillingMode::Unknown => "unknown".to_string(),
            };
            if matches!(verification, originlm_contracts::Verification::Inferred) {
                label.push_str(" (inferred)");
            }
            (label, matches!(freshness, Freshness::Fresh))
        }
        Observation::Unknown { .. } => ("unknown".into(), false),
        Observation::Unavailable { .. } => ("unavailable".into(), false),
    }
}

pub fn severity_label(severity: IssueSeverity) -> &'static str {
    match severity {
        IssueSeverity::Info => "info",
        IssueSeverity::Warning => "warning",
        IssueSeverity::Error => "error",
        IssueSeverity::Critical => "critical",
    }
}

pub fn open_issues(snapshot: &Snapshot) -> Vec<&Issue> {
    snapshot
        .issues
        .iter()
        .filter(|issue| !matches!(issue.state, originlm_contracts::IssueState::Resolved))
        .collect()
}

pub fn series_points(history: &[Sample], pool: &str, window: &str, since: i64) -> Vec<ChartPoint> {
    let matching: Vec<&Sample> = history
        .iter()
        .filter(|sample| sample.pool == pool && sample.window == window && sample.t >= since)
        .collect();
    let span = match (matching.first(), matching.last()) {
        (Some(first), Some(last)) => (last.t - first.t).max(0),
        _ => 0,
    };
    let format = if span <= 3_600 { "%H:%M:%S" } else { "%H:%M" };
    matching
        .into_iter()
        .filter_map(|sample| {
            // Percent left, the same measure the meters show everywhere else.
            let left = sample.remaining?.clamp(0.0, 1.0) * 100.0;
            Some(ChartPoint {
                label: chrono::DateTime::from_timestamp(sample.t, 0)
                    .map(|stamp| stamp.format(format).to_string())
                    .unwrap_or_else(|| "—".into()),
                value: (left * 10.0).round() / 10.0,
            })
        })
        .collect()
}

#[derive(Clone)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

impl ChartPoint {
    pub fn value(&self) -> f64 {
        self.value
    }
}

pub fn capability_label(id: &str) -> &'static str {
    match id {
        "generation.text" => "text",
        "generation.stream" => "streaming",
        "generation.tools" => "tools",
        "input.image" => "images",
        "input.audio" => "audio",
        "output.reasoning" => "reasoning",
        "context.long" => "long context",
        "input.text" => "text input",
        "tools" | "tools.call" => "tools",
        "structured.output" => "structured output",
        _ => "",
    }
}

pub fn chart_variation(points: &[ChartPoint]) -> f64 {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in points {
        min = min.min(point.value);
        max = max.max(point.value);
    }
    if min.is_finite() && max.is_finite() {
        max - min
    } else {
        0.0
    }
}

/// Passive discovery disambiguates duplicate executables and profile roots
/// with indices. Those indices are useful in diagnostics, not in a row a
/// person reads. Drop only the internal segments and keep meaningful ones.
fn clean_display_name(name: &str, provider: &str) -> String {
    let kept: Vec<&str> = name
        .split(" · ")
        .filter(|part| {
            let lower = part.trim().to_ascii_lowercase();
            !(lower == "default profile"
                || lower.starts_with("additional profile")
                || lower.starts_with("installation "))
        })
        .collect();
    let joined = kept.join(" · ");
    if joined.trim().is_empty() {
        provider.to_string()
    } else {
        joined
    }
}

fn clean_profile_label(label: &str) -> String {
    let kept: Vec<&str> = label
        .split(" · ")
        .filter(|part| {
            !part
                .trim()
                .to_ascii_lowercase()
                .starts_with("installation ")
        })
        .collect();
    let joined = kept.join(" · ");
    let lower = joined.to_ascii_lowercase();
    if lower == "default profile" {
        "Default".into()
    } else if lower.starts_with("additional profile") || joined.trim().is_empty() {
        "Profile".into()
    } else {
        joined
    }
}

fn short_account(id: &str) -> String {
    let tail: String = id
        .rsplit('_')
        .next()
        .unwrap_or(id)
        .chars()
        .take(6)
        .collect();
    format!("Account {tail}")
}

/// Collapse passive-discovery duplicates for the desktop view. Multiple
/// command shims, profile roots and bundled copies of one client are one
/// user-facing account unless the service reports distinct account ids.
/// Distinct non-null account ids are preserved.
pub fn canonical_snapshot(mut snapshot: Snapshot) -> Snapshot {
    use std::collections::{BTreeMap, BTreeSet};

    fn rank(connection: &Connection) -> (u8, u8, u8, u8) {
        let auth = match &connection.auth {
            Observation::Known {
                value: originlm_contracts::AuthState::Authenticated,
                ..
            } => 3,
            Observation::Known {
                value:
                    originlm_contracts::AuthState::Unauthenticated
                    | originlm_contracts::AuthState::Expired,
                ..
            } => 1,
            Observation::Unknown { .. } | Observation::Unavailable { .. } => 0,
            Observation::Known { .. } => 2,
        };
        let status = match connection.status.as_str() {
            "metadata_ready" => 3,
            "detected" => 2,
            "configuration_only" => 1,
            _ => 0,
        };
        let account = u8::from(connection.account_id.is_some());
        let plan = u8::from(connection.plan.value().is_some());
        (auth, status, account, plan)
    }

    let mut groups: BTreeMap<String, Vec<Connection>> = BTreeMap::new();
    for connection in snapshot.connections.drain(..) {
        groups
            .entry(connection.provider_id.clone())
            .or_default()
            .push(connection);
    }
    let mut kept: Vec<Connection> = Vec::new();
    // Dropped duplicates keep their usage evidence by pointing it at the one
    // connection the user sees.
    let mut remap: BTreeMap<String, String> = BTreeMap::new();
    for (_, mut group) in groups {
        let original = group.clone();
        let accounts: BTreeSet<String> = group
            .iter()
            .filter_map(|connection| connection.account_id.clone())
            .collect();
        let mut kept_here: Vec<String> = Vec::new();
        if accounts.is_empty() {
            group.sort_by_key(|connection| std::cmp::Reverse(rank(connection)));
            if let Some(best) = group.drain(..).next() {
                kept_here.push(best.id.clone());
                kept.push(best);
            }
        } else {
            for account in accounts {
                let mut same: Vec<Connection> = group
                    .iter()
                    .filter(|connection| connection.account_id.as_deref() == Some(account.as_str()))
                    .cloned()
                    .collect();
                same.sort_by_key(|connection| std::cmp::Reverse(rank(connection)));
                if let Some(best) = same.drain(..).next() {
                    kept_here.push(best.id.clone());
                    kept.push(best);
                }
            }
        }
        let anchor = original
            .iter()
            .filter(|connection| kept_here.contains(&connection.id))
            .max_by_key(|connection| rank(connection))
            .map(|connection| connection.id.clone());
        if let Some(anchor) = anchor {
            for connection in &original {
                if !kept_here.contains(&connection.id) {
                    remap.insert(connection.id.clone(), anchor.clone());
                }
            }
        }
    }
    let kept_connections: BTreeSet<String> = kept
        .iter()
        .map(|connection| connection.id.clone())
        .collect();
    snapshot.connections = kept;
    let provider_names: BTreeMap<String, String> = snapshot
        .providers
        .iter()
        .map(|provider| (provider.id.clone(), provider.name.clone()))
        .collect();
    let account_labels: BTreeMap<String, String> = snapshot
        .accounts
        .iter()
        .filter_map(|account| {
            account
                .display_label
                .value()
                .map(|label| (account.id.clone(), label.clone()))
        })
        .collect();
    let mut provider_counts: BTreeMap<String, usize> = BTreeMap::new();
    for connection in &snapshot.connections {
        *provider_counts
            .entry(connection.provider_id.clone())
            .or_default() += 1;
    }
    for connection in &mut snapshot.connections {
        let provider = provider_names
            .get(&connection.provider_id)
            .cloned()
            .unwrap_or_else(|| connection.provider_id.clone());
        let mut label = clean_display_name(&connection.display_name, &provider);
        if provider_counts
            .get(&connection.provider_id)
            .copied()
            .unwrap_or(0)
            > 1
        {
            let suffix = connection
                .account_id
                .as_ref()
                .and_then(|id| account_labels.get(id).cloned())
                .or_else(|| connection.account_id.as_deref().map(short_account))
                .unwrap_or_else(|| "Account".into());
            label = format!("{label} · {suffix}");
        }
        connection.display_name = label;
    }
    for profile in &mut snapshot.profiles {
        profile.label = clean_profile_label(&profile.label);
    }
    let kept_profiles: BTreeSet<String> = snapshot
        .connections
        .iter()
        .map(|connection| connection.profile_id.clone())
        .collect();
    let kept_installations: BTreeSet<String> = snapshot
        .connections
        .iter()
        .map(|connection| connection.installation_id.clone())
        .collect();
    snapshot
        .profiles
        .retain(|profile| kept_profiles.contains(&profile.id));
    snapshot
        .installations
        .retain(|installation| kept_installations.contains(&installation.id));
    let kept_routes: BTreeSet<String> = snapshot
        .routes
        .iter()
        .filter(|route| kept_connections.contains(&route.connection_id))
        .map(|route| route.id.clone())
        .collect();
    snapshot
        .routes
        .retain(|route| kept_routes.contains(&route.id));
    for route in &mut snapshot.routes {
        let provider = provider_names
            .get(&route.provider_id)
            .cloned()
            .unwrap_or_else(|| route.provider_id.clone());
        route.display_name = clean_display_name(&route.display_name, &provider);
    }
    snapshot
        .models
        .retain(|model| kept_routes.contains(&model.route_id));
    // Prefer pools that already belong to a kept connection, then fold any
    // pool read from a dropped duplicate onto that connection. Identical
    // window sets are one allowance, not several.
    snapshot.quota_pools.sort_by_key(|pool| {
        !pool
            .connection_ids
            .iter()
            .any(|connection| kept_connections.contains(connection))
    });
    let mut seen_pools: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    snapshot.quota_pools.retain_mut(|pool| {
        let mut mapped: Vec<String> = pool
            .connection_ids
            .iter()
            .map(|connection| {
                remap
                    .get(connection)
                    .cloned()
                    .unwrap_or_else(|| connection.clone())
            })
            .filter(|connection| kept_connections.contains(connection))
            .collect();
        mapped.sort();
        mapped.dedup();
        if mapped.is_empty() {
            return false;
        }
        let key = (
            mapped.join(","),
            pool.windows
                .iter()
                .map(|window| window.label.clone())
                .collect::<Vec<_>>()
                .join(","),
        );
        if !seen_pools.insert(key) {
            return false;
        }
        pool.connection_ids = mapped;
        pool.route_ids.retain(|route| kept_routes.contains(route));
        true
    });
    snapshot.issues.retain(|issue| {
        issue
            .connection_id
            .as_ref()
            .is_none_or(|connection| kept_connections.contains(connection))
    });
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use originlm_contracts::{
        AccessRoute, AuthState, Connection, DataMode, EvidenceKind, EvidenceSource, Installation,
        Model, Plan, Profile, Provider, QuotaPool, Snapshot, UsageWindow,
    };

    fn source() -> EvidenceSource {
        EvidenceSource::new(EvidenceKind::LocalConfig, "test", "test-fixture")
    }

    fn auth(state: AuthState) -> Observation<AuthState> {
        Observation::reported(state, source())
    }

    fn plan(label: &str) -> Observation<Plan> {
        Observation::reported(
            Plan {
                native_id: label.into(),
                label: label.into(),
            },
            source(),
        )
    }

    fn connection(
        id: &str,
        provider: &str,
        account: Option<&str>,
        state: AuthState,
        status: &str,
    ) -> Connection {
        Connection {
            id: id.into(),
            provider_id: provider.into(),
            installation_id: id.into(),
            profile_id: id.into(),
            display_name: id.into(),
            account_id: account.map(str::to_string),
            auth: auth(state),
            plan: plan("pro"),
            status: status.into(),
            diagnostics: Vec::new(),
        }
    }

    fn route(id: &str, connection: &Connection) -> AccessRoute {
        AccessRoute {
            id: id.into(),
            connection_id: connection.id.clone(),
            provider_id: connection.provider_id.clone(),
            display_name: id.into(),
            billing: Observation::reported(BillingMode::Subscription, source()),
            capabilities: Vec::new(),
            catalog_revision: "1".into(),
        }
    }

    fn model(route: &AccessRoute) -> Model {
        Model {
            id: format!("{}-model", route.id),
            route_id: route.id.clone(),
            native_id: "native".into(),
            name: "Model".into(),
            revision: "1".into(),
            is_default: true,
            capabilities: Vec::new(),
            options: Vec::new(),
            context_window: Observation::unknown("not reported"),
        }
    }

    fn pool(connection: &Connection, route: &AccessRoute) -> QuotaPool {
        QuotaPool {
            id: format!("{}-pool", connection.id),
            account_id: connection.account_id.clone(),
            connection_ids: vec![connection.id.clone()],
            route_ids: vec![route.id.clone()],
            sharing: "private".into(),
            windows: vec![UsageWindow {
                id: "window".into(),
                label: "Weekly".into(),
                reading: Observation::unknown("not reported"),
                resets_at: None,
                duration_seconds: None,
            }],
        }
    }

    fn snapshot_with(connections: Vec<Connection>) -> Snapshot {
        let mut snapshot = Snapshot::empty(DataMode::Live);
        for connection in &connections {
            snapshot.installations.push(Installation {
                id: connection.installation_id.clone(),
                provider_id: connection.provider_id.clone(),
                environment_id: "test".into(),
                location_hint: connection.id.clone(),
                version: Observation::unknown("not reported"),
            });
            snapshot.profiles.push(Profile {
                id: connection.profile_id.clone(),
                installation_id: connection.installation_id.clone(),
                label: connection.display_name.clone(),
            });
            let route = route(&format!("{}-route", connection.id), connection);
            snapshot.models.push(model(&route));
            snapshot.quota_pools.push(pool(connection, &route));
            snapshot.routes.push(route);
        }
        snapshot.connections = connections;
        snapshot
    }

    #[test]
    fn canonical_snapshot_prefers_the_account_bearing_connection() {
        let snapshot = snapshot_with(vec![
            connection(
                "claude-main",
                "claude_code",
                Some("account-1"),
                AuthState::Authenticated,
                "metadata_ready",
            ),
            connection(
                "claude-shim",
                "claude_code",
                None,
                AuthState::Authenticated,
                "detected",
            ),
            connection(
                "claude-bundled",
                "claude_code",
                None,
                AuthState::Unchecked,
                "configuration_only",
            ),
        ]);

        let canonical = canonical_snapshot(snapshot);

        assert_eq!(canonical.connections.len(), 1);
        assert_eq!(canonical.connections[0].id, "claude-main");
        assert_eq!(canonical.installations.len(), 1);
        assert_eq!(canonical.profiles.len(), 1);
        assert_eq!(canonical.routes.len(), 1);
        assert_eq!(canonical.models.len(), 1);
        assert_eq!(canonical.quota_pools.len(), 1);
    }

    #[test]
    fn canonical_snapshot_keeps_distinct_account_ids() {
        let snapshot = snapshot_with(vec![
            connection(
                "opencode-a",
                "opencode",
                Some("account-a"),
                AuthState::Authenticated,
                "metadata_ready",
            ),
            connection(
                "opencode-b",
                "opencode",
                Some("account-b"),
                AuthState::Authenticated,
                "metadata_ready",
            ),
        ]);

        let canonical = canonical_snapshot(snapshot);
        let ids: Vec<&str> = canonical
            .connections
            .iter()
            .map(|connection| connection.id.as_str())
            .collect();

        assert_eq!(ids, vec!["opencode-a", "opencode-b"]);
        assert_eq!(canonical.routes.len(), 2);
    }

    #[test]
    fn canonical_snapshot_keeps_quota_that_belongs_to_a_dropped_duplicate() {
        let mut snapshot = snapshot_with(vec![
            connection(
                "opencode-kept",
                "opencode",
                None,
                AuthState::Authenticated,
                "metadata_ready",
            ),
            connection(
                "opencode-pool",
                "opencode",
                None,
                AuthState::Unchecked,
                "detected",
            ),
        ]);
        // The quota was read from the less-authenticated duplicate. It must
        // still show on the one account the user sees.
        snapshot
            .quota_pools
            .retain(|pool| pool.id == "opencode-pool-pool");
        snapshot.quota_pools[0].connection_ids = vec!["opencode-pool".into()];
        snapshot.quota_pools[0].route_ids = vec!["opencode-pool-route".into()];

        let canonical = canonical_snapshot(snapshot);

        assert_eq!(canonical.connections.len(), 1);
        assert_eq!(canonical.connections[0].id, "opencode-kept");
        assert_eq!(canonical.quota_pools.len(), 1);
        assert_eq!(
            canonical.quota_pools[0].connection_ids,
            vec!["opencode-kept".to_string()]
        );
        // The duplicate's route is gone; the window still belongs to the kept
        // connection and remains visible.
        assert!(canonical.quota_pools[0].route_ids.is_empty());
    }

    #[test]
    fn canonical_snapshot_removes_installation_indices_from_user_facing_names() {
        let mut snapshot = snapshot_with(vec![connection(
            "opencode-main",
            "opencode",
            None,
            AuthState::Authenticated,
            "metadata_ready",
        )]);
        snapshot.providers.push(Provider {
            id: "opencode".into(),
            name: "OpenCode".into(),
            kind: "gateway".into(),
        });
        snapshot.connections[0].display_name = "OpenCode · Default profile · Installation 2".into();
        snapshot.profiles[0].label = "Default profile · Installation 2".into();
        snapshot.routes[0].display_name = "OpenCode · Default profile · Installation 2".into();

        let canonical = canonical_snapshot(snapshot);

        assert_eq!(canonical.connections[0].display_name, "OpenCode");
        assert_eq!(canonical.profiles[0].label, "Default");
        assert_eq!(canonical.routes[0].display_name, "OpenCode");
    }

    #[test]
    fn opencode_models_are_labeled_by_the_plan_route() {
        let mut snapshot = Snapshot::empty(DataMode::Live);
        for (id, provider) in [
            ("zen-route", "opencode"),
            ("go-route", "opencode-go"),
            ("local-route", "ollama"),
        ] {
            snapshot.routes.push(AccessRoute {
                id: id.into(),
                connection_id: "connection".into(),
                provider_id: provider.into(),
                display_name: id.into(),
                billing: Observation::reported(BillingMode::Subscription, source()),
                capabilities: Vec::new(),
                catalog_revision: "1".into(),
            });
        }
        for (id, route, name, native) in [
            (
                "free",
                "zen-route",
                "Crouton Alpha Free",
                "crouton-alpha-free",
            ),
            ("zen", "zen-route", "Claude Haiku 5.5", "claude-haiku-5-5"),
            ("go", "go-route", "Kimi K3.1 Pro", "kimi-k3.1-pro"),
            ("local", "local-route", "KAT-Coder", "kat-coder"),
            ("go-free", "go-route", "Beam Free", "beam-free"),
        ] {
            snapshot.models.push(Model {
                id: id.into(),
                route_id: route.into(),
                native_id: native.into(),
                name: name.into(),
                revision: "1".into(),
                is_default: false,
                capabilities: Vec::new(),
                options: Vec::new(),
                context_window: Observation::unknown("not reported"),
            });
        }

        let plans: Vec<Option<ModelPlan>> = snapshot
            .models
            .iter()
            .map(|model| model_plan(&snapshot, model))
            .collect();
        assert_eq!(
            plans,
            vec![
                Some(ModelPlan::Free),
                Some(ModelPlan::Zen),
                Some(ModelPlan::Go),
                Some(ModelPlan::Local),
                Some(ModelPlan::Go),
            ]
        );
    }

    #[test]
    fn canonical_snapshot_keeps_one_anonymous_connection_per_provider() {
        let snapshot = snapshot_with(vec![
            connection(
                "codex-ready",
                "codex",
                None,
                AuthState::Authenticated,
                "metadata_ready",
            ),
            connection(
                "codex-shim",
                "codex",
                None,
                AuthState::Authenticated,
                "detected",
            ),
        ]);

        let canonical = canonical_snapshot(snapshot);

        assert_eq!(canonical.connections.len(), 1);
        assert_eq!(canonical.connections[0].id, "codex-ready");
    }
}
