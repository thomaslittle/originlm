use originlm_contracts::redact_diagnostic_detail;

#[test]
fn diagnostic_detail_retains_provider_reason_and_redacts_credentials() {
    assert_eq!(
        redact_diagnostic_detail("HTTP 401: Authorization: Bearer secret-value"),
        "HTTP 401: Authorization: Bearer [REDACTED]"
    );
    assert_eq!(
        redact_diagnostic_detail(
            r#"provider rejected: {"access_token":"secret-value","reason":"expired"}"#
        ),
        r#"provider rejected: {"access_token":"[REDACTED]","reason":"expired"}"#
    );
}

#[test]
fn diagnostic_detail_is_bounded_and_removes_control_characters() {
    let detail = format!("{}\u{1b}[31m", "x".repeat(5000));
    let sanitized = redact_diagnostic_detail(&detail);
    assert!(sanitized.len() <= 2048);
    assert!(!sanitized.chars().any(char::is_control));
}
