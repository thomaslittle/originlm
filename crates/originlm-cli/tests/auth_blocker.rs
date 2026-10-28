#[test]
fn metadata_browser_controller_never_opens_or_echoes_an_auth_url() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_originlm-auth-blocker"))
        .args(["https://accounts.example.invalid/?state=SECRET_SENTINEL"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
