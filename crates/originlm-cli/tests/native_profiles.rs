use serde_json::{Value, json};
use std::process::Command;
#[test]
fn native_profile_inventory_is_explicit_and_inert() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("native.json");
    std::fs::write(&path,json!({"profiles":[{"id":"g","label":"Configured Grok","provider":"grok","executable":dir.path().join("grok.exe"),"profileHome":dir.path().join(".grok"),"billing":"subscription"}]}).to_string()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "scan",
            "--json",
            "--home",
            dir.path().to_str().unwrap(),
            "--native-profiles",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s: Value = serde_json::from_slice(&out.stdout).unwrap();
    let c = s["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["providerId"] == "grok")
        .unwrap();
    assert_eq!(c["status"], "configured");
    assert_eq!(c["auth"]["state"], "unknown");
    assert!(s["models"].as_array().unwrap().is_empty());
    assert!(!String::from_utf8_lossy(&out.stdout).contains(path.to_str().unwrap()));
}
#[test]
fn native_profile_file_rejects_inline_credentials_without_echoing_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path,r#"{"profiles":[{"id":"g","label":"G","provider":"grok","executable":"C:\\x\\grok.exe","profileHome":"C:\\x\\.grok","token":"SECRET_SENTINEL"}]}"#).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "scan",
            "--json",
            "--native-profiles",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("SECRET_SENTINEL"));
}
#[test]
fn executable_only_provider_rejects_explicit_profile_for_same_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("native.json");
    std::fs::write(&path, json!({"profiles":[{"id":"g","label":"Grok","provider":"grok","executable":dir.path().join("grok.exe"),"profileHome":dir.path().join(".grok")}] }).to_string()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "scan",
            "--json",
            "--home",
            dir.path().to_str().unwrap(),
            "--native-profiles",
            path.to_str().unwrap(),
            "--executable-only-provider",
            "grok",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
