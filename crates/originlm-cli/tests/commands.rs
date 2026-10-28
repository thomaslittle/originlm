use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_originlm")
}
#[test]
fn scan_calls_native_discovery_without_verifying_credentials() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".codex")).unwrap();
    let secret = home.path().join(".codex/auth.json");
    std::fs::write(&secret, "not even JSON: DO-NOT-READ-THIS").unwrap();
    let result = Command::new(binary())
        .args(["scan", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(snapshot["mode"], "live");
    let connection = snapshot["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["providerId"] == "codex")
        .unwrap();
    assert_eq!(connection["auth"]["state"], "unknown");
    assert!(snapshot["models"].as_array().unwrap().is_empty());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("DO-NOT-READ-THIS"));
    assert_eq!(
        std::fs::read_to_string(secret).unwrap(),
        "not even JSON: DO-NOT-READ-THIS"
    );
}
#[test]
fn executable_only_scan_omits_legacy_provider_but_keeps_other_profiles() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".local/bin")).unwrap();
    std::fs::create_dir_all(home.path().join(".codex")).unwrap();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    std::fs::write(home.path().join(".local/bin/codex.exe"), b"fixture").unwrap();
    let result = Command::new(binary())
        .args(["scan", "--json", "--home"])
        .arg(home.path())
        .args(["--executable-only-provider", "codex"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(
        snapshot["providers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["id"] != "codex")
    );
    assert!(
        snapshot["connections"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["providerId"] != "codex")
    );
    assert!(
        snapshot["routes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["providerId"] != "codex")
    );
    assert!(
        snapshot["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["providerId"] == "claude_code")
    );
}
#[test]
fn explicit_empty_home_does_not_fall_back_to_personal_profiles() {
    let home = tempfile::tempdir().unwrap();
    let result = Command::new(binary())
        .args(["scan", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    let snapshot: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(snapshot["connections"].as_array().unwrap().is_empty());
}
#[test]
fn unknown_or_incomplete_arguments_fail_without_actions() {
    for args in [
        vec!["serve"],
        vec!["scan", "--home"],
        vec!["--unknown"],
        vec!["scan", "--json", "--home", "relative"],
        vec!["serve", "--stdio", "--insecure"],
    ] {
        let result = Command::new(binary()).args(args).output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
#[test]
fn help_and_version_do_not_scan() {
    let help = Command::new(binary()).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--allow-passive-discovery"));
    let version = Command::new(binary()).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("0.1.0"));
}
#[test]
fn stdio_defaults_to_no_discovery_authorization() {
    let mut process = Command::new(binary())
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = process.stdin.take().unwrap();
    for (id, method) in [(1, "runtime.handshake"), (2, "discovery.refresh")] {
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":{}})
        )
        .unwrap();
    }
    drop(input);
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success());
    let frames: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(frames[1]["error"]["data"]["code"], "LOCAL_ACCESS_DENIED");
}

#[test]
fn codex_probe_is_advertised_without_running_it_during_startup() {
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(binary())
        .args([
            "serve",
            "--stdio",
            "--allow-passive-discovery",
            "--allow-codex-metadata",
            "--home",
        ])
        .arg(home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"runtime.handshake","params":{}})
    )
    .unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["result"]["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "connections.probe")
    );
}
