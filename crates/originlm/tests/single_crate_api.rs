//! Single-crate consumer fixture. This test imports only `originlm` plus std;
//! it proves synthetic inventory and scoped native configuration are usable
//! without depending on another OriginLM crate.

use originlm::{
    AccountScope, ClientPolicy, DataMode, InventorySource, LocalAi, Mode, NativeClientRegistry,
    NativeProfile, NativeProvider, ScanResult, ScopedInventory, ScopedProvider, Snapshot,
    StartOptions, scoped_inventory_source,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

#[test]
fn a_consumer_with_only_the_facade_can_build_synthetic_scope() {
    let inventory: InventorySource = Arc::new(|_cancel| {
        Ok(ScanResult {
            snapshot: Snapshot::empty(DataMode::Synthetic),
            locations: BTreeMap::new(),
            metadata_probes: 0,
        })
    });
    let mut native = NativeClientRegistry::default();
    native.profiles.insert(
        "fixture".into(),
        NativeProfile {
            id: "fixture".into(),
            label: "Fixture".into(),
            provider: NativeProvider::GeminiCli,
            executable: PathBuf::from("/fixture/bin/gemini"),
            profile_home: PathBuf::from("/fixture/home"),
            auth_method: None,
            billing: None,
        },
    );
    let mut ai = LocalAi::start_with(StartOptions {
        required_mode: Some(Mode::InProcess),
        inventory: Some(inventory),
        native_clients: native,
        policy: ClientPolicy::passive(),
        ..StartOptions::default()
    })
    .expect("the public facade must accept a synthetic scoped runtime");
    assert_eq!(ai.mode(), Mode::InProcess);
    let snapshot = ai.scan().expect("synthetic scan");
    assert_eq!(snapshot.mode, DataMode::Synthetic);
    assert!(
        snapshot
            .connections
            .iter()
            .any(|connection| connection.id == "fixture"),
        "host-declared native scope must reach the snapshot"
    );
    ai.close();
}

fn account_home(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

fn account_executable(root: &Path, name: &str) -> PathBuf {
    let path = root.join("bin").join(name).join(name);
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
fn single_facade_scopes_codex_claude_opencode_and_isolates_one_failure() {
    let root = tempfile::tempdir().unwrap();
    let codex_home = account_home(root.path(), "codex-home");
    let codex_exe = account_executable(root.path(), "codex");
    let claude_home = account_home(root.path(), "claude-home");
    let claude_exe = account_executable(root.path(), "claude");
    let opencode_home = account_home(root.path(), "opencode-home");
    let opencode_exe = account_executable(root.path(), "opencode");
    let broken_home = account_home(root.path(), "broken-home");
    let broken_exe = root.path().join("bin/missing/codex");

    let scoped = ScopedInventory::new()
        .with_account(AccountScope::new(
            ScopedProvider::Codex,
            codex_home.clone(),
            codex_exe.clone(),
        ))
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
    let scanned = scoped.scan(&AtomicBool::new(false)).unwrap();
    assert_eq!(scanned.snapshot.connections.len(), 3);
    for (provider, home, executable) in [
        (ScopedProvider::Codex, &codex_home, &codex_exe),
        (ScopedProvider::ClaudeCode, &claude_home, &claude_exe),
        (ScopedProvider::OpenCode, &opencode_home, &opencode_exe),
    ] {
        assert!(
            scanned.locations.values().any(|located| {
                located.provider_id == provider.id()
                    && &located.profile_home == home
                    && located.executable.as_ref() == Some(executable)
            }),
            "the facade must preserve the exact {} home/executable",
            provider.id()
        );
    }
    assert!(
        scanned
            .snapshot
            .diagnostics
            .iter()
            .any(|line| line == "codex_ACCOUNT_SCOPE_UNAVAILABLE"),
        "a failed account must be reported without suppressing the others"
    );

    let mut ai = LocalAi::start_with(StartOptions {
        required_mode: Some(Mode::InProcess),
        inventory: Some(scoped_inventory_source(scoped)),
        ..StartOptions::default()
    })
    .unwrap();
    let snapshot = ai.scan().unwrap();
    assert_eq!(snapshot.connections.len(), 3);
    let provider_ids: std::collections::BTreeSet<_> = snapshot
        .connections
        .iter()
        .map(|connection| connection.provider_id.as_str())
        .collect();
    assert!(provider_ids.contains("codex"));
    assert!(provider_ids.contains("claude_code"));
    assert!(provider_ids.contains("opencode"));
    ai.close();
}
