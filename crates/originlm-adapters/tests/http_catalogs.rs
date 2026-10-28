use originlm_adapters::http_provider::*;
use originlm_contracts::*;
use serde_json::json;
use std::collections::BTreeMap;
fn local(details: serde_json::Value) -> ConnectionMetadata {
    parse_catalog(
        HttpKind::Ollama,
        "connection",
        "route",
        json!({"models":[{"name":"native:model"},{"name":"embed"},{"name":"cloud"}]}),
        &BTreeMap::from([
            ("native:model".into(), details),
            ("embed".into(), json!({"capabilities":["embedding"]})),
            (
                "cloud".into(),
                json!({"remote_host":"https://cloud.invalid"}),
            ),
        ]),
        &[],
    )
    .unwrap()
}
#[test]
fn local_capabilities_are_metadata_not_model_name_guesses() {
    let m = local(
        json!({"capabilities":["completion","vision"],"model_info":{"general.architecture":"synthetic","synthetic.context_length":8192}}),
    );
    assert_eq!(m.models.len(), 2);
    assert!(m.diagnostics.iter().any(|s| s == "REMOTE_MODELS_EXCLUDED"));
    let model = m
        .models
        .iter()
        .find(|m| m.native_id == "native:model")
        .unwrap();
    assert_eq!(model.context_window.value().unwrap(), "8192");
    assert!(
        model
            .capabilities
            .iter()
            .any(|c| c.id == "generation.text" && c.support == Support::Supported)
    );
    assert!(
        !model
            .capabilities
            .iter()
            .any(|c| c.id == "generation.image")
    );
    let embedding = m.models.iter().find(|m| m.native_id == "embed").unwrap();
    assert!(
        embedding
            .capabilities
            .iter()
            .any(|c| c.id == "generation.text" && c.support == Support::Unsupported)
    );
}
#[test]
fn absent_details_are_not_fake_capabilities_or_allowances() {
    let m = local(json!({}));
    let model = &m.models[0];
    assert!(model.options.is_empty());
    assert!(model.context_window.value().is_none());
    assert!(m.quota_pools.is_empty());
    assert!(m.plan.value().is_none());
    assert!(
        !model
            .capabilities
            .iter()
            .any(|c| c.support == Support::Supported)
    );
}
#[test]
fn lmstudio_filters_embeddings_and_preserves_native_context() {
    let m=parse_catalog(HttpKind::Lmstudio,"c","r",json!({"models":[{"key":"llm-native","type":"llm","display_name":"Native","max_context_length":16384},{"key":"embed-native","type":"embedding"}]}),&BTreeMap::new(),&[]).unwrap();
    assert_eq!(m.models.len(), 1);
    assert_eq!(m.models[0].native_id, "llm-native");
    assert_eq!(m.models[0].context_window.value().unwrap(), "16384");
    assert_eq!(m.billing.value(), Some(&BillingMode::Local));
    assert!(
        m.auth.value().is_none(),
        "a reachable local model server is not an authenticated account"
    );
}
#[test]
fn duplicate_or_malformed_catalogs_fail_without_echoing_secrets() {
    for v in [
        json!({}),
        json!({"models":[{"name":"a"},{"name":"a"}]}),
        json!({"models":[{"name":"secret\nsentinel"}]}),
    ] {
        let e = parse_catalog(HttpKind::Ollama, "c", "r", v, &BTreeMap::new(), &[]).unwrap_err();
        assert!(!e.to_string().contains("sentinel"));
    }
}
#[test]
fn route_identity_is_retained_and_api_catalog_is_not_a_subscription() {
    let data = json!({"data":[{"id":"native","token":"secret-sentinel"}]});
    let a = parse_catalog(
        HttpKind::Openai,
        "a",
        "a",
        data.clone(),
        &BTreeMap::new(),
        &["native".into()],
    )
    .unwrap();
    let b = parse_catalog(
        HttpKind::Openai,
        "b",
        "b",
        data,
        &BTreeMap::new(),
        &["native".into()],
    )
    .unwrap();
    assert_ne!(a.models[0].id, b.models[0].id);
    assert_eq!(a.billing.value(), Some(&BillingMode::ApiMetered));
    assert!(a.plan.value().is_none());
    assert!(
        !serde_json::to_string(&a)
            .unwrap()
            .contains("secret-sentinel")
    );
}
#[test]
fn api_models_without_capability_evidence_are_not_automatically_eligible() {
    let m = parse_catalog(
        HttpKind::Openai,
        "c",
        "r",
        json!({"data":[{"id":"native"}]}),
        &BTreeMap::new(),
        &[],
    )
    .unwrap();
    assert!(
        m.models[0]
            .capabilities
            .iter()
            .any(|c| c.id == "generation.text" && c.support == Support::Unknown)
    );
}
#[test]
fn google_native_method_metadata_and_anthropic_efforts_are_preserved() {
    let g=parse_catalog(HttpKind::Gemini,"g","r",json!({"models":[{"name":"models/native","displayName":"Native","supportedGenerationMethods":["generateContent"],"inputTokenLimit":8192}]}),&BTreeMap::new(),&["models/native".into()]).unwrap();
    assert_eq!(g.models[0].native_id, "models/native");
    let a=parse_catalog(HttpKind::Anthropic,"a","r",json!({"data":[{"id":"native","display_name":"Native","capabilities":{"effort":{"supported":true,"future":{"supported":true},"low":{"supported":false}}},"max_input_tokens":100000}]}),&BTreeMap::new(),&[]).unwrap();
    assert_eq!(a.models[0].options.len(), 1);
    let c = &a.models[0].options[0];
    assert!(c.validate(&json!("future")).is_ok());
    assert!(c.validate(&json!("low")).is_err());
}
