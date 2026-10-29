//! Explicit passive inventory. No client launches, secret reads, or network calls.
mod environment;
pub mod knowledge;
pub mod log_tail;
mod registry;
mod scanner;
mod scoped;
pub use environment::{HostPlatform, ScanEnvironment, join_relative};
pub use knowledge::{
    DiscoveryRule, RuleBasis, RuleRole, T3CODE_COMMIT, T3CODE_REPOSITORY, TOKSCALE_COMMIT,
    TOKSCALE_REPOSITORY, discovery_rules, has_profile_override, profile_roots, session_stores,
};
use originlm_contracts::Snapshot;
use originlm_platform::LocalFilesystem;
pub use originlm_platform::{ExecutableIdentity, resolve_executable};
pub use registry::{MUSE_BINARY_OVERRIDE, ProviderDefinition, definitions};
pub use scoped::{AccountScope, ScopedInventory, ScopedProvider};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
#[derive(Debug, Clone)]
pub struct LocatedClient {
    pub provider_id: String,
    pub profile_home: PathBuf,
    pub executable: Option<PathBuf>,
    /// Logical command and pinned Unix target for a discovered executable link.
    pub executable_identity: Option<ExecutableIdentity>,
}
impl LocatedClient {
    pub fn validate_executable(&self) -> originlm_contracts::Result<()> {
        if let Some(identity) = &self.executable_identity {
            if self.executable.as_ref() != Some(&identity.command) {
                return Err(originlm_contracts::DomainError::new(
                    originlm_contracts::ErrorCode::RuntimeMissing,
                    "Executable location changed",
                ));
            }
            identity.validate()?;
        }
        Ok(())
    }
}
/// Locations stay inside the native process; only `snapshot` is a public wire record.
pub struct ScanResult {
    pub snapshot: Snapshot,
    pub locations: BTreeMap<String, LocatedClient>,
    pub metadata_probes: usize,
}
pub struct Scanner<F: LocalFilesystem> {
    filesystem: F,
    executable_only_providers: BTreeSet<String>,
}
impl<F: LocalFilesystem> Scanner<F> {
    pub fn new(filesystem: F) -> Self {
        Self {
            filesystem,
            executable_only_providers: BTreeSet::new(),
        }
    }
    /// Keep selected providers' executable candidates private for dependent integrations.
    pub fn with_executable_only_providers(mut self, providers: BTreeSet<String>) -> Self {
        self.executable_only_providers = providers;
        self
    }
}
