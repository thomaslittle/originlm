//! Host-owned account scopes for passive discovery.
//!
//! A scope names one provider, one exact profile home and one exact logical
//! executable. The official scanner derives the installation/profile/
//! connection identity and any Unix symlink identity; callers do not
//! manufacture those records. Each account is scanned independently and
//! failure in one does not suppress the others.

use crate::{HostPlatform, ScanEnvironment, ScanResult, Scanner};
use originlm_contracts::{
    DataMode, DomainError, ErrorCode, Result, Snapshot, Timestamp, stable_id,
};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

/// Providers that have an exact per-account home and executable contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedProvider {
    Codex,
    ClaudeCode,
    OpenCode,
}

impl ScopedProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
            Self::OpenCode => "opencode",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::OpenCode => "OpenCode",
        }
    }

    fn home_override(self) -> &'static str {
        match self {
            Self::Codex => "CODEX_HOME",
            Self::ClaudeCode => "CLAUDE_CONFIG_DIR",
            Self::OpenCode => "OPENCODE_CONFIG_DIR",
        }
    }
}

/// One exact account slot: provider, profile home and logical executable.
#[derive(Debug, Clone)]
pub struct AccountScope {
    pub provider: ScopedProvider,
    pub home: PathBuf,
    pub executable: PathBuf,
    pub label: Option<String>,
}

impl AccountScope {
    pub fn new(provider: ScopedProvider, home: PathBuf, executable: PathBuf) -> Self {
        Self {
            provider,
            home,
            executable,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn validate(&self) -> Result<()> {
        if !local_path(&self.home) || !local_path(&self.executable) {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "Account home and executable must be absolute local paths",
            ));
        }
        if self.label.as_ref().is_some_and(|label| {
            label.is_empty() || label.len() > 128 || label.chars().any(char::is_control)
        }) {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "Account label must be non-empty and bounded",
            ));
        }
        Ok(())
    }

    fn accept(&self, mut part: ScanResult) -> Result<Option<ScanResult>> {
        let exact: Vec<String> = part
            .locations
            .iter()
            .filter(|(id, located)| {
                located.provider_id == self.provider.id()
                    && located.profile_home == self.home
                    && located.executable.as_ref() == Some(&self.executable)
                    && !part
                        .snapshot
                        .connections
                        .iter()
                        .find(|connection| &connection.id == *id)
                        .is_some_and(|connection| {
                            connection
                                .diagnostics
                                .iter()
                                .any(|kind| kind == "NO_CONFIGURATION_DIRECTORY")
                        })
            })
            .map(|(id, _)| id.clone())
            .collect();
        if exact.is_empty() {
            return Ok(None);
        }
        let exact: std::collections::BTreeSet<String> = exact.into_iter().collect();
        let profiles: std::collections::BTreeSet<String> = part
            .snapshot
            .connections
            .iter()
            .filter(|connection| exact.contains(&connection.id))
            .map(|connection| connection.profile_id.clone())
            .collect();
        let installations: std::collections::BTreeSet<String> = part
            .snapshot
            .connections
            .iter()
            .filter(|connection| exact.contains(&connection.id))
            .map(|connection| connection.installation_id.clone())
            .collect();
        let routes: std::collections::BTreeSet<String> = part
            .snapshot
            .routes
            .iter()
            .filter(|route| exact.contains(&route.connection_id))
            .map(|route| route.id.clone())
            .collect();
        part.snapshot
            .connections
            .retain(|connection| exact.contains(&connection.id));
        part.snapshot
            .profiles
            .retain(|profile| profiles.contains(&profile.id));
        part.snapshot
            .installations
            .retain(|installation| installations.contains(&installation.id));
        part.snapshot
            .routes
            .retain(|route| routes.contains(&route.id));
        part.locations.retain(|id, _| exact.contains(id));
        if let Some(label) = &self.label {
            let display = format!("{} · {label}", self.provider.name());
            for connection in part.snapshot.connections.iter_mut() {
                connection.display_name = display.clone();
                if let Some(profile) = part
                    .snapshot
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == connection.profile_id)
                {
                    profile.label = label.clone();
                }
                if let Some(route) = part
                    .snapshot
                    .routes
                    .iter_mut()
                    .find(|route| route.connection_id == connection.id)
                {
                    route.display_name = display.clone();
                }
            }
        }
        Ok(Some(part))
    }
}

/// A bounded set of exact account scopes.
#[derive(Debug, Clone, Default)]
pub struct ScopedInventory {
    accounts: Vec<AccountScope>,
}

