use originlm_adapters::http_provider::config::*;
use originlm_contracts::*;
use originlm_discovery::ScanResult;
use serde_json::json;
use std::collections::BTreeMap;
fn config() -> serde_json::Value {
    json!({"endpoints":[{"id":"local","label":"Local model server","kind":"ollama","baseUrl":"http://127.0.0.1:11434/"}]})
}
#[test]
fn registered_endpoints_are_inventory_not_authenticated_accounts() {
    let registry = Registry::parse(&config().to_string(), "scope").unwrap();
    let mut scan = ScanResult {
        snapshot: Snapshot::empty(DataMode::Live),
        locations: BTreeMap::new(),
        metadata_probes: 0,
    };
    registry.inject(&mut scan);
    assert_eq!(scan.snapshot.connections.len(), 1);
    assert!(scan.snapshot.connections[0].auth.value().is_none());
    assert!(scan.snapshot.models.is_empty());
    assert!(scan.snapshot.accounts.is_empty());
    let value = serde_json::to_string(&scan.snapshot).unwrap();
    assert!(!value.contains("127.0.0.1"));
    assert_eq!(scan.metadata_probes, 0);
}
#[test]
fn configuration_rejects_inline_secrets_duplicate_names_and_bad_urls() {
    let mut secret = config();
    secret["endpoints"][0]["apiKey"] = json!("secret-sentinel");
    assert!(Registry::parse(&secret.to_string(), "s").is_err());
    let mut duplicate = config();
    let entry = duplicate["endpoints"][0].clone();
    duplicate["endpoints"].as_array_mut().unwrap().push(entry);
    assert!(Registry::parse(&duplicate.to_string(), "s").is_err());
    for url in [
        "http://remote.example/",
        "https://user:pass@example.com/",
        "http://127.0.0.1/?key=secret",
    ] {
        let mut value = config();
        value["endpoints"][0]["baseUrl"] = json!(url);
        assert!(Registry::parse(&value.to_string(), "s").is_err());
    }
}
#[test]
fn scope_and_endpoint_identity_are_part_of_connection_id() {
    let a = Registry::parse(&config().to_string(), "personal").unwrap();
    let b = Registry::parse(&config().to_string(), "work").unwrap();
    assert_ne!(a.entries.keys().next(), b.entries.keys().next());
    assert_eq!(
        a.entries.keys().next(),
        Registry::parse(&config().to_string(), "personal")
            .unwrap()
            .entries
            .keys()
            .next()
    );
}
#[test]
fn parsing_does_not_require_or_read_an_api_key() {
    let value = json!({"endpoints":[{"id":"api","label":"API","kind":"openai","baseUrl":"https://api.openai.com/v1/","keyEnv":"LAI_TEST_KEY_NOT_SET_01289"}]});
    let r = Registry::parse(&value.to_string(), "s").unwrap();
    assert_eq!(
        r.entries
            .values()
            .next()
            .unwrap()
            .endpoint()
            .err()
            .unwrap()
            .code,
        ErrorCode::AuthRequired
    );
}

#[test]
fn opencode_route_billing_configuration_is_explicit_and_transport_stays_local() {
    let value = json!({"endpoints":[{"id":"gateway","label":"OpenCode","kind":"opencode","baseUrl":"http://127.0.0.1:4096/","providerBilling":{"openai":"subscription","anthropic":"api_metered"}}]});
    let registry = Registry::parse(&value.to_string(), "host").unwrap();
    assert_eq!(registry.entries.len(), 1);
    let mut bad = value;
    bad["endpoints"][0]["baseUrl"] = json!("https://remote.example/");
    assert!(Registry::parse(&bad.to_string(), "host").is_err());
}
