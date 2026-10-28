#![cfg(unix)]
//! One isolated discovery -> metadata launch demonstration, without a provider account.
use originlm_discovery::{HostPlatform, ScanEnvironment, Scanner};
use originlm_kit::{ClientPolicy, Kit};
use originlm_platform::{LocalFilesystem, NativeFilesystem};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::AtomicBool,
};

#[tokio::test]
async fn bounded_unix_command_link_discovers_and_probes_without_relaxing_data_reads() {
    let base = std::env::var_os("LAI_SYMLINK_FIXTURE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let base = fs::canonicalize(base).unwrap();
    let scratch = tempfile::Builder::new()
        .prefix("originlm-command-link-")
        .tempdir_in(base)
        .unwrap();
    let home = scratch.path().join("home");
    let bin = home.join(".local/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();
    let target = scratch.path().join("codex-official-layout-fixture-v1");
    let log = scratch.path().join("peer-methods");
    let argv = serde_json::to_string(&vec![
        "fixture".to_string(),
        "valid".to_string(),
        log.to_string_lossy().into_owned(),
    ])
    .unwrap();
    let peer = format!(
        "#!/usr/bin/env python3\nimport sys\nsys.argv = {argv}\n{}",
        include_str!("../../originlm-adapters/tests/support/codex_peer.py")
    );
    fs::write(&target, peer).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&target, bin.join("codex-versioned")).unwrap();
    let command = bin.join("codex");
    symlink("codex-versioned", &command).unwrap();
    let env = ScanEnvironment::for_home(home.clone(), HostPlatform::Linux);
    let scan = || {
        Scanner::new(NativeFilesystem)
            .scan(&env, &AtomicBool::new(false), |_| {})
            .unwrap()
    };
    let found = scan();
    let connection = found
        .snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "codex")
        .unwrap();
    assert_eq!(
        connection.status, "detected",
        "versioned executable symlink became configuration-only"
    );
    let located = found.locations[&connection.id].clone();
    let binding = located
        .executable_identity
        .as_ref()
        .expect("no target identity captured");
    assert_eq!(binding.command, command);
    assert_eq!(binding.resolved, target);
    assert_eq!(
        located.executable.as_deref(),
        Some(command.as_path()),
        "logical command identity was lost"
    );
    assert_eq!(
        originlm_adapters::codex::resolve_executable(&command).unwrap(),
        target
    );
    assert!(
        originlm_adapters::codex::resolve_executable(&target).is_err(),
        "an arbitrary target name acquired Codex identity"
    );
    let mut managed = scan();
    originlm_adapters::native_clients::config::inject_managed_codex(
        &mut managed,
        &scratch.path().join("managed"),
    );
    let managed = managed
        .locations
        .values()
        .find(|c| c.provider_id == "codex_managed")
        .unwrap();
    assert_eq!(managed.executable.as_deref(), Some(command.as_path()));
    assert_eq!(managed.executable_identity.as_ref(), Some(binding));
    managed.validate_executable().unwrap();
    let kit = Kit::builder().home(home).build().unwrap();
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids.insert("codex".into());
    let app = kit.connect("unix-command-link-fixture", policy).unwrap();
    let snapshot = app.scan().await.unwrap();
    let id = &snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "codex")
        .unwrap()
        .id;
    assert!(!log.exists(), "passive discovery launched the peer");
    let snapshot = app.probe(id).await.unwrap();
    assert_eq!(snapshot.models.len(), 2);
    let methods = fs::read_to_string(&log).unwrap();
    assert!(methods.contains("account/read") && methods.contains("model/list"));
    assert!(!methods.contains("turn/") && !methods.contains("thread/"));
    app.close();
    assert!(
        NativeFilesystem.read_chunk(&command, 0, 64).is_err(),
        "executable support loosened data-read policy"
    );

    // Retarget after discovery: reject before launching a replacement peer.
    fs::remove_file(&command).unwrap();
    let replacement = scratch.path().join("different-valid-executable");
    fs::copy(&target, &replacement).unwrap();
    symlink(&replacement, &command).unwrap();
    let error =
        originlm_adapters::codex::probe(located.clone(), "fixture".into(), "fixture-route".into())
            .await
            .unwrap_err();
    assert_eq!(error.code, originlm_kit::ErrorCode::RuntimeMissing);
    assert_eq!(fs::read_to_string(&log).unwrap(), methods);
    fs::remove_file(&command).unwrap();
    symlink("codex-versioned", &command).unwrap();
    fs::rename(&replacement, &target).unwrap();
    assert!(
        located.validate_executable().is_err(),
        "replacement at the same resolved path was accepted"
    );
    fs::remove_file(&command).unwrap();
    symlink("missing-target", &command).unwrap();
    assert!(
        scan()
            .locations
            .values()
            .filter(|c| c.provider_id == "codex")
            .all(|c| c.executable.is_none())
    );
    let unavailable = || {
        assert!(
            scan()
                .locations
                .values()
                .filter(|c| c.provider_id == "codex")
                .all(|c| c.executable.is_none())
        )
    };
    fs::remove_file(&command).unwrap();
    symlink("codex", &command).unwrap();
    unavailable();
    fs::remove_file(&command).unwrap();
    for index in 0..33 {
        symlink(
            if index == 32 {
                target.clone()
            } else {
                bin.join(format!("deep-{}", index + 1))
            },
            bin.join(format!("deep-{index}")),
        )
        .unwrap();
    }
    symlink("deep-0", &command).unwrap();
    unavailable();
    fs::remove_file(&command).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, &command).unwrap();
    unavailable();
    fs::remove_file(&command).unwrap();
    symlink(scratch.path(), &command).unwrap();
    unavailable();
    let data = scratch.path().join("private-data");
    fs::create_dir(&data).unwrap();
    fs::write(data.join("config"), "SYNTHETIC-NOT-A-CREDENTIAL").unwrap();
    let alias = scratch.path().join("data-alias");
    symlink(data, &alias).unwrap();
    assert!(
        NativeFilesystem
            .read_chunk(&alias.join("config"), 0, 64)
            .is_err()
    );
}