impl ScopedInventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_account(mut self, account: AccountScope) -> Self {
        self.accounts.push(account);
        self
    }

    pub fn push(&mut self, account: AccountScope) {
        self.accounts.push(account);
    }

    pub fn accounts(&self) -> &[AccountScope] {
        &self.accounts
    }

    /// Scan each exact account independently with the official provider rules.
    /// A missing/unresolvable account is omitted with a path-free diagnostic;
    /// successful accounts remain distinct. Cancellation fails the whole scan.
    pub fn scan(&self, cancel: &AtomicBool) -> Result<ScanResult> {
        if self.accounts.is_empty() {
            return Err(invalid("At least one account scope is required"));
        }
        if self.accounts.len() > 32 {
            return Err(invalid("At most 32 account scopes are supported"));
        }
        let platform = host_platform();
        let mut merged = ScanResult {
            snapshot: Snapshot::empty(DataMode::Live),
            locations: BTreeMap::new(),
            metadata_probes: 0,
        };
        let mut successes = 0usize;
        let mut last_error = None;
        for account in &self.accounts {
            if cancel.load(Ordering::Acquire) {
                return Err(DomainError::new(
                    ErrorCode::Cancelled,
                    "Discovery cancelled",
                ));
            }
            account.validate()?;
            let mut environment = ScanEnvironment::for_home(account.home.clone(), platform);
            environment.overrides.insert(
                account.provider.home_override().into(),
                account.home.clone(),
            );
            environment
                .executable_overrides
                .insert(account.provider.id().into(), account.executable.clone());
            match Scanner::new(originlm_platform::NativeFilesystem).scan_selected(
                &environment,
                &[account.provider.id()],
                cancel,
                |_| {},
            ) {
                Ok(part) => match account.accept(part)? {
                    Some(part) => {
                        successes += 1;
                        merge(&mut merged, part);
                    }
                    None => merged.snapshot.diagnostics.push(format!(
                        "{}_ACCOUNT_SCOPE_UNAVAILABLE",
                        account.provider.id()
                    )),
                },
                Err(error) if error.code == ErrorCode::Cancelled => return Err(error),
                Err(error) => {
                    last_error = Some(error);
                    merged.snapshot.diagnostics.push(format!(
                        "{}_ACCOUNT_SCOPE_UNAVAILABLE",
                        account.provider.id()
                    ));
                }
            }
        }
        if successes == 0 {
            return Err(last_error.unwrap_or_else(|| {
                DomainError::new(
                    ErrorCode::RuntimeMissing,
                    "No configured account scope was discoverable",
                )
            }));
        }
        merged.snapshot.diagnostics.sort();
        merged.snapshot.diagnostics.dedup();
        merged.snapshot.observed_at = Timestamp::now();
        merged.snapshot.revision = stable_id(
            "inventory",
            &[&merged
                .snapshot
                .connections
                .iter()
                .map(|connection| connection.id.as_str())
                .collect::<Vec<_>>()
                .join("|")],
        );
        Ok(merged)
    }
}

fn merge(into: &mut ScanResult, mut part: ScanResult) {
    merge_by(&mut into.snapshot.providers, part.snapshot.providers, |p| {
        p.id.clone()
    });
    merge_by(
        &mut into.snapshot.installations,
        part.snapshot.installations,
        |i| i.id.clone(),
    );
    merge_by(&mut into.snapshot.profiles, part.snapshot.profiles, |p| {
        p.id.clone()
    });
    merge_by(&mut into.snapshot.accounts, part.snapshot.accounts, |a| {
        a.id.clone()
    });
    merge_by(
        &mut into.snapshot.connections,
        part.snapshot.connections,
        |c| c.id.clone(),
    );
    merge_by(&mut into.snapshot.routes, part.snapshot.routes, |r| {
        r.id.clone()
    });
    merge_by(&mut into.snapshot.models, part.snapshot.models, |m| {
        m.id.clone()
    });
    merge_by(
        &mut into.snapshot.quota_pools,
        part.snapshot.quota_pools,
        |p| p.id.clone(),
    );
    merge_by(&mut into.snapshot.issues, part.snapshot.issues, |i| {
        i.id.clone()
    });
    into.metadata_probes += part.metadata_probes;
    into.snapshot
        .diagnostics
        .append(&mut part.snapshot.diagnostics);
    for (id, located) in part.locations {
        into.locations.entry(id).or_insert(located);
    }
}

fn merge_by<T, K: PartialEq>(target: &mut Vec<T>, source: Vec<T>, key: impl Fn(&T) -> K) {
    for item in source {
        let id = key(&item);
        if !target.iter().any(|existing| key(existing) == id) {
            target.push(item);
        }
    }
}

