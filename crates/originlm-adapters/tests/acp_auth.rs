use originlm_adapters::{acp::client::inspect_authenticated, process::LaunchSpec};
use originlm_contracts::*;
use std::{collections::BTreeMap, path::PathBuf};
fn spec(mode: &str) -> LaunchSpec {
    LaunchSpec {
        executable: std::env::var_os("LAI_TEST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(if cfg!(windows) {
                    "C:/Python313/python.exe"
                } else {
                    "/usr/bin/python3"
                })
            }),
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/acp_peer.py")
                .into(),
            mode.into(),
        ],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}
#[tokio::test]
async fn explicit_advertised_auth_method_can_reuse_native_login_without_tokens() {
    let c = inspect_authenticated(spec("auth"), "fixture", "r", Some("oauth-personal"))
        .await
        .unwrap();
    assert_eq!(c.models.len(), 2);
}
#[tokio::test]
async fn missing_auth_is_not_treated_as_a_connected_subscription() {
    let e = inspect_authenticated(spec("auth"), "fixture", "r", None)
        .await
        .err()
        .unwrap();
    assert_eq!(e.code, ErrorCode::AuthRequired);
}
#[tokio::test]
async fn unadvertised_authentication_methods_are_not_sent_to_the_client() {
    let e = inspect_authenticated(
        spec("auth"),
        "fixture",
        "r",
        Some("unadvertised-future-method"),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(e.code, ErrorCode::OptionUnsupported);
}
#[tokio::test]
async fn auth_errors_are_sanitized_and_do_not_reveal_native_urls() {
    let e = inspect_authenticated(spec("auth_fail"), "fixture", "r", Some("oauth-personal"))
        .await
        .err()
        .unwrap();
    assert_eq!(e.code, ErrorCode::AuthRequired);
    assert!(!e.to_string().contains("SECRET_SENTINEL"));
}
#[tokio::test]
async fn existing_clients_without_auth_negotiation_still_use_the_same_driver() {
    let c = inspect_authenticated(spec("normal"), "fixture", "r", None)
        .await
        .unwrap();
    assert_eq!(c.models.len(), 2);
}
