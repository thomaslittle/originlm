use originlm_contracts::Result;
use originlm_discovery::*;
use originlm_platform::*;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

struct MetadataOnly;

impl LocalFilesystem for MetadataOnly {
    fn probe(&self, path: &Path) -> Result<EntryKind> {
        NativeFilesystem.probe(path)
    }
    fn children(&self, path: &Path, limit: usize) -> Result<DirectoryListing> {
        NativeFilesystem.children(path, limit)
    }
    fn read_chunk(&self, _: &Path, _: u64, _: usize) -> Result<ReadChunk> {
        panic!("passive scan read file content")
    }
    fn resolve_executable(&self, path: &Path) -> Result<ExecutableIdentity> {
        NativeFilesystem.resolve_executable(path)
    }
}

fn scan(env: &ScanEnvironment) -> ScanResult {
    Scanner::new(MetadataOnly)
        .scan(env, &AtomicBool::new(false), |_| {})
        .unwrap()
}

fn vars(platform: HostPlatform, home: &Path, extra: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let home = home.to_string_lossy().into_owned();
    match platform {
        HostPlatform::Windows => {
            map.insert("USERPROFILE".into(), home);
        }
        HostPlatform::Macos | HostPlatform::Linux => {
            map.insert("HOME".into(), home);
        }
    }
    for (key, value) in extra {
        map.insert((*key).into(), (*value).into());
    }
    map
}

fn write(path: impl AsRef<Path>) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"fixture").unwrap();
}

fn executable_for(result: &ScanResult, provider: &str) -> PathBuf {
    result
        .locations
        .values()
        .find(|located| located.provider_id == provider)
        .and_then(|located| located.executable.clone())
        .unwrap_or_else(|| panic!("{provider} was not detected"))
}

#[test]
fn bare_path_still_finds_user_level_version_manager_shims() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    write(home.join(".asdf/shims/codex"));
    write(home.join(".volta/bin/claude"));
    write(home.join(".local/share/mise/shims/grok"));
    let env = ScanEnvironment::from_map(
        HostPlatform::Linux,
        &vars(HostPlatform::Linux, &home, &[("PATH", "/usr/bin:/bin")]),
    )
    .unwrap();
    let result = scan(&env);
    assert_eq!(
        executable_for(&result, "codex"),
        home.join(".asdf/shims/codex")
    );
    assert_eq!(
        executable_for(&result, "claude_code"),
        home.join(".volta/bin/claude")
    );
    assert_eq!(
        executable_for(&result, "grok"),
        home.join(".local/share/mise/shims/grok")
    );
    assert!(
        result
            .snapshot
            .diagnostics
            .contains(&"codex_EXECUTABLE_FROM_FALLBACK_LOCATION".into())
    );
}

#[test]
fn bare_path_still_finds_homebrew_on_macos() {
    // /opt/homebrew cannot be created by a test, but the same fallback list
    // includes the per-user directories a Finder launch cannot see.
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    write(home.join(".nix-profile/bin/codex"));
    write(home.join("Library/pnpm/claude"));
    let env = ScanEnvironment::from_map(
        HostPlatform::Macos,
        &vars(
            HostPlatform::Macos,
            &home,
            &[("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")],
        ),
    )
    .unwrap();
    let result = scan(&env);
    assert_eq!(
        executable_for(&result, "codex"),
        home.join(".nix-profile/bin/codex")
    );
    assert_eq!(
        executable_for(&result, "claude_code"),
        home.join("Library/pnpm/claude")
    );
    // `valid_root` uses host path semantics, so a POSIX literal is only
    // absolute when the test host is a Unix host.
    if !cfg!(windows) {
        assert!(
            env.fallback_search_path
                .iter()
                .any(|path| path
                    == std::path::Path::new("/Applications/Ollama.app/Contents/Resources")),
            "the app-bundled Ollama CLI is searched without a shell PATH"
        );
    }
}

