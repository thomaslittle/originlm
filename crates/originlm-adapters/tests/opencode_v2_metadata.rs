//! OpenCode 2 metadata fixtures; no installed client or credential is used.
use axum::{Json, Router, extract::State, routing::get};
use originlm_adapters::opencode::v2_service;
use originlm_contracts::AuthState;
use originlm_discovery::ProfileScope;
use serde_json::{Value, json};

#[derive(Clone)]
struct Fixture {
    connections: Vec<Value>,
    limits: Value,
}

async fn server(connections: Vec<Value>, limits: Value) -> (String, tokio::task::JoinHandle<()>) {
    let state = Fixture {
        connections,
        limits,
    };
    let app = Router::new()
        .route(
            "/api/provider",
            get(|State(_): State<Fixture>| async {
                Json(json!({"data":[
                    {"id":"opencode","name":"OpenCode","activation":"enabled"},
                    {"id":"opencode-go","name":"OpenCode Go","activation":"enabled"}
                ]}))
            }),
        )
        .route(
            "/api/model",
            get(|State(_): State<Fixture>| async {
                Json(json!({"data":[
                    {"id":"console-model","providerID":"opencode","name":"Console model","enabled":true,"capabilities":{"input":["text"],"output":["text"],"tools":false},"variants":[],"limit":{}}
                ]}))
            }),
        )
        .route(
            "/api/integration/opencode",
            get(|State(s): State<Fixture>| async move {
                Json(json!({"data":{"id":"opencode","name":"OpenCode Console","connections":s.connections}}))
            }),
        )
        .route(
            "/api/experimental/usage/limits",
            get(|State(s): State<Fixture>| async move {
                Json(json!({"data":s.limits}))
            }),
        )
        .route(
            "/api/experimental/mcp/{alias}",
            get(|| async { axum::http::StatusCode::NOT_FOUND }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, task)
}

fn scope(root: &std::path::Path) -> ProfileScope {
    ProfileScope {
        config_home: root.join("config"),
        data_home: root.join("data"),
        state_home: root.join("state"),
    }
}

fn write_descriptor(root: &std::path::Path, url: &str) {
    let path = root.join("state/opencode/service.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        json!({
            "id": "fixture",
            "version": "2.2.5",
            "url": url.trim_end_matches('/'),
            "password": "fixture-password-for-tests",
        })
        .to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[tokio::test]
async fn console_oauth_connection_is_authenticated_without_workspace_limits() {
    let (url, server) = server(
        vec![json!({"type":"credential","id":"cred","label":"Personal","method":"oauth"})],
        json!({}),
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    write_descriptor(root.path(), &url);
    let metadata = v2_service::probe_scoped(
        Some(&scope(root.path())),
        "connection",
        "route",
        "opencode v2.2.5",
    )
    .await
    .unwrap();
    assert_eq!(
        metadata.auth.value(),
        Some(&AuthState::Authenticated),
        "console OAuth must not be reported as unconfigured"
    );
    assert!(
        metadata
            .diagnostics
            .iter()
            .any(|code| code == "OPENCODE_2_CONNECTION_REPORTED_NOT_UPSTREAM_VERIFIED")
    );
    assert!(
        metadata
            .models
            .iter()
            .any(|model| model.native_id == "console-model")
    );
    server.abort();
}

#[tokio::test]
async fn workspace_limits_remain_stronger_authentication_evidence() {
    let (url, server) = server(
        Vec::new(),
        json!({"workspace":{"id":"workspace-1"},"go":{"weekly":{"percent":10,"resetsAt":1790000000}}}),
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    write_descriptor(root.path(), &url);
    let metadata = v2_service::probe_scoped(
        Some(&scope(root.path())),
        "connection",
        "route",
        "opencode v2.2.5",
    )
    .await
    .unwrap();
    assert_eq!(metadata.auth.value(), Some(&AuthState::Authenticated));
    assert!(
        metadata
            .quota_pools
            .iter()
            .any(|pool| !pool.windows.is_empty())
    );
    assert!(
        !metadata
            .diagnostics
            .iter()
            .any(|code| code == "OPENCODE_2_CONNECTION_REPORTED_NOT_UPSTREAM_VERIFIED")
    );
    server.abort();
}

#[tokio::test]
async fn no_connection_remains_unknown() {
    let (url, server) = server(Vec::new(), json!({})).await;
    let root = tempfile::tempdir().unwrap();
    write_descriptor(root.path(), &url);
    let metadata = v2_service::probe_scoped(
        Some(&scope(root.path())),
        "connection",
        "route",
        "opencode v2.2.5",
    )
    .await
    .unwrap();
    assert!(metadata.auth.value().is_none());
    server.abort();
}
