#![cfg(unix)]
//! Focused OpenCode account-scope metadata proof.
//!
//! The child process runs with synthetic global HOME/XDG roots and a default
//! OpenCode service A. Two official account scopes B and C point at separate
//! service descriptors and executables. `connections.probe` must return B/C
//! markers and contact A zero times. No credentials, providers or inference are
//! involved.

use originlm::{
    AccountScope, ClientPolicy, LocalAi, Mode, ScopedInventory, StartOptions,
    scoped_inventory_source,
};
use serde_json::json;
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

#[test]
fn opencode_scoped_metadata_never_uses_default_service() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("default-home");
    let state = root.path().join("default-state");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&state).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "opencode_scoped_child", "--nocapture"])
        .env("ORIGINLM_OPENCODE_CHILD", "1")
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", &home)
        .env("XDG_DATA_HOME", &state)
        .env("XDG_STATE_HOME", &state)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "scoped child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("opencode-scoped-child-ok"));
}

#[test]
fn opencode_scoped_child() {
    if std::env::var_os("ORIGINLM_OPENCODE_CHILD").is_none() {
        return;
    }
    let root = std::env::temp_dir().join(format!("originlm-opencode-scope-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let executable = root.join("bin/opencode");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(&executable, b"#!/bin/sh\necho 2.2.0\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();

    let default_contacts = Arc::new(AtomicUsize::new(0));
    let default_port = serve("A", Arc::clone(&default_contacts));
    let default_state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .expect("synthetic default state");
    write_descriptor(&default_state, default_port);

    let b_config = root.join("accounts/b/config");
    let b_data = root.join("accounts/b/data");
    let b_state = root.join("accounts/b/state");
    let c_config = root.join("accounts/c/config");
    let c_data = root.join("accounts/c/data");
    let c_state = root.join("accounts/c/state");
    for directory in [&b_config, &b_data, &b_state, &c_config, &c_data, &c_state] {
        fs::create_dir_all(directory).unwrap();
    }
    let b_contacts = Arc::new(AtomicUsize::new(0));
    let b_port = serve("B", Arc::clone(&b_contacts));
    write_descriptor(&b_state, b_port);
    let c_contacts = Arc::new(AtomicUsize::new(0));
    let c_port = serve("C", Arc::clone(&c_contacts));
    write_descriptor(&c_state, c_port);

    let scoped = ScopedInventory::new()
        .with_account(AccountScope::open_code(
            b_config.clone(),
            b_data.clone(),
            b_state.clone(),
            executable.clone(),
        ))
        .with_account(AccountScope::open_code(
            c_config.clone(),
            c_data.clone(),
            c_state.clone(),
            executable,
        ));
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids.insert("opencode".into());
    let mut ai = LocalAi::start_with(StartOptions {
        required_mode: Some(Mode::InProcess),
        inventory: Some(scoped_inventory_source(scoped)),
        policy,
        ..StartOptions::default()
    })
    .expect("scoped in-process runtime");
    let snapshot = ai.scan().expect("scoped discovery");
    let connections: Vec<String> = snapshot
        .connections
        .iter()
        .filter(|connection| connection.provider_id == "opencode")
        .map(|connection| connection.id.clone())
        .collect();
    assert_eq!(connections.len(), 2);
    for connection in &connections {
        ai.call("connections.probe", json!({"connectionId": connection}))
            .expect("scoped metadata probe");
    }
    let after = ai.snapshot().expect("post-probe snapshot");
    let names: BTreeSet<&str> = after
        .models
        .iter()
        .map(|model| model.name.as_str())
        .collect();
    assert!(names.contains("Model B"), "missing B metadata: {names:?}");
    assert!(names.contains("Model C"), "missing C metadata: {names:?}");
    assert!(
        !names.contains("Model A"),
        "default metadata leaked: {names:?}"
    );
    assert_eq!(
        default_contacts.load(Ordering::SeqCst),
        0,
        "a scoped probe must never contact the process-global default service"
    );
    assert!(b_contacts.load(Ordering::SeqCst) > 0);
    assert!(c_contacts.load(Ordering::SeqCst) > 0);
    ai.close();
    let _ = fs::remove_dir_all(&root);
    println!("opencode-scoped-child-ok");
}

fn write_descriptor(state_home: &Path, port: u16) {
    let directory = state_home.join("opencode");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("service.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "url": format!("http://127.0.0.1:{port}"),
            "password": "0123456789abcdef0123456789abcdef",
        }))
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn serve(marker: &'static str, contacts: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let contacts = Arc::clone(&contacts);
            thread::spawn(move || handle(stream, marker, contacts));
        }
    });
    port
}

fn handle(mut stream: TcpStream, marker: &'static str, contacts: Arc<AtomicUsize>) {
    let Ok(mut reader) = stream.try_clone().map(BufReader::new) else {
        return;
    };
    loop {
        let mut request = String::new();
        if reader.read_line(&mut request).unwrap_or(0) == 0 {
            return;
        }
        if request.trim().is_empty() {
            continue;
        }
        let path = request.split_whitespace().nth(1).unwrap_or("/");
        let mut header = String::new();
        loop {
            header.clear();
            if reader.read_line(&mut header).unwrap_or(0) == 0 {
                return;
            }
            if header == "\r\n" || header == "\n" {
                break;
            }
        }
        contacts.fetch_add(1, Ordering::SeqCst);
        let body = if path.starts_with("/api/provider") {
            json!({"data":[{"id": format!("marker-{marker}"), "name": format!("Marker {marker}"), "activation":"enabled"}]})
        } else if path.starts_with("/api/model") {
            json!({"data":[{"providerID": format!("marker-{marker}"), "id": "model", "name": format!("Model {marker}"), "enabled": true, "capabilities": {"input": ["text"], "output": ["text"], "tools": false}, "variants": [], "limit": {"context": 1000}}]})
        } else if path.starts_with("/api/integration/opencode") {
            json!({"data":{"connections":[]}})
        } else if path.starts_with("/api/experimental/usage/limits") {
            json!({"data":{"workspace":{"id": format!("workspace-{marker}")}}})
        } else {
            json!({})
        }
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
            body.len(),
            body
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return;
        }
        let _ = stream.flush();
    }
}
