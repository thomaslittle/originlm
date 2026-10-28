use std::process::{Command, Stdio};
#[test]
fn generic_provider_flags_are_explicit_and_repeatable() {
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "serve",
            "--stdio",
            "--allow-metadata",
            "claude_code",
            "--allow-metadata",
            "codex",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn unsupported_provider_ids_fail_without_launch() {
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["serve", "--stdio", "--allow-metadata", "not-a-provider"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
}
#[test]
fn muse_metadata_and_execution_grants_are_accepted() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args([
            "serve",
            "--stdio",
            "--allow-metadata",
            "muse",
            "--allow-execution",
            "muse",
        ])
        .arg("--workspace")
        .arg(workspace.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
