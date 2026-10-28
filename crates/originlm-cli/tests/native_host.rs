use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn run(origin: &str) -> (std::process::Output, tempfile::TempDir) {
    // Two integration tests copy the same test binary. On Linux a concurrent
    // execution can briefly report ETXTBSY; serialize the copy/spawn window.
    static HOST_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = HOST_TEST
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let host = dir.path().join(if cfg!(windows) {
        "originlm-native-host.exe"
    } else {
        "originlm-native-host"
    });
    let mut copied = false;
    for _ in 0..20 {
        match std::fs::copy(env!("CARGO_BIN_EXE_originlm-native-host"), &host) {
            Ok(_) => {
                copied = true;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => panic!("could not prepare native host test binary: {error}"),
        }
    }
    assert!(copied, "native host test binary stayed busy");
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    let cfg = json!({"allowedOrigins":["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"],"args":["--allow-passive-discovery","--home",home]});
    std::fs::write(host.with_extension("json"), cfg.to_string()).unwrap();
    let mut p = Command::new(host)
        .arg(origin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = p.stdin.take().unwrap();
    for v in [
        json!({"jsonrpc":"2.0","id":1,"method":"runtime.handshake","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"discovery.refresh","params":{}}),
    ] {
        let b = serde_json::to_vec(&v).unwrap();
        let _ = input.write_all(&(b.len() as u32).to_ne_bytes());
        let _ = input.write_all(&b);
    }
    drop(input);
    (p.wait_with_output().unwrap(), dir)
}
#[test]
fn registered_host_reaches_native_service_without_installing_browser_entries() {
    let (out, _dir) = run("chrome-extension://abcdefghijklmnopabcdefghijklmnop/");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let size = u32::from_ne_bytes(out.stdout[..4].try_into().unwrap()) as usize;
    let v: Value = serde_json::from_slice(&out.stdout[4..4 + size]).unwrap();
    assert_eq!(v["result"]["protocolVersion"], "1.0");
}
#[test]
fn origin_is_checked_again_even_when_manifest_would_normally_filter_it() {
    let (out, _dir) = run("chrome-extension://pppppppppppppppppppppppppppppppp/");
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}
