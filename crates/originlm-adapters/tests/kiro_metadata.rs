use originlm_adapters::kiro::probe;
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
#[cfg(unix)]
use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
fn fake_cli(home: &std::path::Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = home.join("bin/kiro-cli");
    fs::create_dir_all(bin.parent().unwrap()).unwrap();
    fs::write(&bin, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&bin).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&bin, permissions).unwrap();
    bin
}

#[cfg(unix)]
fn located(home: &std::path::Path, body: &str) -> LocatedClient {
    let executable = fake_cli(home, body);
    fs::create_dir_all(home.join(".kiro")).unwrap();
    LocatedClient {
        executable_identity: None,
        provider_id: "kiro".into(),
        profile_home: home.join(".kiro"),
        executable: Some(executable),
        scope: None,
    }
}

#[cfg(unix)]
fn catalog() -> String {
    r#"case "$1" in
--version) echo "kiro-cli 2.32.0" ;;
whoami) echo '{"accountType":"BuilderId","email":"dev@example.test"}' ;;
chat) echo '{"models":[{"model_id":"claude-sonnet-4.6","model_name":"Claude Sonnet 4.6"},{"model_id":"auto","model_name":"Auto"}],"default_model":"auto"}' ;;
esac"#
        .into()
}

#[cfg(unix)]
#[tokio::test]
async fn signed_in_metadata_keeps_native_model_ids_and_unknown_billing() {
    let home = tempfile::tempdir().unwrap();
    let report = probe(located(home.path(), &catalog()), "connection", "route")
        .await
        .unwrap();
    assert_eq!(report.version.value().unwrap(), "2.32.0");
    assert_eq!(
        report.auth.value(),
        Some(&AuthState::Authenticated),
        "whoami confirmed the session"
    );
    assert_eq!(
        report
            .account
            .as_ref()
            .unwrap()
            .display_label
            .value()
            .unwrap(),
        "dev@example.test"
    );
    assert_eq!(report.models.len(), 2);
    assert_eq!(report.models[0].native_id, "claude-sonnet-4.6");
    assert_eq!(report.models[1].native_id, "auto");
    assert!(report.models[1].is_default);
    assert!(report.billing.value().is_none(), "a login is not a plan");
    assert!(report.quota_pools.is_empty());
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("SECRET_SENTINEL"));
}

#[cfg(unix)]
#[tokio::test]
async fn signed_out_never_requests_the_account_model_catalog() {
    let home = tempfile::tempdir().unwrap();
    let marker = home.path().join("catalog-was-requested");
    let body = format!(
        r#"case "$1" in
--version) echo "kiro-cli 2.32.0" ;;
whoami) echo '{{"account":null}}'; exit 1 ;;
chat) touch "{}"; exit 0 ;;
esac"#,
        marker.display()
    );
    let report = probe(located(home.path(), &body), "connection", "route")
        .await
        .unwrap();
    assert_eq!(
        report.auth.value(),
        Some(&AuthState::Unauthenticated),
        "an explicit account:null is a signed-out observation"
    );
    assert!(report.models.is_empty());
    assert!(
        !marker.exists(),
        "signed-out model request starts a browser login"
    );
    assert!(
        report
            .diagnostics
            .contains(&"KIRO_MODEL_CATALOG_REQUIRES_SIGN_IN".into())
    );
}

#[cfg(unix)]
#[tokio::test]
async fn unknown_auth_is_not_reported_as_signed_out() {
    let home = tempfile::tempdir().unwrap();
    let body = r#"case "$1" in
--version) echo "kiro-cli 2.32.0" ;;
whoami) echo "error: service error"; exit 2 ;;
chat) exit 0 ;;
esac"#;
    let report = probe(located(home.path(), body), "connection", "route")
        .await
        .unwrap();
    assert!(
        report.auth.value().is_none(),
        "a failed whoami is not evidence of a signed-out account"
    );
    assert!(report.models.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn model_list_failure_is_explained_instead_of_showing_zero_models() {
    let home = tempfile::tempdir().unwrap();
    let body = r#"case "$1" in
--version) echo "kiro-cli 2.32.0" ;;
whoami) echo '{"accountType":"BuilderId","email":"dev@example.test"}' ;;
chat) echo "upstream failure"; exit 1 ;;
esac"#;
    let report = probe(located(home.path(), body), "connection", "route")
        .await
        .unwrap();
    assert_eq!(report.auth.value(), Some(&AuthState::Authenticated));
    assert!(report.models.is_empty());
    assert!(
        report
            .diagnostics
            .contains(&"KIRO_MODEL_CATALOG_UNAVAILABLE".into())
    );
}

#[cfg(unix)]
#[tokio::test]
async fn malformed_catalog_fails_closed_without_echoing_private_fields() {
    let home = tempfile::tempdir().unwrap();
    let body = r#"case "$1" in
--version) echo "kiro-cli 2.32.0" ;;
whoami) echo '{"accountType":"BuilderId","email":"dev@example.test"}' ;;
chat) echo '{"models":"not-an-array","token":"SECRET_SENTINEL_TOKEN"}' ;;
esac"#;
    let error = probe(located(home.path(), body), "connection", "route")
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ProtocolIncompatible);
    assert!(!error.to_string().contains("SECRET_SENTINEL"));
}

#[tokio::test]
async fn disconnected_connection_identity_is_rejected() {
    let located = LocatedClient {
        executable_identity: None,
        provider_id: "kiro".into(),
        profile_home: PathBuf::from("/tmp/kiro"),
        executable: Some(PathBuf::from("/tmp/kiro-cli")),
        scope: None,
    };
    let error = probe(located, "", "route").await.unwrap_err();
    assert_eq!(error.code, ErrorCode::ProviderUnavailable);
}
