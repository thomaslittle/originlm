use std::{fs, io::Write, path::Path, process::Command};

fn write(path: &Path, lines: &[&str]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(path).unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
}

#[test]
fn usage_reads_claude_and_codex_history_without_prompts_or_secrets() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".claude/projects/private/one.jsonl"),
        &[
            r#"{"type":"assistant","sessionId":"c1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":2,"cache_read_input_tokens":90},"content":"PRIVATE_PROMPT"}}"#,
            r#"{"type":"assistant","sessionId":"c1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":90}}}"#,
        ],
    );
    write(
        &home.path().join(".codex/sessions/2026/one.jsonl"),
        &[
            r#"{"type":"session_meta","payload":{"id":"k1"},"secret":"PRIVATE_TOKEN"}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-x"}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":20,"cached_input_tokens":5,"output_tokens":7,"reasoning_output_tokens":1},"last_token_usage":{"input_tokens":20,"cached_input_tokens":5,"output_tokens":7,"reasoning_output_tokens":1}}}}"#,
        ],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["usage", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("PRIVATE_PROMPT"));
    assert!(!text.contains("PRIVATE_TOKEN"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let claude = value["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["providerId"] == "claude_code")
        .unwrap();
    assert_eq!(claude["totals"]["inputTokens"], 10);
    assert_eq!(claude["totals"]["cachedInputTokens"], 90);
    assert_eq!(claude["totals"]["outputTokens"], 5);
    let codex = value["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["providerId"] == "codex")
        .unwrap();
    assert_eq!(codex["totals"]["inputTokens"], 15);
    assert_eq!(codex["totals"]["cachedInputTokens"], 5);
    assert_eq!(codex["totals"]["outputTokens"], 7);
}

#[test]
fn usage_on_an_empty_home_reports_no_providers() {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["usage", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["providers"].as_array().map(Vec::len), Some(0));
}
