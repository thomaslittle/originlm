//! Live OpenCode 2 session creation through the discovered command shim.
//! Run with:
//! `OPENCODE_WRAPPER=/absolute/path/to/opencode.cmd cargo test -p originlm-adapters --test opencode_connect_live -- --ignored`
use originlm_adapters::opencode;
use originlm_contracts::*;
use std::path::Path;

fn model() -> Model {
    Model {
        id: "model_test".into(),
        route_id: "route_test".into(),
        native_id: "pulse-alpha-free".into(),
        name: "Pulse Alpha Free".into(),
        revision: "2.2.5".into(),
        is_default: true,
        capabilities: vec![],
        options: vec![],
        context_window: Observation::unknown("fixture"),
    }
}

#[tokio::test]
#[ignore = "requires a running OpenCode 2 service and a discovered wrapper"]
async fn wrapper_is_resolved_before_the_version_probe() {
    let wrapper = std::env::var("OPENCODE_WRAPPER").expect("OPENCODE_WRAPPER");
    let root = tempfile::tempdir().unwrap();
    let mut agent = opencode::open_session(
        Path::new(&wrapper),
        None,
        model(),
        "opencode".into(),
        root.path().into(),
        false,
        None,
        &[],
        false,
    )
    .await
    .expect("open session");
    let _ = agent.close().await;
}

#[tokio::test]
#[ignore = "requires a running OpenCode 2 service and a running host MCP server"]
async fn host_mcp_and_native_policy_open_a_working_session() {
    let wrapper = std::env::var("OPENCODE_WRAPPER").expect("OPENCODE_WRAPPER");
    let mcp_path = std::env::var("TWINHAUS_MCP_JSON").expect("TWINHAUS_MCP_JSON");
    let mcp: serde_json::Value =
        serde_json::from_slice(&std::fs::read(mcp_path).expect("mcp descriptor"))
            .expect("mcp json");
    let server = HostMcpServer {
        name: "twinhaus".into(),
        url: mcp["url"].as_str().expect("mcp url").into(),
        bearer_token: mcp["token"].as_str().expect("mcp token").into(),
        allowed_tools: vec!["home_read".into(), "home_apply".into(), "devices".into()],
    };
    let root = tempfile::tempdir().unwrap();
    let mut agent = opencode::open_session(
        Path::new(&wrapper),
        None,
        model(),
        "opencode".into(),
        root.path().into(),
        false,
        None,
        &[server],
        true,
    )
    .await
    .expect("open session with host MCP");
    let _ = agent.close().await;
}