#[test]
fn windows_bare_path_finds_npm_winget_scoop_and_per_user_programs() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let app_data = home.join("AppData/Roaming");
    let local_app_data = home.join("AppData/Local");
    write(app_data.join("npm/claude.cmd"));
    write(local_app_data.join("Microsoft/WinGet/Links/grok.exe"));
    write(home.join("scoop/shims/opencode.exe"));
    write(local_app_data.join("Programs/Codex/codex.exe"));
    write(local_app_data.join("Kiro-Cli/kiro-cli.exe"));
    let program_files = tmp.path().join("Program Files");
    let program_data = tmp.path().join("ProgramData");
    write(program_data.join("chocolatey/bin/pi.exe"));
    let env = ScanEnvironment::from_map(
        HostPlatform::Windows,
        &vars(
            HostPlatform::Windows,
            &home,
            &[
                ("PATH", r"C:\Windows\System32"),
                ("ProgramFiles", &program_files.to_string_lossy()),
                ("ProgramData", &program_data.to_string_lossy()),
            ],
        ),
    )
    .unwrap();
    let result = scan(&env);
    assert_eq!(
        executable_for(&result, "claude_code"),
        app_data.join("npm/claude.cmd")
    );
    assert_eq!(
        executable_for(&result, "grok"),
        local_app_data.join("Microsoft/WinGet/Links/grok.exe")
    );
    assert_eq!(
        executable_for(&result, "opencode"),
        home.join("scoop/shims/opencode.exe")
    );
    assert_eq!(
        executable_for(&result, "codex"),
        local_app_data.join("Programs/Codex/codex.exe")
    );
    assert_eq!(
        executable_for(&result, "kiro"),
        local_app_data.join("Kiro-Cli/kiro-cli.exe")
    );
    assert_eq!(
        executable_for(&result, "pi"),
        program_data.join("chocolatey/bin/pi.exe")
    );
}

#[test]
fn inherited_path_keeps_working_and_is_deduplicated_with_fallbacks() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let path_bin = tmp.path().join("path-bin");
    write(path_bin.join("codex"));
    let env = ScanEnvironment::from_map(
        HostPlatform::Linux,
        &vars(
            HostPlatform::Linux,
            &home,
            &[("PATH", &path_bin.to_string_lossy())],
        ),
    )
    .unwrap();
    let result = scan(&env);
    assert_eq!(executable_for(&result, "codex"), path_bin.join("codex"));
    assert!(
        !result
            .snapshot
            .diagnostics
            .contains(&"codex_EXECUTABLE_FROM_FALLBACK_LOCATION".into()),
        "a PATH hit must not be reported as a fallback hit"
    );
}

#[test]
fn fallbacks_are_bounded_and_do_not_escape_local_absolute_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let env = ScanEnvironment::from_map(
        HostPlatform::Linux,
        &vars(
            HostPlatform::Linux,
            &home,
            &[("HOME", &home.to_string_lossy()), ("PATH", "/usr/bin:/bin")],
        ),
    )
    .unwrap();
    assert!(env.fallback_search_path.len() <= 256);
    assert!(
        env.fallback_search_path.iter().all(|path| {
            path.is_absolute() && !path.as_os_str().to_string_lossy().starts_with("//")
        }),
        "{:?}",
        env.fallback_search_path
    );
    assert!(env.fallback_search_path.contains(&home.join(".local/bin")));
}

#[test]
fn windows_program_files_kiro_is_found_without_local_appdata() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let program_files = tmp.path().join("Program Files");
    write(program_files.join("Kiro-Cli/kiro-cli.exe"));
    let env = ScanEnvironment::from_map(
        HostPlatform::Windows,
        &vars(
            HostPlatform::Windows,
            &home,
            &[
                ("PATH", r"C:\Windows\System32"),
                ("ProgramFiles", &program_files.to_string_lossy()),
            ],
        ),
    )
    .unwrap();
    let result = scan(&env);
    assert_eq!(
        executable_for(&result, "kiro"),
        program_files.join("Kiro-Cli/kiro-cli.exe")
    );
}
