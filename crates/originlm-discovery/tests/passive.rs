use originlm_contracts::*;
use originlm_discovery::*;
use originlm_platform::*;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
struct ProbeSpy {
    probed: Arc<Mutex<Vec<PathBuf>>>,
}
impl LocalFilesystem for ProbeSpy {
    fn probe(&self, p: &Path) -> Result<EntryKind> {
        self.probed.lock().unwrap().push(p.into());
        NativeFilesystem.probe(p)
    }
    fn children(&self, p: &Path, n: usize) -> Result<DirectoryListing> {
        NativeFilesystem.children(p, n)
    }
    fn read_chunk(&self, _: &Path, _: u64, _: usize) -> Result<ReadChunk> {
        panic!("executable-only scan read file content")
    }
}
#[test]
fn executable_only_provider_never_probes_its_profiles_or_publishes_legacy_routes() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".codex-work")).unwrap();
    fs::create_dir_all(tmp.path().join(".codex")).unwrap();
    fs::create_dir_all(tmp.path().join(".claude")).unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let codex = bin.join("codex");
    fs::write(&codex, b"fixture").unwrap();
    let mut env = environment(tmp.path(), HostPlatform::Linux);
    env.search_path.push(bin);
    let probed = Arc::new(Mutex::new(Vec::new()));
    let spy = ProbeSpy {
        probed: probed.clone(),
    };
    let result = Scanner::new(spy)
        .with_executable_only_providers(BTreeSet::from(["codex".into()]))
        .scan(&env, &AtomicBool::new(false), |_| {})
        .unwrap();
    let probes = probed.lock().unwrap();
    assert!(
        !probes
            .iter()
            .any(|p| p == &env.home.join(".codex") || p == &env.home.join(".codex-work")),
        "executable-only provider profile root was probed"
    );
    assert!(probes.iter().any(|p| p == &env.home.join(".claude")));
    drop(probes);
    assert!(result.snapshot.providers.iter().all(|p| p.id != "codex"));
    assert!(
        result
            .snapshot
            .installations
            .iter()
            .all(|i| i.provider_id != "codex")
    );
    assert!(
        result
            .snapshot
            .connections
            .iter()
            .all(|c| c.provider_id != "codex")
    );
    assert!(
        result
            .snapshot
            .routes
            .iter()
            .all(|r| r.provider_id != "codex")
    );
    assert!(
        result
            .snapshot
            .connections
            .iter()
            .any(|c| c.provider_id == "claude_code")
    );
    let private: Vec<_> = result
        .locations
        .values()
        .filter(|c| c.provider_id == "codex")
        .collect();
    assert_eq!(private.len(), 1);
    assert_eq!(private[0].executable.as_deref(), Some(codex.as_path()));
    assert_eq!(private[0].profile_home, env.home);
}
struct MetadataOnly {
    reads: AtomicUsize,
}
impl LocalFilesystem for MetadataOnly {
    fn probe(&self, p: &Path) -> Result<EntryKind> {
        NativeFilesystem.probe(p)
    }
    fn children(&self, p: &Path, n: usize) -> Result<DirectoryListing> {
        NativeFilesystem.children(p, n)
    }
    fn read_chunk(&self, _: &Path, _: u64, _: usize) -> Result<ReadChunk> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        panic!("passive scan read file content")
    }
}
fn scan(env: &ScanEnvironment) -> ScanResult {
    Scanner::new(MetadataOnly {
        reads: AtomicUsize::new(0),
    })
    .scan(env, &AtomicBool::new(false), |_| {})
    .unwrap()
}
fn environment(root: &Path, platform: HostPlatform) -> ScanEnvironment {
    ScanEnvironment::for_home(root.into(), platform)
}
#[test]
fn empty_home_has_no_fabricated_connections_or_models() {
    let tmp = tempfile::tempdir().unwrap();
    let s = scan(&environment(tmp.path(), HostPlatform::Linux));
    assert!(s.snapshot.connections.is_empty());
    assert!(s.snapshot.models.is_empty());
    assert_eq!(s.snapshot.mode, DataMode::Live);
}
#[test]
fn credential_presence_is_unchecked_not_authentication_and_is_never_read() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".codex")).unwrap();
    fs::write(tmp.path().join(".codex/auth.json"), b"DO_NOT_READ_SECRET").unwrap();
    let s = scan(&environment(tmp.path(), HostPlatform::Linux));
    let c = s
        .snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "codex")
        .unwrap();
    assert_eq!(c.display_name, "Codex · Default profile");
    assert_ne!(c.auth.fresh_value(), Some(&AuthState::Authenticated));
    assert!(c.plan.value().is_none());
    assert!(s.snapshot.models.is_empty());
    assert!(
        !serde_json::to_string(&s.snapshot)
            .unwrap()
            .contains("DO_NOT_READ_SECRET")
    );
}
#[test]
fn user_profile_paths_do_not_escape_the_normalized_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("private-personal-name");
    fs::create_dir_all(home.join(".claude")).unwrap();
    let s = scan(&environment(&home, HostPlatform::Linux));
    assert!(
        !serde_json::to_string(&s.snapshot)
            .unwrap()
            .contains("private-personal-name")
    );
    assert_eq!(s.locations.len(), 1);
}
#[test]
fn custom_home_override_is_respected_without_mutating_either_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let custom = tmp.path().join("work 世界");
    fs::create_dir(&custom).unwrap();
    let mut env = environment(tmp.path(), HostPlatform::Linux);
    env.overrides.insert("CODEX_HOME".into(), custom.clone());
    let s = scan(&env);
    assert!(
        s.locations
            .values()
            .any(|l| l.provider_id == "codex" && l.profile_home == custom)
    );
    assert!(!tmp.path().join(".codex").exists());
}
#[test]
fn gemini_and_antigravity_are_distinct_routes() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".gemini/antigravity")).unwrap();
    let s = scan(&environment(tmp.path(), HostPlatform::Linux));
    let ids: Vec<_> = s
        .snapshot
        .connections
        .iter()
        .map(|c| c.provider_id.as_str())
        .collect();
    assert!(ids.contains(&"gemini_cli"));
    assert!(ids.contains(&"antigravity"));
}
#[test]
fn windows_shims_are_detected_as_candidates_without_execution() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin with spaces");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("codex.cmd"), b"SHOULD_NOT_EXECUTE").unwrap();
    let mut env = environment(tmp.path(), HostPlatform::Windows);
    env.search_path = vec![bin.clone(), bin];
    let s = scan(&env);
    assert_eq!(
        s.snapshot
            .connections
            .iter()
            .filter(|c| c.provider_id == "codex")
            .count(),
        1
    );
    assert!(s.locations.values().any(|l| {
        l.executable
            .as_ref()
            .is_some_and(|p| p.ends_with("codex.cmd"))
    }));
}

