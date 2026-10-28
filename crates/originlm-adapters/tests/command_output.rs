use originlm_adapters::process::{LaunchSpec, capture};
use originlm_contracts::ErrorCode;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
fn spec(code: &str) -> LaunchSpec {
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
        args: vec!["-c".into(), code.into()],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}
#[tokio::test]
async fn collects_utf8_status_without_shell() {
    assert_eq!(
        capture(spec("print('native status')"), Duration::from_secs(2))
            .await
            .unwrap()
            .trim(),
        "native status"
    );
}
#[tokio::test]
async fn failed_command_does_not_return_error_text() {
    let e = capture(
        spec("import sys;print('secret-sentinel');sys.exit(2)"),
        Duration::from_secs(2),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::ProviderUnavailable);
    assert!(!e.to_string().contains("secret-sentinel"));
}
#[tokio::test]
async fn large_output_and_timeout_are_bounded() {
    assert_eq!(
        capture(spec("print('x'*1200000)"), Duration::from_secs(2))
            .await
            .unwrap_err()
            .code,
        ErrorCode::FrameTooLarge
    );
    assert_eq!(
        capture(
            spec("import time;time.sleep(60)"),
            Duration::from_millis(100)
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::Timeout
    );
}
