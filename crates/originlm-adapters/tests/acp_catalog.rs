use originlm_adapters::acp::*;
use originlm_contracts::*;
use serde_json::{Value, json};
fn init() -> Value {
    json!({"protocolVersion":1,"agentInfo":{"name":"test","version":"1.0"},"agentCapabilities":{"promptCapabilities":{"image":true}}})
}
fn session() -> Value {
    json!({"sessionId":"s","configOptions":[{"id":"native-model","name":"Model","category":"model","type":"select","currentValue":"native/thinking-x","options":[{"group":"new","name":"New","options":[{"value":"native/thinking-x","name":"Native X"},{"value":"native-y","name":"Native Y"}]}]},{"id":"effort-native","name":"Thought","category":"thought_level","type":"select","currentValue":"future","options":[{"value":"future","name":"Future"},{"value":"tiny","name":"Tiny"}]}]})
}
#[test]
fn preserves_native_ids_groups_and_thought_options() {
    let c = parse_catalog("test", "r", &init(), &session()).unwrap();
    assert_eq!(c.models.len(), 2);
    assert_eq!(c.models[0].native_id, "native/thinking-x");
    assert!(c.models[0].is_default);
    assert_eq!(c.models[0].options[0].id, "effort-native");
    c.models[0].options[0].validate(&json!("future")).unwrap();
    assert!(c.models[0].options[0].validate(&json!("high")).is_err());
}
#[test]
fn prefers_config_options_over_legacy_modes_and_models() {
    let mut s = session();
    s["models"] = json!({"currentModelId":"obsolete","availableModels":[{"modelId":"obsolete","name":"Old"}]});
    s["modes"] = json!({"currentModeId":"unsafe","availableModes":[]});
    let c = parse_catalog("test", "r", &init(), &s).unwrap();
    assert_eq!(c.models.len(), 2);
    assert!(
        !serde_json::to_string(&c.models)
            .unwrap()
            .contains("obsolete")
    );
}
#[test]
fn omitted_capabilities_are_unsupported_not_enabled() {
    let c = parse_catalog("test", "r", &json!({"protocolVersion":1}), &session()).unwrap();
    assert!(!c.load_session);
    let cap = c.models[0]
        .capabilities
        .iter()
        .find(|c| c.id == "input.image")
        .unwrap();
    assert_eq!(cap.support, Support::Unsupported);
    assert!(
        !c.models[0]
            .capabilities
            .iter()
            .any(|c| c.id == "generation.image")
    );
}
#[test]
fn unknown_control_type_is_inspectable_not_editable() {
    let mut s = session();
    s["configOptions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"new-control","name":"Future","type":"hologram","currentValue":"yes"}));
    let c = parse_catalog("test", "r", &init(), &s).unwrap();
    let d = c.models[0]
        .options
        .iter()
        .find(|d| d.id == "new-control")
        .unwrap();
    assert!(matches!(d.control, OptionControl::Unknown { .. }));
    assert!(d.validate(&json!("yes")).is_err());
}
#[test]
fn mode_and_unclassified_controls_cannot_disable_permissions_silently() {
    let mut s = session();
    s["configOptions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"brave","name":"Brave mode","type":"boolean","currentValue":false}));
    let c = parse_catalog("test", "r", &init(), &s).unwrap();
    assert!(
        c.models[0]
            .options
            .iter()
            .find(|d| d.id == "brave")
            .unwrap()
            .permission_sensitive
    );
}
#[test]
fn malformed_versions_duplicate_ids_and_values_fail() {
    assert!(parse_catalog("test", "r", &json!({"protocolVersion":9}), &session()).is_err());
    let mut s = session();
    let duplicate = s["configOptions"][1].clone();
    s["configOptions"].as_array_mut().unwrap().push(duplicate);
    assert!(parse_catalog("test", "r", &init(), &s).is_err());
    let mut s = session();
    s["configOptions"][1]["options"][1]["value"] = json!("future");
    assert!(parse_catalog("test", "r", &init(), &s).is_err());
}
#[test]
fn legacy_catalog_is_supported_without_inventing_effort() {
    let s = json!({"sessionId":"s","models":{"currentModelId":"native","availableModels":[{"modelId":"native","name":"Model"}]}});
    let c = parse_catalog("test", "r", &init(), &s).unwrap();
    assert_eq!(c.models[0].options.len(), 0);
    assert!(c.models[0].context_window.value().is_none());
}
#[test]
fn route_identity_and_configuration_changes_invalidate_selections() {
    let a = parse_catalog("test", "a", &init(), &session()).unwrap();
    let b = parse_catalog("test", "b", &init(), &session()).unwrap();
    assert_ne!(a.models[0].id, b.models[0].id);
    let mut s = session();
    s["configOptions"][1]["options"][0]["value"] = json!("new");
    s["configOptions"][1]["currentValue"] = json!("new");
    let c = parse_catalog("test", "a", &init(), &s).unwrap();
    assert_ne!(a.models[0].revision, c.models[0].revision);
}
#[test]
fn unknown_upstream_fields_do_not_leak_into_catalog() {
    let mut s = session();
    s["token"] = json!("private-sentinel");
    s["configOptions"][1]["secret"] = json!("private-sentinel");
    let c = parse_catalog("test", "r", &init(), &s).unwrap();
    assert!(
        !serde_json::to_string(&c.models)
            .unwrap()
            .contains("private-sentinel")
    );
}
