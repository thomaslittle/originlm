use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn run(args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn explicit_endpoint_inventory_is_available_through_the_real_helper() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("endpoints.json");
    std::fs::write(&file,json!({"endpoints":[{"id":"local","label":"Configured local server","kind":"ollama","baseUrl":"http://127.0.0.1:65530/"}]}).to_string()).unwrap();
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"runtime.handshake\",\"params\":{\"protocolVersion\":\"1.0\"}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"discovery.refresh\",\"params\":{\"level\":\"passive\"}}\n";
    let output = run(
        &[
            "serve",
            "--stdio",
            "--allow-passive-discovery",
            "--endpoints",
            file.to_str().unwrap(),
            "--home",
            dir.path().to_str().unwrap(),
        ],
        input,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let scan = run(
        &[
            "scan",
            "--json",
            "--home",
            dir.path().to_str().unwrap(),
            "--endpoints",
            file.to_str().unwrap(),
        ],
        "",
    );
    assert!(scan.status.success());
    let snapshot: Value = serde_json::from_slice(&scan.stdout).unwrap();
    assert_eq!(snapshot["connections"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["models"].as_array().unwrap().len(), 0);
    assert!(!String::from_utf8_lossy(&scan.stdout).contains("65530"));
    assert!(!String::from_utf8_lossy(&scan.stdout).contains(file.to_str().unwrap()));
}
#[test]
fn metadata_permission_supports_registered_providers_without_starting_them() {
    for provider in [
        "ollama",
        "lmstudio",
        "openai_api",
        "anthropic_api",
        "gemini_api",
        "xai_api",
        "compatible_api",
    ] {
        let output = run(&["serve", "--stdio", "--allow-metadata", provider], "");
        assert!(output.status.success(), "{provider}");
    }
}
#[test]
fn config_with_inline_secret_is_rejected_without_echoing_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "{\"endpoints\":[],\"apiKey\":\"secret-sentinel\"}").unwrap();
    let output = run(
        &["scan", "--json", "--endpoints", path.to_str().unwrap()],
        "",
    );
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-sentinel"));
}