fn local_path(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    path.is_absolute()
        && !text.starts_with("//")
        && !text.starts_with(r"\\")
        && !text.chars().any(char::is_control)
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn host_platform() -> HostPlatform {
    if cfg!(windows) {
        HostPlatform::Windows
    } else if cfg!(target_os = "macos") {
        HostPlatform::Macos
    } else {
        HostPlatform::Linux
    }
}

fn invalid(message: &str) -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path, sync::atomic::AtomicBool};

    fn home(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn executable(root: &Path, relative: &str) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        path
    }

    #[test]
    fn three_exact_accounts_remain_distinct_and_one_failure_is_isolated() {
        let root = tempfile::tempdir().unwrap();
        let codex_home = home(root.path(), "codex-a-home");
        let codex_exe = executable(root.path(), "bin/a/codex");
        let claude_home = home(root.path(), "claude-home");
        let claude_exe = executable(root.path(), "bin/b/claude");
        let opencode_home = home(root.path(), "opencode-home");
        let opencode_exe = executable(root.path(), "bin/c/opencode");
        let second_codex_home = home(root.path(), "codex-b-home");
        let second_codex_exe = executable(root.path(), "bin/d/codex");
        let broken_home = home(root.path(), "broken-home");
        let broken_exe = root.path().join("bin/e/codex");

        let scoped = ScopedInventory::new()
            .with_account(
                AccountScope::new(ScopedProvider::Codex, codex_home.clone(), codex_exe.clone())
                    .with_label("Codex A"),
            )
            .with_account(
                AccountScope::new(
                    ScopedProvider::Codex,
                    second_codex_home.clone(),
                    second_codex_exe.clone(),
                )
                .with_label("Codex B"),
            )
            .with_account(AccountScope::new(
                ScopedProvider::ClaudeCode,
                claude_home.clone(),
                claude_exe.clone(),
            ))
            .with_account(AccountScope::new(
                ScopedProvider::OpenCode,
                opencode_home.clone(),
                opencode_exe.clone(),
            ))
            .with_account(AccountScope::new(
                ScopedProvider::Codex,
                broken_home,
                broken_exe,
            ));
        let result = scoped.scan(&AtomicBool::new(false)).unwrap();

        assert_eq!(result.snapshot.connections.len(), 4);
        let providers: std::collections::BTreeSet<_> = result
            .snapshot
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect();
        assert!(providers.contains("codex"));
        assert!(providers.contains("claude_code"));
        assert!(providers.contains("opencode"));
        assert!(
            result
                .snapshot
                .diagnostics
                .iter()
                .any(|line| line == "codex_ACCOUNT_SCOPE_UNAVAILABLE")
        );

        for (provider, home, executable) in [
            (ScopedProvider::Codex, &codex_home, &codex_exe),
            (ScopedProvider::Codex, &second_codex_home, &second_codex_exe),
            (ScopedProvider::ClaudeCode, &claude_home, &claude_exe),
            (ScopedProvider::OpenCode, &opencode_home, &opencode_exe),
        ] {
            assert!(
                result.locations.values().any(|located| {
                    located.provider_id == provider.id()
                        && &located.profile_home == home
                        && located.executable.as_ref() == Some(executable)
                }),
                "missing exact location for {} at {}",
                provider.id(),
                home.display()
            );
        }

        let codex_connections: Vec<_> = result
            .snapshot
            .connections
            .iter()
            .filter(|connection| connection.provider_id == "codex")
            .collect();
        assert_eq!(codex_connections.len(), 2);
        assert_ne!(codex_connections[0].id, codex_connections[1].id);
        assert!(
            codex_connections
                .iter()
                .any(|connection| connection.display_name.contains("Codex A"))
        );
        assert!(
            codex_connections
                .iter()
                .any(|connection| connection.display_name.contains("Codex B"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_logical_symlink_keeps_its_official_resolved_identity() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let account_home = home(root.path(), "codex-home");
        let target = executable(root.path(), "bin/real/codex-1.2.3");
        let logical = root.path().join("bin/logical/codex");
        fs::create_dir_all(logical.parent().unwrap()).unwrap();
        symlink(&target, &logical).unwrap();

        let result = ScopedInventory::new()
            .with_account(AccountScope::new(
                ScopedProvider::Codex,
                account_home,
                logical.clone(),
            ))
            .scan(&AtomicBool::new(false))
            .unwrap();
        let located = result
            .locations
            .values()
            .find(|located| located.executable.as_ref() == Some(&logical))
            .expect("logical executable");
        let identity = located
            .executable_identity
            .as_ref()
            .expect("symlink identity");
        assert_eq!(identity.command, logical);
        assert_eq!(identity.resolved, target);
    }
}
