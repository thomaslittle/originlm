use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_discovery::ScanResult;
use originlm_service::{InventorySource, NativeService};
use serde_json::{Value, json};
use std::{io::Write, sync::Arc, time::Duration};

fn empty_source() -> InventorySource {
    Arc::new(|_| {
        Ok(ScanResult {
            snapshot: Snapshot::empty(DataMode::Live),
            locations: Default::default(),
            metadata_probes: 0,
        })
    })
}

fn write(path: &std::path::Path, lines: &[&str]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = std::fs::File::create(path).unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
}

#[tokio::test]
async fn detection_status_explains_every_known_provider() {
    let home = tempfile::tempdir().unwrap();
    let service = NativeService::new(empty_source(), Duration::from_secs(2));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service.set_usage_home(home.path().to_path_buf());
    let report: Value = service
        .handle("owner", "detection.status", json!({}))
        .await
        .unwrap();
    let providers = report["providers"].as_array().unwrap();
    let kiro = providers
        .iter()
        .find(|provider| provider["providerId"] == "kiro")
        .unwrap();
    assert_eq!(kiro["status"], "not_found");
    assert!(!kiro["pathsChecked"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn local_models_are_served_from_the_configured_home() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home
            .path()
            .join(".ollama/models/manifests/registry.ollama.ai/library/llama3.2/latest"),
        &[r#"{"layers":[{"size":100}]}"#],
    );
    let service = NativeService::new(empty_source(), Duration::from_secs(2));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service.set_usage_home(home.path().to_path_buf());
    let models: Value = service
        .handle("owner", "models.local", json!({}))
        .await
        .unwrap();
    assert_eq!(models[0]["providerId"], "ollama");
    assert_eq!(models[0]["models"][0]["nativeId"], "llama3.2:latest");
    assert!(models[0]["roots"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn restricted_clients_do_not_read_unrelated_provider_surfaces() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".claude/projects/p/one.jsonl"),
        &[
            r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":2}}}"#,
        ],
    );
    write(
        &home
            .path()
            .join(".ollama/models/manifests/registry.ollama.ai/library/llama3.2/latest"),
        &[r#"{"layers":[{"size":100}]}"#],
    );
    let service = NativeService::new(empty_source(), Duration::from_secs(2));
    service
        .register_client(
            "restricted",
            ClientPolicy {
                connection_ids: Some(Default::default()),
                ..ClientPolicy::passive()
            },
        )
        .unwrap();
    service.set_usage_home(home.path().to_path_buf());

    let history: Value = service
        .handle("restricted", "usage.history", json!({}))
        .await
        .unwrap();
    assert!(history["providers"].as_array().unwrap().is_empty());
    let models: Value = service
        .handle("restricted", "models.local", json!({}))
        .await
        .unwrap();
    assert!(models.as_array().unwrap().is_empty());
    let status: Value = service
        .handle("restricted", "detection.status", json!({}))
        .await
        .unwrap();
    assert!(status["providers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn usage_history_is_served_from_the_configured_home_and_cached_in_costs() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".claude/projects/p/one.jsonl"),
        &[
            r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":2,"cache_read_input_tokens":90}}}"#,
        ],
    );
    write(
        &home.path().join(".gemini/tmp/p/chats/session-1.json"),
        &[
            r#"{"sessionId":"g1","messages":[{"model":"gemini-x","tokens":{"input":7,"output":3}}]}"#,
        ],
    );
    let service = NativeService::new(empty_source(), Duration::from_secs(2));
    service
        .register_client("owner", ClientPolicy::passive())
        .unwrap();
    service.set_usage_home(home.path().to_path_buf());
    let history: Value = service
        .handle("owner", "usage.history", json!({}))
        .await
        .unwrap();
    assert!(
        history["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["providerId"] == "gemini_cli")
    );
    let claude = history["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["providerId"] == "claude_code")
        .unwrap();
    assert_eq!(claude["totals"]["inputTokens"], 10);
    assert_eq!(claude["totals"]["cachedInputTokens"], 90);
    assert!(claude["roots"].as_array().unwrap().is_empty());
    let costs: Value = service
        .handle("owner", "costs.read", json!({}))
        .await
        .unwrap();
    assert!(
        costs["historical"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["totals"]["outputTokens"] == 2)
    );
    for provider in costs["historical"]["providers"].as_array().unwrap() {
        assert!(provider["roots"].as_array().unwrap().is_empty());
    }
}
