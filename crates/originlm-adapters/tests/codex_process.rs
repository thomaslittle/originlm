use originlm_adapters::codex::{ManagedIdentity, managed_metadata};
use originlm_adapters::process::{LaunchSpec, codex_metadata};
use originlm_contracts::{ErrorCode, OptionControl};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
fn launch(mode: &str, dir: &Path) -> LaunchSpec {
    let py = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .expect("Python is required for protocol fixture tests");
    assert!(py.status.success());
    let executable = PathBuf::from(String::from_utf8(py.stdout).unwrap().trim());
    LaunchSpec {
        executable,
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/codex_peer.py")
                .into_os_string(),
            mode.into(),
            dir.join("methods.log").into_os_string(),
        ],
        cwd: dir.into(),
        env: BTreeMap::new(),
    }
}
#[tokio::test]
async fn native_protocol_reads_metadata_without_starting_threads_or_turns() {
    let tmp = tempfile::tempdir().unwrap();
    let report = codex_metadata(
        launch("normal", tmp.path()),
        "connection",
        "route",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(report.models.len(), 2);
    assert_eq!(report.quota_pools.len(), 1);
    let methods = std::fs::read_to_string(tmp.path().join("methods.log")).unwrap();
    assert_eq!(
        methods,
        "initialize\ninitialized\naccount/read\nmodel/list\nmodel/list\nconfig/read\naccount/rateLimits/read\n"
    );
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("DO-NOT-EXPORT")
    );
}
#[tokio::test]
async fn unauthenticated_client_does_not_fetch_model_or_usage_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let report = codex_metadata(
        launch("signed-out", tmp.path()),
        "c",
        "r",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert!(report.models.is_empty());
    let methods = std::fs::read_to_string(tmp.path().join("methods.log")).unwrap();
    assert!(!methods.contains("model/list"));
    assert!(!methods.contains("config/read"));
}

#[tokio::test]
async fn managed_identity_fetches_catalog_without_native_account_or_usage() {
    let tmp = tempfile::tempdir().unwrap();
    let report = managed_metadata(
        launch("managed", tmp.path()),
        "managed-connection",
        "managed-route",
        ManagedIdentity {
            app_name: "example-app".into(),
            subject: "subject-123".into(),
            client_id: "oaiapp_client".into(),
            email: Some("person@example.test".into()),
        },
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(report.models.len(), 2);
    assert!(report.quota_pools.is_empty());
    assert_eq!(
        report.billing.fresh_value(),
        Some(&originlm_contracts::BillingMode::Subscription)
    );
    let methods = std::fs::read_to_string(tmp.path().join("methods.log")).unwrap();
    assert!(methods.contains("model/list"));
    assert!(!methods.contains("account/rateLimits/read"));
}

#[tokio::test]
async fn managed_route_requires_codex_0157_or_newer() {
    let tmp = tempfile::tempdir().unwrap();
    let error = managed_metadata(
        launch("managed-old", tmp.path()),
        "c",
        "r",
        ManagedIdentity {
            app_name: "example-app".into(),
            subject: "subject".into(),
            client_id: "oaiapp_id".into(),
            email: None,
        },
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ProtocolIncompatible);
}
#[tokio::test]
async fn saved_fast_tier_selects_priority_and_does_not_add_a_tier() {
    let tmp = tempfile::tempdir().unwrap();
    let report = codex_metadata(
        launch("service-tier", tmp.path()),
        "connection",
        "route",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    let option = report.models[0]
        .options
        .iter()
        .find(|option| option.id == "serviceTier")
        .unwrap();
    assert_eq!(
        option.current_value.as_ref(),
        Some(&serde_json::json!("priority"))
    );
    let OptionControl::Select {
        choices,
        default_value,
    } = &option.control
    else {
        panic!("service tier is a select");
    };
    assert_eq!(default_value.as_deref(), Some("flex"));
    assert!(choices.iter().any(|choice| choice.value == "priority"));
    assert!(!choices.iter().any(|choice| choice.value == "fast"));
    assert!(report.models[1].options.is_empty());
}
#[tokio::test]
async fn quota_endpoint_failure_keeps_account_catalog_and_redacts_error() {
    let tmp = tempfile::tempdir().unwrap();
    let report = codex_metadata(
        launch("usage-error", tmp.path()),
        "c",
        "r",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(report.models.len(), 2);
    assert!(report.quota_pools.is_empty());
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("PRIVATE-TOKEN")
    );
}
#[tokio::test]
async fn repeated_pagination_cursor_fails_instead_of_looping() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        codex_metadata(launch("loop", tmp.path()), "c", "r", Duration::from_secs(5))
            .await
            .unwrap_err()
            .code,
        ErrorCode::ProtocolIncompatible
    );
}
#[tokio::test]
async fn unsolicited_execution_request_fails_closed() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        codex_metadata(
            launch("server-request", tmp.path()),
            "c",
            "r",
            Duration::from_secs(5)
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::IntegrationRestricted
    );
}
#[tokio::test]
async fn malformed_oversized_and_timed_out_peers_are_bounded() {
    for (mode, expected) in [
        ("malformed", ErrorCode::ProtocolIncompatible),
        ("huge", ErrorCode::FrameTooLarge),
        ("slow", ErrorCode::Timeout),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let error = codex_metadata(
            launch(mode, tmp.path()),
            "c",
            "r",
            if mode == "slow" {
                Duration::from_millis(500)
            } else {
                Duration::from_secs(5)
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, expected, "{mode}");
    }
}
#[tokio::test]
async fn relative_executable_and_loader_hook_environment_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let mut spec = launch("normal", tmp.path());
    spec.executable = "relative".into();
    assert!(
        codex_metadata(spec, "c", "r", Duration::from_secs(5))
            .await
            .is_err()
    );
    let mut spec = launch("normal", tmp.path());
    spec.env.insert("NODE_OPTIONS".into(), "arbitrary".into());
    assert!(
        codex_metadata(spec, "c", "r", Duration::from_secs(5))
            .await
            .is_err()
    );
    assert!(!tmp.path().join("methods.log").exists());
}

#[tokio::test]
async fn startup_account_notice_precedes_authoritative_account_read() {
    let tmp = tempfile::tempdir().unwrap();
    let report = codex_metadata(
        launch("initial-account", tmp.path()),
        "c",
        "r",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(report.models.len(), 2);
}
#[tokio::test]
async fn account_change_after_initial_read_invalidates_catalog_probe() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        codex_metadata(
            launch("changed-account", tmp.path()),
            "c",
            "r",
            Duration::from_secs(5)
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::SelectionStale
    );
}
#[tokio::test]
async fn token_refresh_during_probe_keeps_the_same_account() {
    let tmp = tempfile::tempdir().unwrap();
    let report = codex_metadata(
        launch("token-refresh", tmp.path()),
        "c",
        "r",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(report.models.len(), 2);
}
