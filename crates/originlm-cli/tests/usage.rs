//! `originlm usage` (also `olm usage` from npm) shows plan quota for every
//! detected provider. The local session-history token totals it used to print
//! are kept, unchanged, behind `--history`: same reader, same JSON shape, so
//! nothing that consumed them is lost.
use std::{fs, io::Write, path::Path, process::Command};

fn write(path: &Path, lines: &[&str]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(path).unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
}

fn originlm(args: &[&str], home: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(args)
        .arg("--home")
        .arg(home)
        .output()
        .unwrap()
}

#[test]
fn history_reads_claude_and_codex_history_without_prompts_or_secrets() {
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
    let output = originlm(&["usage", "--history", "--json"], home.path());
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

    // Without --json, the same totals are readable lines.
    let output = originlm(&["usage", "--history"], home.path());
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("claude_code: input 10"), "{text}");
    assert!(!text.contains('{'), "{text}");
}

#[test]
fn history_on_an_empty_home_reports_no_providers() {
    let home = tempfile::tempdir().unwrap();
    let output = originlm(&["usage", "--history", "--json"], home.path());
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["providers"].as_array().map(Vec::len), Some(0));
}

#[test]
fn usage_on_an_empty_home_names_the_default_providers_as_not_found() {
    let home = tempfile::tempdir().unwrap();
    let output = originlm(&["usage", "--no-metadata-cache"], home.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("Claude Code: not found on this machine."),
        "{text}"
    );
    assert!(text.contains("Codex: not found on this machine."), "{text}");
    assert!(
        !text.contains('%') || text.contains("unknown is not zero"),
        "{text}"
    );
    assert!(!text.contains('{'), "terminal output is not JSON:\n{text}");

    let output = originlm(&["usage", "--json", "--no-metadata-cache"], home.path());
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["providers"], serde_json::json!([]));
    assert_eq!(
        value["notFound"],
        serde_json::json!(["claude_code", "codex"])
    );
}

#[test]
fn usage_finds_a_detected_provider_without_a_usage_source_and_lists_it() {
    // A Gemini CLI profile on disk is detected passively; OriginLM has no
    // usage source for it, so it is listed as such, never with a number.
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".gemini/settings.json"), &["{}"]);
    let output = originlm(&["usage", "--json", "--no-metadata-cache"], home.path());
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = value["providers"].as_array().unwrap();
    let gemini = rows
        .iter()
        .find(|row| row["providerId"] == "gemini_cli")
        .expect("Gemini CLI is detected from its profile directory");
    assert_eq!(gemini["usageSource"], "none");
    assert_eq!(gemini["data"]["state"], "no_source");
    assert_eq!(gemini["windows"], serde_json::json!([]));
    for row in rows {
        // Nothing detected in an isolated home was probed, so nothing has numbers.
        assert_eq!(row["windows"], serde_json::json!([]), "{row}");
    }
}

#[test]
fn history_takes_no_provider_grants() {
    let home = tempfile::tempdir().unwrap();
    for flags in [
        &["usage", "--history", "--allow-metadata", "codex"][..],
        &["usage", "--history", "--refresh"],
        &["usage", "--history", "--allow-identity"],
    ] {
        assert_eq!(
            originlm(flags, home.path()).status.code(),
            Some(2),
            "{flags:?}"
        );
    }
}