#[test]
fn multiple_windows_shims_for_one_profile_do_not_duplicate_the_profile() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".codex")).unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    for name in ["codex", "codex.cmd", "codex.exe"] {
        fs::write(bin.join(name), b"DO_NOT_EXECUTE").unwrap();
    }
    let mut env = environment(tmp.path(), HostPlatform::Windows);
    env.search_path = vec![bin.clone()];

    let result = scan(&env);
    let profiles: Vec<_> = result
        .snapshot
        .connections
        .iter()
        .filter(|connection| connection.provider_id == "codex")
        .collect();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].display_name, "Codex · Default profile");
    assert_eq!(result.snapshot.profiles.len(), 1);
    assert_eq!(
        result.locations[&profiles[0].id].executable.as_deref(),
        Some(bin.join("codex.exe").as_path())
    );
}
#[test]
fn separate_installation_directories_remain_selectable_connections() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".codex")).unwrap();
    let paths = [tmp.path().join("first"), tmp.path().join("second")];
    for path in &paths {
        fs::create_dir(path).unwrap();
        fs::write(path.join("codex.exe"), b"DO_NOT_EXECUTE").unwrap();
        fs::write(path.join("codex.cmd"), b"DO_NOT_EXECUTE").unwrap();
    }
    let mut env = environment(tmp.path(), HostPlatform::Windows);
    env.search_path = paths.to_vec();
    let result = scan(&env);
    assert_eq!(result.snapshot.installations.len(), 2);
    assert_eq!(result.snapshot.connections.len(), 2);
    for path in &paths {
        assert!(
            result
                .locations
                .values()
                .any(|location| location.executable.as_ref() == Some(&path.join("codex.exe")))
        );
    }
    assert_ne!(
        result.snapshot.connections[0].installation_id,
        result.snapshot.connections[1].installation_id
    );
}
#[test]
fn same_client_has_distinct_work_and_personal_profiles() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".codex")).unwrap();
    fs::create_dir(tmp.path().join(".codex-work")).unwrap();
    let s = scan(&environment(tmp.path(), HostPlatform::Linux));
    let profiles: Vec<_> = s
        .snapshot
        .connections
        .iter()
        .filter(|c| c.provider_id == "codex")
        .collect();
    assert_eq!(profiles.len(), 2);
    assert_ne!(profiles[0].id, profiles[1].id);
    assert!(profiles.iter().all(|c| c.account_id.is_none()));
}
#[test]
fn cancellation_before_scan_causes_no_successful_inventory() {
    let tmp = tempfile::tempdir().unwrap();
    let result = Scanner::new(MetadataOnly {
        reads: AtomicUsize::new(0),
    })
    .scan(
        &environment(tmp.path(), HostPlatform::Linux),
        &AtomicBool::new(true),
        |_| {},
    );
    assert_eq!(result.err().unwrap().code, ErrorCode::Cancelled);
}
#[test]
fn partial_results_are_emitted_progressively_and_ids_stay_stable() {
    let tmp = tempfile::tempdir().unwrap();
    for p in [".codex", ".claude", ".grok"] {
        fs::create_dir(tmp.path().join(p)).unwrap();
    }
    let env = environment(tmp.path(), HostPlatform::Linux);
    let mut progress = Vec::new();
    let s = Scanner::new(NativeFilesystem)
        .scan(&env, &AtomicBool::new(false), |snapshot| {
            progress.push(snapshot.connections.len())
        })
        .unwrap();
    assert!(
        progress
            .iter()
            .any(|n| *n > 0 && *n < s.snapshot.connections.len())
    );
    assert_eq!(
        s.snapshot
            .connections
            .iter()
            .map(|c| &c.id)
            .collect::<Vec<_>>(),
        scan(&env)
            .snapshot
            .connections
            .iter()
            .map(|c| &c.id)
            .collect::<Vec<_>>()
    );
}
#[test]
fn relative_and_network_profile_roots_are_rejected() {
    let mut env = ScanEnvironment::for_home(PathBuf::from("relative"), HostPlatform::Linux);
    assert!(
        Scanner::new(NativeFilesystem)
            .scan(&env, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    let tmp = tempfile::tempdir().unwrap();
    env = environment(tmp.path(), HostPlatform::Windows);
    env.overrides
        .insert("CODEX_HOME".into(), PathBuf::from(r"\\server\private"));
    assert!(
        Scanner::new(NativeFilesystem)
            .scan(&env, &AtomicBool::new(false), |_| {})
            .is_err()
    );
}

#[test]
fn another_providers_agent_binary_is_not_misidentified_as_cursor() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join(".grok/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("agent.exe"), b"DO_NOT_EXECUTE").unwrap();
    let mut env = environment(tmp.path(), HostPlatform::Windows);
    fs::create_dir_all(env.app_data.join("Cursor")).unwrap();
    env.search_path = vec![bin];
    let result = scan(&env);
    let cursor = result
        .snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "cursor")
        .unwrap();
    assert_eq!(cursor.status, "configuration_only");
    assert!(result.locations[&cursor.id].executable.is_none());
}
#[test]
fn cursor_named_binary_remains_a_passive_candidate() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join(".local/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("cursor-agent"), b"DO_NOT_EXECUTE").unwrap();
    let result = scan(&environment(tmp.path(), HostPlatform::Linux));
    assert!(
        result
            .locations
            .values()
            .any(|l| l.provider_id == "cursor" && l.executable.is_some())
    );
}

#[test]
fn opencode_xdg_config_and_data_directories_are_one_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let mut env = environment(tmp.path(), HostPlatform::Linux);
    fs::create_dir_all(env.config_home.join("opencode")).unwrap();
    fs::create_dir_all(env.data_home.join("opencode")).unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("opencode"), b"DO_NOT_EXECUTE").unwrap();
    env.search_path = vec![bin];

    let result = scan(&env);
    let profiles: Vec<_> = result
        .snapshot
        .connections
        .iter()
        .filter(|connection| connection.provider_id == "opencode")
        .collect();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].display_name, "OpenCode · Default profile");
}

