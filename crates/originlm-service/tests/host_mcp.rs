#[path = "../../originlm-core/tests/common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::{LocatedClient, ScanResult};
use originlm_runtime::{Agent, BoxFuture, TurnContext};
use originlm_service::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};
const TOKEN: &str = "host-secret-token-0123456789abcdef-SENTINEL";
struct TestAgent;
impl Agent for TestAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, Value>,
        _: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
type Seen = Arc<Mutex<Vec<Vec<HostMcpServer>>>>;
async fn setup(allow_writes: bool) -> (Arc<NativeService>, Seen) {
    let seen: Seen = Arc::default();
    let record = seen.clone();
    let service = Arc::new(
        NativeService::new(
            Arc::new(|_| {
                let s = common::fixture();
                let locations = s
                    .connections
                    .iter()
                    .map(|c| {
                        (
                            c.id.clone(),
                            LocatedClient {
                                executable_identity: None,
                                provider_id: "test".into(),
                                profile_home: std::env::temp_dir(),
                                executable: None,
                                scope: None,
                            },
                        )
                    })
                    .collect();
                Ok(ScanResult {
                    snapshot: s,
                    locations,
                    metadata_probes: 0,
                })
            }),
            Duration::from_secs(60),
        )
        .with_execution(
            BTreeSet::from(["test".into()]),
            Arc::new(move |input| {
                record.lock().unwrap().push(input.host_mcp_servers.clone());
                Box::pin(async { Ok(Box::new(TestAgent) as Box<dyn Agent>) })
            }),
        ),
    );
    service
        .register_client("a", ClientPolicy::passive())
        .unwrap();
    service
        .register_client("ungranted", ClientPolicy::passive())
        .unwrap();
    service
        .register_execution(
            "a",
            ExecutionGrant {
                connection_ids: BTreeSet::from(["personal".into()]),
                workspaces: BTreeMap::from([("scratch".into(), std::env::temp_dir())]),
                allow_writes,
                ..Default::default()
            },
        )
        .unwrap();
    service
        .handle("a", "discovery.refresh", json!({}))
        .await
        .unwrap();
    (service, seen)
}
fn server() -> Value {
    json!({"name":"design","url":"http://127.0.0.1:48123/mcp","bearerToken":TOKEN,"allowedTools":["echo_color","set_fill"]})
}
fn create(servers: Value) -> Value {
    json!({"selection":common::selection(),"workspaceId":"scratch","hostMcpServers":servers})
}
#[tokio::test]
async fn valid_host_servers_reach_the_adapter_and_are_never_echoed() {
    let (service, seen) = setup(false).await;
    let ipv6 =
        json!({"name":"tools_2","url":"http://[::1]:9/","bearerToken":TOKEN,"allowedTools":["a"]});
    let session = service
        .handle("a", "sessions.create", create(json!([server(), ipv6])))
        .await
        .unwrap();
    assert!(!session.to_string().contains(TOKEN));
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].len(), 2);
    assert_eq!(seen[0][0].name, "design");
    assert_eq!(seen[0][0].bearer_token, TOKEN);
    assert_eq!(seen[0][0].allowed_tools, ["echo_color", "set_fill"]);
    let events = service
        .handle(
            "a",
            "events.read",
            json!({"sessionId":session["id"].as_str().unwrap()}),
        )
        .await
        .unwrap();
    assert!(!events.to_string().contains(TOKEN));
}
#[tokio::test]
async fn a_host_mcp_allowlist_of_130_tools_fits_session_create() {
    let (service, seen) = setup(false).await;
    let tools: Vec<String> = (0..130).map(|i| format!("tool_{i:03}")).collect();
    let mut host = server();
    host["allowedTools"] = json!(tools);
    service
        .handle("a", "sessions.create", create(json!([host])))
        .await
        .unwrap();
    assert_eq!(seen.lock().unwrap()[0][0].allowed_tools.len(), 130);
}
#[tokio::test]
async fn two_host_servers_can_share_a_256_tool_allowlist() {
    let (service, seen) = setup(false).await;
    let mut first = server();
    first["allowedTools"] = json!(
        (0..128)
            .map(|i| format!("first_{i:03}"))
            .collect::<Vec<_>>()
    );
    let mut second = server();
    second["name"] = json!("tools_2");
    second["allowedTools"] = json!(
        (0..128)
            .map(|i| format!("second_{i:03}"))
            .collect::<Vec<_>>()
    );
    service
        .handle("a", "sessions.create", create(json!([first, second])))
        .await
        .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].len(), 2);
    assert_eq!(seen[0][0].allowed_tools.len(), 128);
    assert_eq!(seen[0][1].allowed_tools.len(), 128);
}
#[tokio::test]
async fn omitted_host_servers_are_backwards_compatible() {
    let (service, seen) = setup(false).await;
    service
        .handle(
            "a",
            "sessions.create",
            json!({"selection":common::selection(),"workspaceId":"scratch"}),
        )
        .await
        .unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![Vec::<HostMcpServer>::new()]);
}
#[tokio::test]
async fn invalid_host_servers_are_refused_before_any_launch() {
    let (service, seen) = setup(false).await;
    let with = |key: &str, value: Value| {
        let mut s = server();
        s[key] = value;
        json!([s])
    };
    let many_tools: Vec<String> = (0..257).map(|i| format!("t{i}")).collect();
    let mut first = server();
    first["allowedTools"] = json!(
        (0..128)
            .map(|i| format!("first_{i:03}"))
            .collect::<Vec<_>>()
    );
    let mut second = server();
    second["name"] = json!("tools_2");
    second["allowedTools"] = json!(
        (0..129)
            .map(|i| format!("second_{i:03}"))
            .collect::<Vec<_>>()
    );
    let cases = [
        with("url", json!("https://127.0.0.1:48123/mcp")),
        with("url", json!("http://localhost:48123/mcp")),
        with("url", json!("http://127.0.0.2:48123/mcp")),
        with("url", json!("http://10.0.0.1:48123/mcp")),
        with("url", json!("http://127.0.0.1/mcp")),
        with("url", json!("http://127.0.0.1:0/mcp")),
        with("url", json!("http://127.0.0.1:70000/mcp")),
        with("url", json!("http://127.0.0.1:48123")),
        with("url", json!("http://127.0.0.1:48123@evil.test/mcp")),
        with("url", json!("http://[::1]:1/a b")),
        with("bearerToken", json!("short")),
        with("bearerToken", json!("x".repeat(513))),
        with("bearerToken", json!(format!("{TOKEN} with space"))),
        with("bearerToken", json!(format!("{TOKEN}\n"))),
        with("name", json!("Design")),
        with("name", json!("1design")),
        with("name", json!("design-tools")),
        with("name", json!("")),
        with("name", json!("a".repeat(33))),
        with("allowedTools", json!([])),
        with("allowedTools", json!(["Bash"])),
        with("allowedTools", json!(["echo-color"])),
        with("allowedTools", json!(["echo", "echo"])),
        with("allowedTools", json!(["a".repeat(65)])),
        with("allowedTools", json!(many_tools)),
        json!([first, second]),
        with("extra", json!(true)),
        json!([server(), server()]),
        json!([
            server(),
            {"name":"b","url":"http://127.0.0.1:1/","bearerToken":TOKEN,"allowedTools":["x"]},
            {"name":"c","url":"http://127.0.0.1:1/","bearerToken":TOKEN,"allowedTools":["x"]}
        ]),
    ];
    for servers in cases {
        let error = service
            .handle("a", "sessions.create", create(servers.clone()))
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{servers}");
        assert!(!error.message.contains(TOKEN));
    }
    assert!(seen.lock().unwrap().is_empty());
}
#[tokio::test]
async fn host_servers_require_the_execution_grant_and_reject_workspace_write() {
    let (service, seen) = setup(true).await;
    let ungranted = service
        .handle("ungranted", "sessions.create", create(json!([server()])))
        .await
        .unwrap_err();
    assert_eq!(ungranted.code, ErrorCode::LocalAccessDenied);
    let mut write = create(json!([server()]));
    write["permissionMode"] = json!("workspace_write");
    let error = service
        .handle("a", "sessions.create", write)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::IntegrationRestricted);
    assert!(seen.lock().unwrap().is_empty());
}
#[tokio::test]
async fn handshake_advertises_host_mcp() {
    let (service, _) = setup(false).await;
    let handshake = service
        .handle("a", "runtime.handshake", json!({}))
        .await
        .unwrap();
    assert!(
        handshake["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("session.host_mcp"))
    );
}
#[test]
fn debug_output_redacts_the_bearer_token() {
    let server: HostMcpServer = serde_json::from_value(server()).unwrap();
    let debug = format!("{server:?}");
    assert!(!debug.contains(TOKEN));
    assert!(debug.contains("[redacted]"));
    let params: SessionCreateParams = serde_json::from_value(create(json!([server]))).unwrap();
    assert!(!format!("{params:?}").contains(TOKEN));
}
