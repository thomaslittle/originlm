//! Native selection and permission-scoped projections. No I/O or provider code.
mod projection;
mod selection;
pub use originlm_contracts::{Candidate, Requirements};
pub use projection::project_snapshot;
pub use selection::{is_accountless_local_route, resolve, validate_selection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPolicy {
    pub allow_passive_discovery: bool,
    /// None means all metadata; Some(empty) means none. Only the native host sets this.
    pub connection_ids: Option<BTreeSet<String>>,
    pub reveal_identity: bool,
    pub probe_connection_ids: BTreeSet<String>,
    #[serde(default)]
    pub probe_provider_ids: BTreeSet<String>,
    /// Host-only administrative surface (list/revoke application clients).
    /// Never granted by a protocol request.
    #[serde(default)]
    pub admin: bool,
}
impl Default for ClientPolicy {
    fn default() -> Self {
        Self {
            allow_passive_discovery: false,
            connection_ids: Some(BTreeSet::new()),
            reveal_identity: false,
            probe_connection_ids: BTreeSet::new(),
            probe_provider_ids: BTreeSet::new(),
            admin: false,
        }
    }
}
impl ClientPolicy {
    pub fn denied() -> Self {
        Self::default()
    }
    pub fn passive() -> Self {
        Self {
            allow_passive_discovery: true,
            connection_ids: None,
            ..Self::default()
        }
    }
    pub fn allows(&self, id: &str) -> bool {
        self.connection_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(id))
    }
}
