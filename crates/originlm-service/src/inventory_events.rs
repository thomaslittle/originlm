//! Journal records are global internally. Public deltas disclose only resources
//! visible in the caller's current projection. Removed or historically unknown
//! resources use an anonymous invalidation hint so clients still resynchronize.
use originlm_contracts::{InventoryEventPage, ResourceChange, ResourceChangeKind, Snapshot};
use originlm_core::ClientPolicy;
use std::collections::BTreeSet;

pub(crate) fn project(
    mut page: InventoryEventPage,
    visible: &Snapshot,
    policy: &ClientPolicy,
) -> InventoryEventPage {
    let mut ids = BTreeSet::new();
    macro_rules! include {
        ($field:ident, $kind:literal) => {
            for item in &visible.$field {
                ids.insert(($kind, item.id.as_str()));
            }
        };
    }
    include!(providers, "provider");
    include!(installations, "installation");
    include!(profiles, "profile");
    include!(accounts, "account");
    include!(connections, "connection");
    include!(routes, "route");
    include!(models, "model");
    include!(quota_pools, "quota_pool");
    include!(issues, "issue");
    for event in &mut page.events {
        event.changes.retain(|change| {
            ids.contains(&(change.resource.as_str(), change.id.as_str()))
                || (change.resource == "connection" && policy.allows(&change.id))
        });
        event.issues_changed = event.changes.iter().any(|c| c.resource == "issue");
        if event.changes.is_empty() {
            event.changes.push(ResourceChange {
                resource: "inventory".into(),
                id: "*".into(),
                change: ResourceChangeKind::Updated,
                reason: Some("Read a scoped snapshot to reconcile inventory".into()),
            });
        }
    }
    page
}
