use crate::{Selection, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Ready,
    Running,
    AwaitingApproval,
    Completed,
    Error,
    Cancelled,
    Closed,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub connection_id: String,
    pub selection: Selection,
    pub state: SessionState,
    pub created_at: Timestamp,
    pub native_resumption: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum EventPayload {
    State {
        state: SessionState,
    },
    TextDelta {
        text: String,
    },
    ReasoningSummary {
        text: String,
    },
    ApprovalRequest {
        approval_id: String,
        action: String,
        detail: String,
    },
    ApprovalResolved {
        approval_id: String,
        allowed: bool,
    },
    Tool {
        name: String,
        status: String,
    },
    Usage {
        unit: String,
        used: String,
    },
    Artifact {
        artifact_id: String,
        media_type: String,
        size: String,
    },
    OptionsChanged {
        revision: String,
    },
    Warning {
        code: String,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
    Final {
        status: String,
    },
    Extension {
        namespace: String,
        data: Value,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    pub session_id: String,
    pub turn_id: Option<String>,
    pub sequence: String,
    pub timestamp: Timestamp,
    pub adapter_id: String,
    pub schema_version: String,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceChangeKind {
    Added,
    Updated,
    Removed,
}

/// A bounded description of what changed. The full authoritative state remains
/// `snapshot.get`; consumers that cannot replay must resynchronize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceChange {
    /// `connection`, `route`, `model`, `quota_pool`, `account`, `issue`, or `health`.
    pub resource: String,
    pub id: String,
    pub change: ResourceChangeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InventoryChange {
    pub sequence: String,
    pub cursor: String,
    pub revision: String,
    pub previous_revision: String,
    pub observed_at: Timestamp,
    pub reason: String,
    pub changes: Vec<ResourceChange>,
    /// True when the affected issues changed. Issue text is available from
    /// `issues.list`, which applies the caller's permission projection.
    pub issues_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InventoryEventPage {
    pub events: Vec<InventoryChange>,
    pub cursor: String,
    pub tail_cursor: String,
    pub has_more: bool,
    pub revision: String,
}
