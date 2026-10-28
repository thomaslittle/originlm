//! Persistent, actionable health state. An issue is an observation about a
//! resource, not a claim that a provider or account has been changed.
use crate::{EvidenceSource, Freshness, Observation, Timestamp, Verification};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    /// Still observed or not yet verified as recovered.
    Open,
    /// The user has seen it; it is still unhealthy.
    Acknowledged,
    /// Notifications are suppressed until this time. The issue is still open.
    Snoozed,
    /// A later observation verified recovery. Dismissal alone never sets this.
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    AuthRequired,
    AuthExpired,
    KeyringLocked,
    UnsupportedClient,
    UsageUnavailable,
    QuotaExhausted,
    RateLimited,
    AdapterFailure,
    StorageUnavailable,
    PermissionDenied,
    ServiceUnavailable,
    ConfigurationInvalid,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RemediationKind {
    Refresh,
    SignIn,
    UnlockKeyring,
    UpdateClient,
    OpenSettings,
    RetryLater,
    RestartService,
    FreeStorage,
    ContactSupport,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Remediation {
    pub kind: RemediationKind,
    pub label: String,
    pub description: String,
    /// True when the runtime cannot safely perform this action on the user's behalf.
    pub requires_user_action: bool,
}

/// A bounded, sanitized fact supporting an issue. It never contains credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IssueEvidence {
    pub code: String,
    pub message: String,
    pub source: Option<EvidenceSource>,
    pub observed_at: Option<Timestamp>,
    pub freshness: Option<Freshness>,
    pub verification: Option<Verification>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Issue {
    /// Stable across restarts for the same resource and cause.
    pub id: String,
    /// Additional non-secret grouping material. Never a provider token or email.
    pub fingerprint: String,
    pub kind: IssueKind,
    pub severity: IssueSeverity,
    pub state: IssueState,
    pub title: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    pub first_seen_at: Timestamp,
    pub last_seen_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<Timestamp>,
    pub occurrences: u32,
    pub evidence: Vec<IssueEvidence>,
    pub remediation: Vec<Remediation>,
    /// Last known observation while a person decides what to do. Display-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_observation: Option<Observation<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HealthSummary {
    pub status: HealthStatus,
    pub open_issues: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst_severity: Option<IssueSeverity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<Timestamp>,
    /// True when the summary is built from persisted state that has not yet been reverified.
    pub hydrated: bool,
}
impl Default for HealthSummary {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            open_issues: 0,
            worst_severity: None,
            checked_at: None,
            hydrated: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IssueUpdateParams {
    pub issue_id: String,
    /// ISO-8601 time until which notifications are suppressed. Only for `snooze`.
    #[serde(default)]
    pub until: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IssueUpdateResult {
    pub issue: Issue,
}

/// A deduplicated OS-notification decision. Delivery state is persisted by the
/// broker so multiple consumers do not each emit the same alert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationDecision {
    pub issue_id: String,
    pub severity: IssueSeverity,
    pub title: String,
    pub body: String,
    pub should_notify: bool,
    pub reason: String,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationReason {
    NewIssue,
    Escalated,
    Reminder,
    Recovered,
    SuppressedDuplicate,
    SuppressedQuietHours,
    SuppressedDisabled,
    SuppressedSnoozed,
}

impl NotificationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NewIssue => "new_issue",
            Self::Escalated => "escalated",
            Self::Reminder => "reminder",
            Self::Recovered => "recovered",
            Self::SuppressedDuplicate => "suppressed_duplicate",
            Self::SuppressedQuietHours => "suppressed_quiet_hours",
            Self::SuppressedDisabled => "suppressed_disabled",
            Self::SuppressedSnoozed => "suppressed_snoozed",
        }
    }
}

/// Per-install notification preference. Quiet hours use local wall-clock
/// minutes since midnight and never contain a provider identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct NotificationPreferences {
    pub enabled: bool,
    pub minimum_severity: IssueSeverity,
    pub quiet_hours_start_minute: Option<u16>,
    pub quiet_hours_end_minute: Option<u16>,
    pub reminder_after_seconds: u64,
    /// Suppress issue text in the OS notification; consumers show it inline.
    pub private_content: bool,
}
impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            minimum_severity: IssueSeverity::Warning,
            quiet_hours_start_minute: None,
            quiet_hours_end_minute: None,
            reminder_after_seconds: 24 * 60 * 60,
            private_content: false,
        }
    }
}

pub fn issue_severity_label(value: IssueSeverity) -> &'static str {
    match value {
        IssueSeverity::Info => "info",
        IssueSeverity::Warning => "warning",
        IssueSeverity::Error => "error",
        IssueSeverity::Critical => "critical",
    }
}
