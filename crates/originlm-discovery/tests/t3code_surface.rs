use originlm_discovery::*;
use originlm_platform::*;
use std::{fs, path::Path, sync::atomic::AtomicBool};

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

#[test]
fn pi_profile_follows_the_t3code_snapshot() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../upstream/t3code/snapshot.json");
    let snapshot: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(snapshot["source"]["commit"], T3CODE_COMMIT);
    assert_eq!(snapshot["source"]["repository"], T3CODE_REPOSITORY);

    let pi = definitions().iter().find(|item| item.id == "pi").unwrap();
    let commands: Vec<_> = snapshot["adopted"]["pi"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect();
    assert_eq!(pi.commands, commands.as_slice());

    let home = Path::new("/home/user");
    let env = ScanEnvironment::for_home(home.into(), HostPlatform::Linux);
    let profile = &profile_roots("pi", &env)[0];
    assert_eq!(
        profile,
        &join_relative(
            home,
            snapshot["adopted"]["pi"]["profileRelative"]
                .as_str()
                .unwrap()
        )
    );
    assert_eq!(
        session_stores("pi", profile)[0],
        join_relative(
            profile,
            snapshot["adopted"]["pi"]["sessionRelative"]
                .as_str()
                .unwrap()
        )
    );

    let custom = Path::new("/work/pi-agent");
    let mut overridden = env;
    overridden
        .overrides
        .insert("PI_CODING_AGENT_DIR".into(), custom.into());
    assert_eq!(profile_roots("pi", &overridden), vec![custom.to_path_buf()]);
    assert!(
        discovery_rules()
            .iter()
            .any(|rule| rule.id == "pi-agent" && rule.provider_id == "pi")
    );
}

#[test]
fn fx_profile_follows_the_t3code_snapshot_and_is_not_launched() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../upstream/t3code/snapshot.json");
    let snapshot: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let fx = definitions().iter().find(|item| item.id == "fx").unwrap();
    let commands: Vec<_> = snapshot["adopted"]["fx"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect();
    assert_eq!(fx.commands, commands.as_slice());
    assert_eq!(snapshot["adopted"]["fx"]["execution"], false);

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let bin = root.path().join("bin");
    fs::create_dir_all(home.join(".fx/skills/review")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        home.join(".fx/skills/review/SKILL.md"),
        b"DO_NOT_READ_FX_SKILL",
    )
    .unwrap();
    let binary = bin.join("fx");
    fs::write(&binary, b"DO_NOT_EXECUTE").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
    }
    let mut env = ScanEnvironment::for_home(home.clone(), HostPlatform::Linux);
    env.search_path = vec![bin];
    assert_eq!(
        profile_roots("fx", &env),
        vec![join_relative(
            &home,
            snapshot["adopted"]["fx"]["profileRelative"]
                .as_str()
                .unwrap()
        )]
    );
    assert!(session_stores("fx", &profile_roots("fx", &env)[0]).is_empty());
    assert!(
        discovery_rules()
            .iter()
            .any(|rule| rule.id == "fx-home" && rule.provider_id == "fx")
    );
    let result = Scanner::new(MetadataOnly)
        .scan(&env, &AtomicBool::new(false), |_| {})
        .unwrap();
    let connection = result
        .snapshot
        .connections
        .iter()
        .find(|connection| connection.provider_id == "fx")
        .unwrap();
    assert_eq!(connection.status, "detected");
    let location = result.locations.get(&connection.id).unwrap();
    assert_eq!(location.executable.as_deref(), Some(binary.as_path()));
    assert_eq!(fs::read_to_string(&binary).unwrap(), "DO_NOT_EXECUTE");
    assert!(
        !serde_json::to_string(&result.snapshot)
            .unwrap()
            .contains("DO_NOT_READ_FX_SKILL")
    );
}

#[test]
fn pi_configuration_and_off_path_opencode_are_inventory_only() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir_all(home.join(".pi/agent/sessions")).unwrap();
    fs::create_dir_all(home.join(".opencode/bin")).unwrap();
    let binary = home.join(".opencode/bin/opencode");
    fs::write(&binary, b"DO_NOT_EXECUTE").unwrap();
    let env = ScanEnvironment::for_home(home, HostPlatform::Linux);
    let result = Scanner::new(MetadataOnly)
        .scan(&env, &AtomicBool::new(false), |_| {})
        .unwrap();

    let pi = result
        .snapshot
        .connections
        .iter()
        .find(|connection| connection.provider_id == "pi")
        .unwrap();
    assert_eq!(pi.status, "configuration_only");
    assert!(
        result
            .locations
            .get(&pi.id)
            .is_some_and(|location| location.executable.is_none())
    );

    let opencode = result
        .locations
        .values()
        .find(|location| location.provider_id == "opencode")
        .unwrap();
    assert_eq!(opencode.executable.as_deref(), Some(binary.as_path()));
    assert_eq!(fs::read_to_string(&binary).unwrap(), "DO_NOT_EXECUTE");
}
