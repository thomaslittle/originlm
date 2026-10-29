use crate::environment::{join_relative, valid_root};
use crate::{HostPlatform, LocatedClient, ScanEnvironment, ScanResult, Scanner, definitions};
use originlm_contracts::*;
use originlm_platform::{EntryKind, LocalFilesystem};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
fn path_key(path: &Path) -> String {
    format!("{:x}", Sha256::digest(path.as_os_str().as_encoded_bytes()))
}
fn preferred_executable(
    binaries: &BTreeSet<std::path::PathBuf>,
    platform: HostPlatform,
) -> Option<std::path::PathBuf> {
    binaries
        .iter()
        .min_by_key(|path| {
            if platform != HostPlatform::Windows {
                return 0;
            }
            match path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("exe") => 0,
                Some("cmd") => 1,
                Some("bat") => 2,
                _ => 3,
            }
        })
        .cloned()
}
fn cancelled(flag: &AtomicBool) -> Result<()> {
    if flag.load(Ordering::Relaxed) {
        Err(DomainError::new(
            ErrorCode::Cancelled,
            "Discovery cancelled",
        ))
    } else {
        Ok(())
    }
}
impl<F: LocalFilesystem> Scanner<F> {
    pub fn scan(
        &self,
        env: &ScanEnvironment,
        cancel: &AtomicBool,
        progress: impl FnMut(&Snapshot),
    ) -> Result<ScanResult> {
        self.scan_selected(env, &[], cancel, progress)
    }

