use originlm_adapters::process::{LaunchSpec, codex_metadata};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
fn python() -> PathBuf {
    std::env::var_os("LAI_TEST_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                PathBuf::from("C:/Python313/python.exe")
            } else {
                PathBuf::from("/usr/bin/python3")
            }
        })
}
#[tokio::test]
async fn closing_native_peer_stops_owned_descendants_but_not_separate_processes() {
    let dir = tempfile::tempdir().unwrap();
    let owned = dir.path().join("owned");
    let unrelated = dir.path().join("unrelated");
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/process_descendant.py");
    let mut control_cmd = tokio::process::Command::new(python());
    control_cmd
        .args([
            script.as_os_str(),
            "heartbeat".as_ref(),
            unrelated.as_os_str(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    control_cmd.creation_flags(0x08000000);
    let mut control = control_cmd.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !unrelated.exists() {
            if let Some(status) = control.try_wait().unwrap() {
                panic!("Independent fixture exited before heartbeat: {status}");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("independent process must start before lifecycle assertion");
    let spec = LaunchSpec {
        executable: python(),
        args: vec![script.into(), "parent".into(), owned.clone().into()],
        cwd: dir.path().into(),
        env: BTreeMap::new(),
    };
    codex_metadata(spec, "c", "r", Duration::from_secs(2))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    let before = std::fs::metadata(&owned)
        .expect("owned child must start before close")
        .len();
    let other = std::fs::metadata(&unrelated).map(|m| m.len()).unwrap_or(0);
    // Wait for positive liveness evidence. A fixed 180ms window is scheduler
    // dependent when the workspace suites run alongside other development apps.
    tokio::time::timeout(Duration::from_secs(2), async {
        while std::fs::metadata(&unrelated).unwrap().len() <= other {
            assert!(
                control.try_wait().unwrap().is_none(),
                "independent process exited"
            );
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("independent process must remain alive");
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert_eq!(
        std::fs::metadata(&owned).map(|m| m.len()).unwrap_or(0),
        before,
        "owned descendant still writes after driver close"
    );
    assert!(
        std::fs::metadata(&unrelated).unwrap().len() > other,
        "independent process must remain alive"
    );
    control.kill().await.unwrap();
}
