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
