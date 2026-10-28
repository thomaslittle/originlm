use originlm_adapters::claude::parse_metadata;
use originlm_contracts::*;
use serde_json::{Value, json};
fn init() -> Value {
    json!({"account":{"email":"person@example.test","subscriptionType":"Claude Test Plan","apiProvider":"firstParty"},"models":[{"value":"native-alias[long]","resolvedModel":"concrete-native","displayName":"Native model","supportsEffort":true,"supportedEffortLevels":["future-effort","standard"]},{"value":"small","displayName":"Small"}],"current_permission_mode":"dontAsk","pid":123,"user_output_styles_dir":"C:/PRIVATE"})
}
#[test]
fn reported_plan_native_ids_and_effort_are_preserved() {
    let r = parse_metadata("connection", "route", &init(), None).unwrap();
    assert_eq!(r.auth.value(), Some(&AuthState::Authenticated));
    assert_eq!(r.plan.value().unwrap().native_id, "Claude Test Plan");
    assert_eq!(r.models[0].native_id, "native-alias[long]");
    assert!(
        r.models[0].options[0]
            .validate(&json!("future-effort"))
            .is_ok()
    );
    assert!(r.models[1].options.is_empty());
    assert!(r.models[0].context_window.value().is_none());
}
#[test]
fn unknown_or_metered_auth_is_not_a_subscription() {
    let mut i = init();
    i["account"] = json!({"tokenSource":"apiKey","apiProvider":"firstParty"});
    assert_eq!(
        parse_metadata("c", "r", &i, None).unwrap().billing.value(),
        Some(&BillingMode::ApiMetered)
    );
    i["account"] = json!({"apiProvider":"bedrock"});
    assert_eq!(
        parse_metadata("c", "r", &i, None).unwrap().billing.value(),
        Some(&BillingMode::ApiMetered)
    );
    i["account"] = json!({});
    let r = parse_metadata("c", "r", &i, None).unwrap();
    assert!(r.auth.value().is_none());
    assert!(r.plan.value().is_none());
}
#[test]
fn normalized_usage_does_not_duplicate_legacy_buckets() {
    let usage = json!({"rate_limits_available":true,"rate_limits":{"limits":[{"kind":"session","percent":20,"resets_at":"2026-09-25T00:00:00Z"},{"kind":"weekly_scoped","percent":5,"scope":{"model":{"display_name":"Example"}}}],"five_hour":{"utilization":20}}});
    let r = parse_metadata("c", "r", &init(), Some(&usage)).unwrap();
    assert_eq!(r.quota_pools.len(), 1);
    assert_eq!(r.quota_pools[0].windows.len(), 2);
    assert_eq!(
        r.quota_pools[0].windows[0]
            .reading
            .value()
            .unwrap()
            .remaining
            .as_ref()
            .unwrap()
            .as_str(),
        "80"
    );
}
#[test]
fn private_initialization_fields_and_secrets_do_not_escape() {
    let mut i = init();
    i["authToken"] = json!("TOKEN_SECRET");
    i["models"][0]["credentials"] = json!("TOKEN_SECRET");
    let text = serde_json::to_string(&parse_metadata("c", "r", &i, None).unwrap()).unwrap();
    assert!(!text.contains("TOKEN_SECRET"));
    assert!(!text.contains("PRIVATE"));
    assert!(!text.contains("user_output_styles"));
}
#[test]
fn enterprise_monthly_spend_prefers_the_enabled_budget() {
    let usage = json!({"rate_limits_available":true,"rate_limits":{"five_hour":{"utilization":12,"resets_at":null},"spend":{"enabled":true,"used":{"amount_minor":4631,"currency":"USD","exponent":2},"limit":{"amount_minor":50000,"currency":"USD","exponent":2}},"extra_usage":{"is_enabled":true,"used_credits":30000,"monthly_limit":50000,"utilization":60}}});
    let report = parse_metadata("c", "r", &init(), Some(&usage)).unwrap();
    assert_eq!(report.quota_pools[0].windows.len(), 2);
    let spend = report.quota_pools[0]
        .windows
        .iter()
        .find(|window| window.id == "monthly_spend")
        .unwrap();
    assert_eq!(spend.label, "Monthly spend");
    assert_eq!(spend.reading.fresh_value().unwrap().used.as_str(), "9.262");
    assert!(
        !report.quota_pools[0]
            .windows
            .iter()
            .any(|window| window.id == "extra_usage")
    );
}
#[test]
fn disabled_spend_and_extra_usage_do_not_invent_a_budget() {
    let usage = json!({"rate_limits_available":true,"rate_limits":{"five_hour":{"utilization":12,"resets_at":null},"spend":{"enabled":false},"extra_usage":{"is_enabled":false,"utilization":40}}});
    let report = parse_metadata("c", "r", &init(), Some(&usage)).unwrap();
    assert_eq!(report.quota_pools[0].windows.len(), 1);
    assert_eq!(report.quota_pools[0].windows[0].id, "five_hour");
}
#[test]
fn unavailable_usage_is_not_zero() {
    let r = parse_metadata("c", "r", &init(), None).unwrap();
    assert!(r.quota_pools.is_empty());
    assert!(r.diagnostics.contains(&"USAGE_UNAVAILABLE".into()));
}
#[test]
fn duplicate_model_values_and_invalid_usage_fail() {
    let mut i = init();
    i["models"][1] = i["models"][0].clone();
    assert!(parse_metadata("c", "r", &i, None).is_err());
    assert!(parse_metadata("c","r",&init(),Some(&json!({"rate_limits_available":true,"rate_limits":{"limits":[{"kind":"session","percent":-1}]}}))).is_err());
}
#[test]
fn distinct_profile_same_email_is_not_linked() {
    let a = parse_metadata("one", "r1", &init(), None).unwrap();
    let b = parse_metadata("two", "r2", &init(), None).unwrap();
    assert_ne!(a.account.unwrap().id, b.account.unwrap().id);
    assert_ne!(a.models[0].id, b.models[0].id);
}
