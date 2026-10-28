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
        Observation::Unknown { .. } => "plan unknown".into(),
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
            let remaining = sample.remaining?;
            let used = (1.0 - remaining).clamp(0.0, 1.0) * 100.0;
            Some(ChartPoint {
                label: chrono::DateTime::from_timestamp(sample.t, 0)
                    .map(|stamp| stamp.format(format).to_string())
                    .unwrap_or_else(|| "—".into()),
                value: (used * 10.0).round() / 10.0,
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
