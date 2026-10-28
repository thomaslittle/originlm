use originlm_adapters::opencode::catalog_v2;
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn inputs() -> (Value, Value) {
    (
        json!({"data":[{"id":"a","name":"A","activation":"enabled"},{"id":"b","name":"B","activation":"auto"}]}),
        json!({"data":[
        {"id":"native","modelID":"underlying","providerID":"a","name":"Same","enabled":true,"capabilities":{"input":["text","image"],"output":["text"],"tools":true},"variants":[{"id":"future-effort","headers":{"authorization":"secret-sentinel"}}],"limit":{"context":8192},"headers":{"authorization":"secret-sentinel"}},
        {"id":"native","providerID":"b","name":"Same","enabled":true,"capabilities":{"input":["text"],"output":["text"],"tools":false},"variants":[],"limit":{}}
        ]}),
    )
}
#[test]
fn retains_route_native_id_and_options_but_never_infers_auth_or_billing() {
    let (p, m) = inputs();
    let result = catalog_v2::catalog("c", "root", &p, &m, &BTreeMap::new()).unwrap();
    assert_eq!(result.models.len(), 2);
    assert_ne!(result.models[0].id, result.models[1].id);
    assert_eq!(result.models[0].native_id, "native");
    assert!(
        result.models[0].options[0]
            .validate(&json!("future-effort"))
            .is_ok()
    );
    assert!(result.auth.value().is_none());
    assert!(
        result
            .additional_routes
            .iter()
            .all(|r| r.billing.value().is_none())
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("secret-sentinel")
    );
}
#[test]
fn opencode_go_v2_is_subscription_and_payg_stays_unknown() {
    let providers = json!({"data":[
        {"id":"opencode-go","name":"OpenCode Go","activation":"enabled"},
        {"id":"opencode","name":"OpenCode","activation":"enabled"}
    ]});
    let models = json!({"data":[
        {"id":"go-model","providerID":"opencode-go","name":"Go","enabled":true,"capabilities":{"input":["text"],"output":["text"],"tools":false},"variants":[],"limit":{}},
        {"id":"payg-model","providerID":"opencode","name":"PAYG","enabled":true,"capabilities":{"input":["text"],"output":["text"],"tools":false},"variants":[],"limit":{}}
    ]});
    let result = catalog_v2::catalog("c", "root", &providers, &models, &BTreeMap::new()).unwrap();
    let go = result
        .additional_routes
        .iter()
        .find(|route| route.provider_id == "opencode-go")
        .unwrap();
    assert_eq!(go.billing.value(), Some(&BillingMode::Subscription));
    assert!(matches!(
        &go.billing,
        Observation::Known {
            verification: Verification::Inferred,
            ..
        }
    ));
    assert!(
        result
            .additional_routes
            .iter()
            .find(|route| route.provider_id == "opencode")
            .unwrap()
            .billing
            .value()
            .is_none()
    );
    assert!(result.auth.value().is_none(), "route class is not auth");
    assert!(result.quota_pools.is_empty());
}

#[test]
fn ignores_disabled_and_non_text_models() {
    let (p, mut m) = inputs();
    m["data"][0]["enabled"] = json!(false);
    m["data"][1]["capabilities"]["output"] = json!(["embedding"]);
    assert!(
        catalog_v2::catalog("c", "root", &p, &m, &BTreeMap::new())
            .unwrap()
            .models
            .is_empty()
    );
}
#[test]
fn rejects_duplicate_model_and_unknown_provider() {
    let (p, mut m) = inputs();
    m["data"][1]["providerID"] = json!("a");
    assert!(catalog_v2::catalog("c", "root", &p, &m, &BTreeMap::new()).is_err());
    m["data"][1]["providerID"] = json!("not-declared");
    assert!(catalog_v2::catalog("c", "root", &p, &m, &BTreeMap::new()).is_err());
}
#[test]
fn explicit_billing_does_not_fabricate_authentication() {
    let (p, m) = inputs();
    let result = catalog_v2::catalog(
        "c",
        "root",
        &p,
        &m,
        &BTreeMap::from([("a".into(), BillingMode::Subscription)]),
    )
    .unwrap();
    assert!(result.auth.value().is_none());
    assert_eq!(
        result.additional_routes[0].billing.value(),
        Some(&BillingMode::Subscription)
    );
    assert!(result.additional_routes[1].billing.value().is_none());
}
