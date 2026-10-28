use originlm_contracts::*;
use serde_json::json;
fn source() -> serde_json::Value {
    json!({"kind":"official_protocol","adapterId":"codex","adapterVersion":"test","reference":"account/read"})
}
#[test]
fn known_requires_evidence_and_time() {
    assert!(validate_observation(json!({"state":"known","value":"Plus"})).is_err());
}
#[test]
fn known_roundtrips_without_losing_stale_state() {
    let value = json!({"state":"known","value":"native-plan-99","source":source(),"observedAt":"2026-09-24T00:00:00Z","freshness":"stale","verification":"reported"});
    assert_eq!(validate_observation(value.clone()).unwrap(), value);
}
#[test]
fn unknown_cannot_smuggle_a_value() {
    assert!(
        validate_observation(json!({"state":"unknown","reason":"NOT_REPORTED","value":"Plus"}))
            .is_err()
    );
}
#[test]
fn invalid_timestamp_is_rejected() {
    assert!(validate_observation(json!({"state":"known","value":"x","source":source(),"observedAt":"yesterday","freshness":"fresh","verification":"reported"})).is_err());
}
#[test]
fn unavailable_is_not_unknown() {
    let value = json!({"state":"unavailable","reason":"KEYRING_LOCKED"});
    assert_eq!(validate_observation(value.clone()).unwrap(), value);
}
#[test]
fn unknown_quota_denominator_stays_unknown() {
    let result = normalize_usage(json!({"unit":"requests","used":"17","limit":null})).unwrap();
    assert_eq!(result["remaining"], json!(null));
    assert_eq!(result["usedFraction"], json!(null));
    assert_eq!(result["used"], "17");
}
#[test]
fn counts_beyond_javascript_precision_stay_exact() {
    let result = normalize_usage(
        json!({"unit":"tokens","used":"9007199254740993","limit":"9007199254741000"}),
    )
    .unwrap();
    assert_eq!(result["used"], "9007199254740993");
    assert_eq!(result["remaining"], "7");
}
#[test]
fn money_stays_decimal_and_usage_over_limit_is_not_negative_remaining() {
    let result = normalize_usage(json!({"unit":"USD","used":"0.10","limit":"0.30"})).unwrap();
    assert_eq!(result["remaining"], "0.2");
    let over = normalize_usage(json!({"unit":"credits","used":"125","limit":"100"})).unwrap();
    assert_eq!(over["remaining"], "0");
    assert_eq!(over["usedFraction"], json!(1.25));
}
#[test]
fn negative_or_numeric_or_nondecimal_usage_is_rejected() {
    for used in [
        json!(17),
        json!("-1"),
        json!("NaN"),
        json!("1e99"),
        json!(""),
    ] {
        assert!(normalize_usage(json!({"unit":"requests","used":used,"limit":null})).is_err());
    }
}
#[test]
fn future_native_effort_is_accepted_only_when_advertised() {
    let descriptor = json!({"id":"thinking","label":"Thinking","scope":"turn","revision":"v1","control":{"type":"select","choices":[{"value":"cosmic","label":"Cosmic"}],"defaultValue":"cosmic"}});
    assert!(validate_option(descriptor.clone(), json!("cosmic")).is_ok());
    assert!(validate_option(descriptor, json!("high")).is_err());
}
#[test]
fn numeric_bounds_are_enforced() {
    let descriptor = json!({"id":"budget","label":"Budget","scope":"turn","revision":"v1","control":{"type":"number","min":1.0,"max":10.0,"step":1.0}});
    assert!(validate_option(descriptor.clone(), json!(7)).is_ok());
    assert!(validate_option(descriptor.clone(), json!(11)).is_err());
    assert!(validate_option(descriptor, json!(2.5)).is_err());
}
#[test]
fn identical_native_ids_remain_distinct_across_routes_and_delimiters() {
    assert_ne!(
        route_model_key("personal", "model"),
        route_model_key("work", "model")
    );
    assert_ne!(route_model_key("a:b", "c"), route_model_key("a", "b:c"));
    assert_eq!(
        route_model_key("personal", "model"),
        route_model_key("personal", "model")
    );
}
#[test]
fn schema_is_generated_and_contains_domain_records() {
    let schema = schema_document();
    assert_eq!(schema["title"], "PublicContracts");
    for record in [
        "Snapshot",
        "Connection",
        "AccessRoute",
        "Model",
        "OptionDescriptor",
        "QuotaPool",
        "Session",
        "DomainError",
    ] {
        assert!(schema["$defs"].get(record).is_some(), "missing {record}");
    }
}
