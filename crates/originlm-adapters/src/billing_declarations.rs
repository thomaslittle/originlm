//! Host-owned billing declarations for connections whose provider does not
//! report how the account pays.
//!
//! A declaration is user-supplied evidence, never provider verification: it
//! fills only billing that is still unknown after a probe, never replaces a
//! reported value, and is labeled `userSupplied` on every route it touches.
//! Unknown billing without a declaration stays refused by selection.
use originlm_contracts::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Native ACP clients a declaration may make executable. Only Grok can run a
/// text-only session with every native tool removed (see `native_clients`).
const DECLARED_NATIVE_EXECUTION: &[&str] = &["grok"];
/// Native clients whose billing is an account plan, never a local runtime.
const ACCOUNT_CLIENTS: &[&str] = &["gemini_cli", "antigravity", "grok", "muse"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingDeclaration {
    pub provider_id: String,
    pub billing: BillingMode,
}

#[derive(Debug, Clone, Default)]
pub struct BillingDeclarations {
    entries: BTreeMap<String, BillingDeclaration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    connection_id: String,
    provider_id: String,
    billing: BillingMode,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    declarations: Vec<Entry>,
}

fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Invalid billing declarations; each entry needs a connection, provider and known billing mode",
    )
}
fn identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

impl BillingDeclarations {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 65536 {
            return Err(invalid());
        }
        let document: Document = serde_json::from_str(text).map_err(|_| invalid())?;
        if document.declarations.len() > 128 {
            return Err(invalid());
        }
        let mut entries = BTreeMap::new();
        for entry in document.declarations {
            if !identifier(&entry.connection_id, 256)
                || !identifier(&entry.provider_id, 64)
                || matches!(entry.billing, BillingMode::Unknown | BillingMode::Mixed)
                || (entry.billing == BillingMode::Local
                    && ACCOUNT_CLIENTS.contains(&entry.provider_id.as_str()))
            {
                return Err(invalid());
            }
            let declaration = BillingDeclaration {
                provider_id: entry.provider_id,
                billing: entry.billing,
            };
            if entries.insert(entry.connection_id, declaration).is_some() {
                return Err(invalid());
            }
        }
        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The declaration for this connection, only when it names the provider
    /// that was actually discovered there (a reused id never inherits one).
    pub fn get(&self, connection: &str, provider: &str) -> Option<&BillingDeclaration> {
        self.entries
            .get(connection)
            .filter(|declaration| declaration.provider_id == provider)
    }

    /// Declared connections that may run a native text-only session although
    /// they were discovered rather than registered as a native profile.
    pub fn native_execution_connections(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|(_, d)| DECLARED_NATIVE_EXECUTION.contains(&d.provider_id.as_str()))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Fill unknown billing on the probed route and any gateway sub-routes.
    /// Reported billing (of any kind) is left untouched.
    pub fn apply(&self, provider: &str, report: &mut ConnectionMetadata) {
        let Some(declaration) = self.get(&report.connection_id, provider) else {
            return;
        };
        let declared = || {
            Observation::reported(
                declaration.billing.clone(),
                EvidenceSource::new(
                    EvidenceKind::UserSupplied,
                    provider,
                    "host billing declaration; not verified by the provider",
                ),
            )
        };
        let mut applied = false;
        if report.billing.value().is_none() {
            report.billing = declared();
            applied = true;
        }
        for route in &mut report.additional_routes {
            if route.connection_id == report.connection_id && route.billing.value().is_none() {
                route.billing = declared();
                applied = true;
            }
        }
        // Do not mark the connection "declared" when provider evidence already
        // classified every route (for example OpenCode Go). The diagnostic is
        // about an observation that the host supplied, not the presence of a
        // matching declaration entry.
        if applied {
            report.diagnostics.push("BILLING_DECLARED_BY_HOST".into());
        }
    }
}
