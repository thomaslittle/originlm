use std::{fs, io::Write, path::Path, process::Command};

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(path).unwrap();
    file.write_all(bytes).unwrap();
}

#[test]
fn models_lists_ollama_manifests_and_lmstudio_gguf_files() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home
            .path()
            .join(".ollama/models/manifests/registry.ollama.ai/library/llama3.2/latest"),
        br#"{"layers":[{"size":100}]}"#,
    );
    write(
        &home
            .path()
            .join(".lmstudio/models/infra-ai/ocelot/ocelot-q4_0.gguf"),
        b"gguf",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["models", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let providers = value["providers"].as_array().unwrap();
    let ollama = providers
        .iter()
        .find(|provider| provider["providerId"] == "ollama")
        .unwrap();
    assert_eq!(ollama["models"][0]["nativeId"], "llama3.2:latest");
    assert_eq!(ollama["models"][0]["sizeBytes"], 100);
    let lmstudio = providers
        .iter()
        .find(|provider| provider["providerId"] == "lmstudio")
        .unwrap();
    assert_eq!(
        lmstudio["models"][0]["nativeId"],
        "infra-ai/ocelot/ocelot-q4_0"
    );
    assert_eq!(lmstudio["models"][0]["format"], "gguf");
}

#[test]
fn models_reports_missing_roots_without_inventing_models() {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_originlm"))
        .args(["models", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for provider in value["providers"].as_array().unwrap() {
        assert_eq!(provider["models"].as_array().map(Vec::len), Some(0));
        assert!(
            !provider["diagnostics"].as_array().unwrap().is_empty(),
            "a missing root is explained"
        );
    }
}
