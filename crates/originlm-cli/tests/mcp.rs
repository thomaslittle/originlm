//! Drives `originlm mcp` over stdio as an MCP client would.
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Client {
    fn start(home: &Path, extra: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_originlm"))
            .arg("mcp")
            .args(extra)
            .arg("--no-metadata-cache")
            .arg("--home")
            .arg(home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }
    fn send_line(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }
    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn request(&mut self, message: Value) -> Value {
        self.send_line(&message.to_string());
        self.read()
    }
    fn initialize(&mut self) {
        let reply = self.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}));
        assert_eq!(reply["result"]["serverInfo"]["name"], "originlm");
        assert_eq!(
            reply["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
        self.send_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    }
    fn call(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
            "params":{"name":name,"arguments":arguments}}))
    }
    fn finish(mut self) {
        drop(self.stdin);
        assert!(self.child.wait().unwrap().success());
    }
}

fn write(path: &Path, lines: &[&str]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, lines.join("\n") + "\n").unwrap();
}

#[test]
fn handshake_is_required_and_tools_are_listed() {
    let home = tempfile::tempdir().unwrap();
    let mut client = Client::start(home.path(), &[]);
    let early = client.request(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}));
    assert_eq!(early["error"]["code"], -32002);
    client.initialize();
    let tools = client.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].clone())
        .collect();
    assert_eq!(
        names,
        [json!("usage"), json!("providers"), json!("usage_history")]
    );
    let unknown = client.call(3, "nope", json!({}));
    assert_eq!(unknown["error"]["code"], -32602);
    client.send_line("{not json");
    assert_eq!(client.read()["error"]["code"], -32700);
    let ping = client.request(json!({"jsonrpc":"2.0","id":4,"method":"ping"}));
    assert_eq!(ping["result"], json!({}));
    client.finish();
}

#[test]
fn usage_on_an_empty_home_names_the_missing_providers_without_inventing_numbers() {
    let home = tempfile::tempdir().unwrap();
    let mut client = Client::start(home.path(), &[]);
    client.initialize();
    let reply = client.call(2, "usage", json!({}));
    let result = &reply["result"];
    assert_eq!(result["isError"], false, "{reply}");
    assert_eq!(result["structuredContent"]["providers"], json!([]));
    assert_eq!(
        result["structuredContent"]["notFound"],
        json!(["claude_code", "codex"])
    );
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("Claude Code: not found on this machine."),
        "{text}"
    );
    assert!(text.contains("Codex: not found on this machine."), "{text}");
    client.finish();
}

#[test]
fn usage_rejects_ungranted_providers_and_bad_arguments_as_tool_errors() {
    let home = tempfile::tempdir().unwrap();
    let mut client = Client::start(home.path(), &["--allow-metadata", "codex"]);
    client.initialize();
    let reply = client.call(2, "usage", json!({"providers":["claude_code"]}));
    assert_eq!(reply["result"]["isError"], true);
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("not granted"), "{text}");
    let reply = client.call(3, "usage", json!({"refresh":"yes"}));
    assert_eq!(reply["result"]["isError"], true);
    let reply = client.call(4, "providers", json!({"x":1}));
    assert_eq!(reply["result"]["isError"], true);
    client.finish();
}

#[test]
fn usage_history_reads_local_token_totals_without_prompts() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".claude/projects/p/one.jsonl"),
        &[
            r#"{"type":"assistant","sessionId":"c1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":90},"content":"PRIVATE_PROMPT"}}"#,
        ],
    );
    let mut client = Client::start(home.path(), &[]);
    client.initialize();
    let reply = client.call(2, "usage_history", json!({}));
    assert!(!reply.to_string().contains("PRIVATE_PROMPT"));
    let claude = &reply["result"]["structuredContent"]["providers"][0];
    assert_eq!(claude["providerId"], "claude_code");
    assert_eq!(claude["totals"]["outputTokens"], 5);
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("claude_code: input 10"), "{text}");
    client.finish();
}

#[test]
fn mcp_mode_takes_no_execution_or_serving_flags() {
    for flags in [
        &["--allow-execution", "codex"][..],
        &["--stdio"],
        &["--http"],
        &["--json"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
            .arg("mcp")
            .args(flags)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flags:?}");
    }
    let help = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("originlm mcp"));
}
