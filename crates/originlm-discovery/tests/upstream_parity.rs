use originlm_discovery::*;
use originlm_platform::*;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
};

struct MetadataOnly;
impl LocalFilesystem for MetadataOnly {
    fn probe(&self, path: &Path) -> originlm_contracts::Result<EntryKind> {
        NativeFilesystem.probe(path)
    }
    fn children(&self, path: &Path, limit: usize) -> originlm_contracts::Result<DirectoryListing> {
        NativeFilesystem.children(path, limit)
    }
    fn read_chunk(&self, _: &Path, _: u64, _: usize) -> originlm_contracts::Result<ReadChunk> {
        panic!("passive scan read file content")
    }
}

fn repo_file(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn normals(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

fn contained(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn show(path: &Path) -> String {
    path.display().to_string()
}

fn environment(
    platform: HostPlatform,
    home: &Path,
    extra: &[(&str, &str)],
) -> (ScanEnvironment, BTreeMap<String, String>) {
    let mut vars = BTreeMap::new();
    let home_text = home.to_string_lossy().into_owned();
    match platform {
        HostPlatform::Windows => {
            vars.insert("USERPROFILE".into(), home_text);
        }
        HostPlatform::Macos | HostPlatform::Linux => {
            vars.insert("HOME".into(), home_text);
        }
    }
    for (key, value) in extra {
        vars.insert((*key).into(), (*value).into());
    }
    let env = ScanEnvironment::from_map(platform, &vars).unwrap();
    (env, vars)
}

fn tokscale_path(
    client: &Value,
    env: &ScanEnvironment,
    vars: &BTreeMap<String, String>,
) -> PathBuf {
    let root = &client["root"];
    let base = match root["kind"].as_str().unwrap() {
        "home" => env.home.clone(),
        "xdg-data" => env.data_home.clone(),
        "config" => join_relative(&env.home, ".config/tokscale"),
        "app-data" => env.app_data.clone(),
        "env" => {
            let name = root["var"].as_str().unwrap();
            let fallback = root["fallback"].as_str().unwrap();
            match vars.get(name).filter(|value| !value.trim().is_empty()) {
                Some(value) => PathBuf::from(value),
                None => join_relative(&env.home, fallback),
            }
        }
        other => panic!("unsupported Tokscale root kind {other}"),
    };
    join_relative(&base, client["relative"].as_str().unwrap())
}

#[test]
fn upstream_parity_matches_pinned_tokscale_discovery() {
    let oracle = std::env::var("LAI_UPSTREAM_ORACLE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_file("../../upstream/tokscale/snapshot.json"));
    let snapshot: Value = serde_json::from_str(&fs::read_to_string(&oracle).unwrap()).unwrap();
    let mappings: Value = serde_json::from_str(
        &fs::read_to_string(repo_file("../../upstream/tokscale/mappings.json")).unwrap(),
    )
    .unwrap();
    let lock: Value = serde_json::from_str(
        &fs::read_to_string(repo_file("../../upstream/tokscale/lock.json")).unwrap(),
    )
    .unwrap();
    if std::env::var_os("LAI_UPSTREAM_ORACLE").is_none() {
        assert_eq!(snapshot["commit"].as_str(), Some(TOKSCALE_COMMIT));
        assert_eq!(lock["commit"].as_str(), Some(TOKSCALE_COMMIT));
    }
    let clients = snapshot["clients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|client| (client["id"].as_str().unwrap().to_string(), client.clone()))
        .collect::<BTreeMap<_, _>>();
    for identifier in [
        "codex",
        "claude",
        "gemini",
        "antigravity-cli",
        "antigravity-extension",
        "opencode",
        "cursor",
        "copilot",
        "grok",
        "lmstudio",
    ] {
        assert!(
            mappings["clients"].get(identifier).is_some(),
            "{identifier} is no longer mapped"
        );
        assert!(
            clients.contains_key(identifier),
            "{identifier} left the snapshot"
        );
    }

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let codex_home = root.path().join("custom-codex");
    let claude_home = root.path().join("custom-claude");
    let gemini_home = root.path().join("custom-gemini");
    let grok_home = root.path().join("custom-grok");
    let lmstudio_home = root.path().join("custom-lmstudio");
    let xdg_data = root.path().join("xdg-data");
    let xdg_config = root.path().join("xdg-config");
    let app_data = root.path().join("roaming");
    let redirected = root.path().join("redirected-home");
    let profile = root.path().join("win-profile");
    let mut failures = Vec::new();
    let scenarios = [
        (
            "windows-normal",
            HostPlatform::Windows,
            home.as_path(),
            vec![],
        ),
        (
            "windows-appdata",
            HostPlatform::Windows,
            home.as_path(),
            vec![("APPDATA", app_data.to_string_lossy().into_owned())],
        ),
        (
            "windows-redirected-home",
            HostPlatform::Windows,
            profile.as_path(),
            vec![("HOME", redirected.to_string_lossy().into_owned())],
        ),
        ("macos-normal", HostPlatform::Macos, home.as_path(), vec![]),
        ("linux-normal", HostPlatform::Linux, home.as_path(), vec![]),
        (
            "linux-xdg",
            HostPlatform::Linux,
            home.as_path(),
            vec![
                ("XDG_DATA_HOME", xdg_data.to_string_lossy().into_owned()),
                ("XDG_CONFIG_HOME", xdg_config.to_string_lossy().into_owned()),
            ],
        ),
        (
            "custom-client-homes",
            HostPlatform::Linux,
            home.as_path(),
            vec![
                ("CODEX_HOME", codex_home.to_string_lossy().into_owned()),
                (
                    "CLAUDE_CONFIG_DIR",
                    claude_home.to_string_lossy().into_owned(),
                ),
                (
                    "GEMINI_CLI_HOME",
                    gemini_home.to_string_lossy().into_owned(),
                ),
                ("GROK_HOME", grok_home.to_string_lossy().into_owned()),
                (
                    "LM_STUDIO_HOME",
                    lmstudio_home.to_string_lossy().into_owned(),
                ),
            ],
        ),
        (
            "blank-environment-overrides",
            HostPlatform::Linux,
            home.as_path(),
            vec![
                ("CODEX_HOME", "   ".into()),
                ("CLAUDE_CONFIG_DIR", "".into()),
                ("GEMINI_CLI_HOME", " ".into()),
                ("GROK_HOME", "".into()),
                ("LM_STUDIO_HOME", "\t".into()),
            ],
        ),
    ];

    for (name, platform, scenario_home, extra) in &scenarios {
        let owned: Vec<(&str, String)> = extra
            .iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect();
        let pairs: Vec<(&str, &str)> = owned
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        let (env, vars) = environment(*platform, scenario_home, &pairs);
        if *name == "windows-redirected-home" {
            assert_eq!(env.home, redirected);
        }
        if *name == "blank-environment-overrides" {
            assert!(env.overrides.is_empty(), "{:?}", env.overrides);
        }
        for (identifier, mapping) in mappings["clients"].as_object().unwrap() {
            let Some(client) = clients.get(identifier) else {
                failures.push(format!(
                    "DISCOVERY PARITY FAILURE\n\nClient: {identifier}\nScenario: {name}\n\nTokscale snapshot has no client entry."
                ));
                continue;
            };
            let Some(adapter) = mapping["localAdapter"].as_str() else {
                continue;
            };
            let resolved = tokscale_path(client, &env, &vars);
            let roots = profile_roots(adapter, &env);
            let parity = mapping["parity"].as_str().unwrap_or("");
            let matches = roots.iter().any(|root| contained(&resolved, root));
            if parity == "contained" && !matches {
                failures.push(format!(
                    "DISCOVERY PARITY FAILURE\n\nClient: {identifier}\nPlatform: {platform:?}\nScenario: {name}\n\nTokscale resolves:\n{}\n\nOriginLM resolves:\n{}",
                    show(&resolved),
                    roots.iter().map(|root| show(root)).collect::<Vec<_>>().join("\n")
                ));
            }
            if parity == "intentionally-different" && matches {
                failures.push(format!(
                    "DISCOVERY PARITY FAILURE\n\nClient: {identifier}\nPlatform: {platform:?}\nScenario: {name}\n\nTokscale now resolves inside a OriginLM profile, but the mapping still says the paths are different:\n{}\n\nOriginLM resolves:\n{}",
                    show(&resolved),
                    roots.iter().map(|root| show(root)).collect::<Vec<_>>().join("\n")
                ));
            }
        }
        let ollama = &profile_roots("ollama", &env)[0];
        assert!(
            normals(ollama).ends_with(&[".ollama".into()]),
            "Ollama is local-only and must keep its own profile: {}",
            show(ollama)
        );
    }

    if !failures.is_empty() {
        panic!("{}", failures.join("\n\n"));
    }
}

#[test]
fn windows_redirect_ignores_posix_and_drive_relative_home() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    let mut vars = BTreeMap::from([("USERPROFILE".into(), profile.to_string_lossy().into_owned())]);
    vars.insert("HOME".into(), r"C:temp".into());
    let env = ScanEnvironment::from_map(HostPlatform::Windows, &vars).unwrap();
    assert_eq!(env.home, profile);
}

#[cfg(windows)]
#[test]
fn windows_redirect_ignores_git_bash_home() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().join("profile");
    let vars = BTreeMap::from([
        ("USERPROFILE".into(), profile.to_string_lossy().into_owned()),
        ("HOME".into(), "/home/runner".into()),
    ]);
    let env = ScanEnvironment::from_map(HostPlatform::Windows, &vars).unwrap();
    assert_eq!(env.home, profile);
}

#[cfg(windows)]
#[test]
fn windows_discovery_paths_keep_native_separators() {
    let env = ScanEnvironment::for_home(PathBuf::from(r"C:\Users\User"), HostPlatform::Windows);
    for provider in [
        "codex",
        "claude_code",
        "gemini_cli",
        "antigravity",
        "opencode",
        "cursor",
        "github_copilot",
        "ollama",
        "grok",
        "lmstudio",
        "pi",
        "fx",
    ] {
        for root in profile_roots(provider, &env) {
            let text = root.to_string_lossy();
            assert!(
                !text.contains('/'),
                "DISCOVERY PARITY FAILURE\n\nClient: {provider}\nPlatform: Windows\n\nOriginLM resolves:\n{text}"
            );
        }
    }
    let codex = &profile_roots("codex", &env)[0];
    assert_eq!(
        session_stores("codex", codex)[0].to_string_lossy(),
        r"C:\Users\User\.codex\sessions"
    );
    assert_eq!(
        normals(&profile_roots("cursor", &env)[0]),
        ["Users", "User", "AppData", "Roaming", "Cursor"]
    );
}

#[test]
fn platform_profile_components_stay_split() {
    let home = PathBuf::from("home");
    let windows = ScanEnvironment::for_home(home.clone(), HostPlatform::Windows);
    assert_eq!(
        normals(&profile_roots("cursor", &windows)[0]),
        ["home", "AppData", "Roaming", "Cursor"]
    );
    assert_eq!(
        normals(&profile_roots("opencode", &windows)[0]),
        ["home", ".local", "share", "opencode"]
    );
    let macos = ScanEnvironment::for_home(home.clone(), HostPlatform::Macos);
    assert_eq!(
        normals(&profile_roots("cursor", &macos)[0]),
        ["home", "Library", "Application Support", "Cursor"]
    );
    let linux = ScanEnvironment::for_home(home, HostPlatform::Linux);
    assert_eq!(
        normals(&profile_roots("cursor", &linux)[0]),
        ["home", ".config", "Cursor"]
    );
}

#[test]
fn adopted_homes_are_discovered_without_reading_session_files() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let gemini = root.path().join("gemini-home");
    fs::create_dir_all(home.join(".copilot")).unwrap();
    fs::create_dir_all(gemini.join("antigravity-cli/conversations")).unwrap();
    fs::create_dir_all(gemini.join("tmp")).unwrap();
    fs::write(
        home.join(".copilot/session-store.db"),
        b"DO_NOT_READ_SESSION_STORE",
    )
    .unwrap();
    let (mut env, _) = environment(HostPlatform::Linux, &home, &[]);
    env.overrides
        .insert("GEMINI_CLI_HOME".into(), gemini.clone());
    let result = Scanner::new(MetadataOnly)
        .scan(&env, &AtomicBool::new(false), |_| {})
        .unwrap();
    let homes = result
        .locations
        .values()
        .map(|client| (client.provider_id.clone(), client.profile_home.clone()))
        .collect::<Vec<_>>();
    assert!(
        homes
            .iter()
            .any(|(id, path)| id == "github_copilot" && path.ends_with(".copilot"))
    );
    assert!(
        homes
            .iter()
            .any(|(id, path)| id == "gemini_cli" && path == &gemini)
    );
    assert!(
        homes
            .iter()
            .any(|(id, path)| { id == "antigravity" && path.ends_with("antigravity-cli") })
    );
    assert!(
        !serde_json::to_string(&result.snapshot)
            .unwrap()
            .contains("DO_NOT_READ_SESSION_STORE")
    );
}

#[test]
fn secure_storage_override_stays_ahead_of_the_tokscale_claude_home() {
    let root = tempfile::tempdir().unwrap();
    let secure = root.path().join("secure");
    let config = root.path().join("claude-config");
    let (env, _) = environment(
        HostPlatform::Linux,
        &root.path().join("home"),
        &[
            ("CLAUDE_SECURESTORAGE_CONFIG_DIR", secure.to_str().unwrap()),
            ("CLAUDE_CONFIG_DIR", config.to_str().unwrap()),
        ],
    );
    assert_eq!(profile_roots("claude_code", &env), vec![secure]);
}