#[test]
fn opencode_configuration_only_keeps_identity_when_data_directory_appears() {
    let tmp = tempfile::tempdir().unwrap();
    let env = environment(tmp.path(), HostPlatform::Linux);
    fs::create_dir_all(env.config_home.join("opencode")).unwrap();
    let before = scan(&env);
    assert_eq!(before.snapshot.connections.len(), 1);
    fs::create_dir_all(env.data_home.join("opencode")).unwrap();
    let after = scan(&env);
    assert_eq!(
        before.snapshot.connections[0].id,
        after.snapshot.connections[0].id
    );
}

#[test]
fn muse_is_found_in_its_windows_install_directory_without_reading_it() {
    let tmp = tempfile::tempdir().unwrap();
    let install = tmp.path().join("AppData/Local/Programs/muse");
    fs::create_dir_all(&install).unwrap();
    fs::write(install.join("muse.cmd"), b"SHOULD_NOT_EXECUTE").unwrap();
    fs::write(install.join(".muse-version"), b"1.4.7-R6101.1").unwrap();
    fs::create_dir_all(tmp.path().join(".config/muse")).unwrap();
    fs::write(
        tmp.path().join(".config/muse/auth.json"),
        b"DO_NOT_READ_SECRET",
    )
    .unwrap();
    let s = scan(&environment(tmp.path(), HostPlatform::Windows));
    let muse: Vec<_> = s
        .locations
        .values()
        .filter(|l| l.provider_id == "muse")
        .collect();
    assert_eq!(muse.len(), 1);
    assert_eq!(
        muse[0].executable.as_deref(),
        Some(install.join("muse.cmd").as_path())
    );
    assert_eq!(muse[0].profile_home, tmp.path().join(".config/muse"));
    let connection = s
        .snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "muse")
        .unwrap();
    assert_eq!(connection.display_name, "Muse Code · Default profile");
    assert_eq!(connection.status, "detected");
    assert_ne!(
        connection.auth.fresh_value(),
        Some(&AuthState::Authenticated)
    );
    assert!(
        !serde_json::to_string(&s.snapshot)
            .unwrap()
            .contains("DO_NOT_READ_SECRET")
    );
    // Linux has no such install directory.
    let linux = scan(&environment(tmp.path(), HostPlatform::Linux));
    assert!(
        linux
            .locations
            .values()
            .filter(|l| l.provider_id == "muse")
            .all(|l| l.executable.is_none())
    );
}

#[test]
fn a_configured_muse_binary_replaces_the_search() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("muse"), b"on path").unwrap();
    let custom = tmp.path().join("tools/muse-bin-1.4.7-R6101.1.exe");
    fs::create_dir_all(custom.parent().unwrap()).unwrap();
    fs::write(&custom, b"configured").unwrap();
    let mut env = environment(tmp.path(), HostPlatform::Linux);
    env.search_path.push(bin);
    env.overrides
        .insert(MUSE_BINARY_OVERRIDE.into(), custom.clone());
    let s = scan(&env);
    let muse: Vec<_> = s
        .locations
        .values()
        .filter(|l| l.provider_id == "muse")
        .collect();
    assert_eq!(muse.len(), 1);
    assert_eq!(muse[0].executable.as_deref(), Some(custom.as_path()));
    env.overrides
        .insert(MUSE_BINARY_OVERRIDE.into(), tmp.path().join("missing"));
    let s = scan(&env);
    assert!(
        s.snapshot
            .diagnostics
            .contains(&"muse_CONFIGURED_EXECUTABLE_UNAVAILABLE".to_owned())
    );
}