    /// Scan only the named providers. An empty slice keeps the full registry.
    /// This is the supported path for a host-owned per-account scope; it does
    /// not widen discovery to unrelated providers under the same home.
    pub fn scan_selected(
        &self,
        env: &ScanEnvironment,
        providers: &[&str],
        cancel: &AtomicBool,
        mut progress: impl FnMut(&Snapshot),
    ) -> Result<ScanResult> {
        cancelled(cancel)?;
        env.validate()?;
        let environment_id = stable_id(
            "environment",
            &[&path_key(&env.home), &format!("{:?}", env.platform)],
        );
        let mut result = ScanResult {
            snapshot: Snapshot::empty(DataMode::Live),
            locations: BTreeMap::new(),
            metadata_probes: 0,
        };
        let mut directories = env
            .search_path
            .iter()
            .filter(|p| valid_root(p))
            .cloned()
            .collect::<BTreeSet<_>>();
        directories.extend([
            join_relative(&env.home, ".local/bin"),
            join_relative(&env.home, ".cargo/bin"),
            join_relative(&env.home, ".bun/bin"),
        ]);
        if env.platform == HostPlatform::Windows {
            directories.insert(join_relative(&env.app_data, "npm"));
        }
        let children = self.filesystem.children(&env.home, 128);
        let extra = match children {
            Ok(listing) => {
                if listing.truncated {
                    result
                        .snapshot
                        .diagnostics
                        .push("PROFILE_ENUMERATION_PARTIAL".into());
                }
                listing.entries
            }
            Err(_) => {
                result
                    .snapshot
                    .diagnostics
                    .push("PROFILE_ENUMERATION_UNAVAILABLE".into());
                vec![]
            }
        };
        let mut installations = BTreeSet::new();
        for definition in definitions() {
            if !providers.is_empty() && !providers.contains(&definition.id) {
                continue;
            }
            cancelled(cancel)?;
            let executable_only = self.executable_only_providers.contains(definition.id);
            let mut binaries = BTreeSet::new();
            let mut identities = BTreeMap::new();
            let extra_dirs = definition.extra_directories(env);
            let configured = definition.executable_override(env);
            if let Some(path) = configured {
                result.metadata_probes += 1;
                match self.filesystem.probe(path) {
                    Ok(EntryKind::File) => {
                        binaries.insert(path.clone());
                    }
                    Ok(EntryKind::Symlink) if env.platform != HostPlatform::Windows => {
                        if let Ok(identity) = self.filesystem.resolve_executable(path) {
                            identities.insert(path.clone(), identity);
                            binaries.insert(path.clone());
                        } else {
                            result.snapshot.diagnostics.push(format!(
                                "{}_CONFIGURED_EXECUTABLE_UNAVAILABLE",
                                definition.id
                            ));
                        }
                    }
                    _ => result.snapshot.diagnostics.push(format!(
                        "{}_CONFIGURED_EXECUTABLE_UNAVAILABLE",
                        definition.id
                    )),
                }
            }
            let search = if configured.is_some() {
                vec![]
            } else {
                directories
                    .iter()
                    .chain(extra_dirs.iter())
                    .collect::<Vec<_>>()
            };
            for directory in search {
                for command in definition.commands {
                    for suffix in if env.platform == HostPlatform::Windows {
                        &[".exe", ".cmd", ".bat", ""][..]
                    } else {
                        &[""][..]
                    } {
                        cancelled(cancel)?;
                        let path = directory.join(format!("{command}{suffix}"));
                        result.metadata_probes += 1;
                        match self.filesystem.probe(&path) {
                            Ok(EntryKind::File) => {
                                binaries.insert(path);
                            }
                            Ok(EntryKind::Symlink) if env.platform != HostPlatform::Windows => {
                                match self.filesystem.resolve_executable(&path) {
                                    Ok(identity) => {
                                        identities.insert(path.clone(), identity);
                                        binaries.insert(path);
                                    }
                                    Err(_) if !executable_only => result.snapshot.diagnostics.push(
                                        format!("{}_EXECUTABLE_SYMLINK_UNAVAILABLE", definition.id),
                                    ),
                                    Err(_) => {}
                                }
                            }
                            Ok(EntryKind::Symlink) if !executable_only => result
                                .snapshot
                                .diagnostics
                                .push(format!("{}_EXECUTABLE_SYMLINK_NOT_FOLLOWED", definition.id)),
                            Err(_) if !executable_only => result
                                .snapshot
                                .diagnostics
                                .push(format!("{}_EXECUTABLE_METADATA_UNAVAILABLE", definition.id)),
                            _ => {}
                        }
                    }
                }
            }
            if binaries.len() > 8 && !executable_only {
                result
                    .snapshot
                    .diagnostics
                    .push(format!("{}_INSTALLATION_LIST_PARTIAL", definition.id));
            }
            // Windows launch shims in one directory represent one installation.
            // Different directories may contain different versions; retain them.
            let mut groups = BTreeMap::<_, BTreeSet<_>>::new();
            for binary in &binaries {
                groups
                    .entry(binary.parent().map(Path::to_path_buf))
                    .or_default()
                    .insert(binary.clone());
            }
            let mut executables: Vec<_> = groups
                .values()
                .take(8)
                .map(|group| preferred_executable(group, env.platform))
                .collect();
            if executables.is_empty() {
                executables.push(None);
            }
            if executable_only {
                for executable in executables.into_iter().flatten() {
                    let key = stable_id(
                        "private-executable",
                        &[definition.id, &path_key(&executable)],
                    );
                    result.locations.insert(
                        key,
                        LocatedClient {
                            executable_identity: identities.get(&executable).cloned(),
                            provider_id: definition.id.into(),
                            profile_home: env.home.clone(),
                            executable: Some(executable),
                        },
                    );
                }
                progress(&result.snapshot);
                continue;
            }
            result.snapshot.providers.push(Provider {
                id: definition.id.into(),
                name: definition.name.into(),
                kind: "local_client".into(),
            });
            let mut roots = definition.roots(env);
            if !definition.has_override(env)
                && let Some(prefix) = definition.profile_prefix
            {
                roots.extend(
                    extra
                        .iter()
                        .filter(|p| {
                            p.file_name()
                                .is_some_and(|n| n.to_string_lossy().starts_with(prefix))
                        })
                        .cloned(),
                );
            }
            let default_root = roots.first().cloned();
            roots.sort();
            roots.dedup();
            if roots.len() > 16 {
                roots.truncate(16);
                result
                    .snapshot
                    .diagnostics
                    .push(format!("{}_PROFILE_LIST_PARTIAL", definition.id));
            }
            for (index, root) in roots.into_iter().enumerate() {
                cancelled(cancel)?;
                result.metadata_probes += 1;
                let mut config_present = match self.filesystem.probe(&root) {
                    Ok(EntryKind::Directory) => true,
                    Ok(EntryKind::Missing) => false,
                    Ok(_) => {
                        result
                            .snapshot
                            .diagnostics
                            .push(format!("{}_CONFIG_ROOT_UNSUPPORTED", definition.id));
                        false
                    }
                    Err(_) => {
                        result
                            .snapshot
                            .diagnostics
                            .push(format!("{}_CONFIG_METADATA_UNAVAILABLE", definition.id));
                        false
                    }
                };
                if !config_present
                    && definition.id == "opencode"
                    && root == env.data_home.join("opencode")
                {
                    result.metadata_probes += 1;
                    config_present = matches!(
                        self.filesystem.probe(&env.config_home.join("opencode")),
                        Ok(EntryKind::Directory)
                    );
                }
                if !config_present && (binaries.is_empty() || Some(&root) != default_root.as_ref())
                {
                    continue;
                }
                for (installation_index, executable) in executables.iter().cloned().enumerate() {
                    let binary_key = executable
                        .as_ref()
                        .map(|path| {
                            let logical = path_key(path);
                            identities
                                .get(path)
                                .map(|identity| {
                                    stable_id(
                                        "executable-binding",
                                        &[&logical, &format!("{identity:?}")],
                                    )
                                })
                                .unwrap_or(logical)
                        })
                        .unwrap_or_else(|| format!("configuration-only-{}", definition.id));
                    let installation_id = stable_id(
                        "installation",
                        &[&environment_id, definition.id, &binary_key],
                    );
                    // A profile belongs to its provider configuration root. Multiple command shims
                    // for the same root are installation candidates, not separate accounts.
                    let profile_id = stable_id(
                        "profile",
                        &[
                            &environment_id,
                            definition.id,
                            &path_key(&root),
                            &installation_id,
                        ],
                    );
                    let connection_id = stable_id("connection", &[&profile_id, &installation_id]);
                    if installations.insert(installation_id.clone()) {
                        result.snapshot.installations.push(Installation {
                            id: installation_id.clone(),
                            provider_id: definition.id.into(),
                            environment_id: environment_id.clone(),
                            location_hint: if executable.is_some() {
                                "Executable candidate present; not launched".into()
                            } else {
                                "Configuration only; no executable found".into()
                            },
                            version: Observation::unknown("VERSION_NOT_PROBED"),
                        });
                    }
                    let mut label = if Some(&root) == default_root.as_ref() {
                        "Default profile".into()
                    } else {
                        format!("Additional profile {}", index + 1)
                    };
                    if executables.len() > 1 {
                        label = format!("{label} · Installation {}", installation_index + 1);
                    }
                    result.snapshot.profiles.push(Profile {
                        id: profile_id.clone(),
                        installation_id: installation_id.clone(),
                        label: label.clone(),
                    });
                    result.snapshot.connections.push(Connection {
                        id: connection_id.clone(),
                        provider_id: definition.id.into(),
                        installation_id,
                        profile_id,
                        display_name: format!("{} · {label}", definition.name),
                        account_id: None,
                        auth: Observation::unknown("AUTH_NOT_CHECKED_PASSIVE_SCAN"),
                        plan: Observation::unknown("PLAN_NOT_REPORTED"),
                        status: if executable.is_some() {
                            "detected".into()
                        } else {
                            "configuration_only".into()
                        },
                        diagnostics: vec![if config_present {
                            "CONFIGURATION_DIRECTORY_PRESENT".into()
                        } else {
                            "NO_CONFIGURATION_DIRECTORY".into()
                        }],
                    });
                    result.snapshot.routes.push(AccessRoute {
                        id: stable_id("route", &[&connection_id]),
                        connection_id: connection_id.clone(),
                        provider_id: definition.id.into(),
                        display_name: format!("{} · {label}", definition.name),
                        billing: Observation::unknown("BILLING_NOT_CHECKED"),
                        capabilities: vec![],
                        catalog_revision: "unprobed".into(),
                    });
                    result.locations.insert(
                        connection_id,
                        LocatedClient {
                            executable_identity: executable
                                .as_ref()
                                .and_then(|path| identities.get(path))
                                .cloned(),
                            provider_id: definition.id.into(),
                            profile_home: root.clone(),
                            executable,
                        },
                    );
                }
            }
            result.snapshot.diagnostics.sort();
            result.snapshot.diagnostics.dedup();
            result.snapshot.revision = stable_id(
                "inventory",
                &[&result
                    .snapshot
                    .connections
                    .iter()
                    .map(|c| c.id.as_str())
                    .collect::<Vec<_>>()
                    .join("|")],
            );
            progress(&result.snapshot);
        }
        Ok(result)
    }
}
