use std::process::Command;

fn scan(home: &std::path::Path, extra: &[&str]) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_originlm"));
    command.args(["scan", "--json", "--home"]).arg(home);
    command.args(extra);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn scan_locations_are_explicitly_opt_in() {
    let home = tempfile::tempdir().unwrap();
    let plain = scan(home.path(), &[]);
    assert!(plain.get("locations").is_none());
    assert!(plain.get("snapshot").is_none());
    assert!(plain.get("providers").is_some(), "plain scan is a snapshot");

    let detailed = scan(home.path(), &["--include-locations"]);
    assert!(detailed.get("snapshot").is_some());
    assert_eq!(
        detailed["locations"].as_array().map(Vec::len),
        Some(0),
        "an empty home has no private locations"
    );
    assert!(detailed.get("metadataProbes").is_some());
    let checked = detailed["checkedProviders"].as_array().unwrap();
    assert!(
        checked
            .iter()
            .any(|provider| provider["providerId"] == "kiro" && provider["status"] == "not_found"),
        "every known provider has a found/not_found status"
    );
}

#[test]
fn include_locations_is_only_accepted_for_scan() {
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["status", "--include-locations"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[cfg(unix)]
#[test]
fn probe_versions_is_explicit_and_returns_the_binary_output() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".codex")).unwrap();
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let codex = bin.join("codex");
    std::fs::write(&codex, "#!/bin/sh\necho 'codex 9.8.7'\n").unwrap();
    let mut permissions = std::fs::metadata(&codex).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&codex, permissions).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["scan", "--json", "--home"])
        .arg(home.path())
        .args(["--include-locations", "--probe-versions"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let versions = value["versions"].as_object().unwrap();
    let entry = versions.values().find(|entry| entry["ok"] == true).unwrap();
    assert!(entry["output"].as_str().unwrap().contains("9.8.7"));
}
