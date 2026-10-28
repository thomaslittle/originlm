use originlm_adapters::codex::{apply_configured_service_tier, parse_metadata};
use originlm_contracts::*;
use serde_json::{Value, json};
fn init() -> Value {
    json!({"userAgent":"codex_cli_rs/0.156.1","codexHome":"PRIVATE-HOME","platformFamily":"windows","platformOs":"windows"})
}
fn account() -> Value {
    json!({"account":{"type":"chatgpt","email":"synthetic@example.test","planType":"future-plan","secretToken":"DO-NOT-EXPORT"},"requiresOpenaiAuth":true})
}
fn model() -> Value {
    json!({"id":"native-id","model":"native/model-with-suffix","displayName":"Native model","isDefault":true,"hidden":false,"defaultReasoningEffort":"future-effort","supportedReasoningEfforts":[{"reasoningEffort":"future-effort","description":"Provider option"}],"inputModalities":["text","image"],"serviceTiers":[{"id":"native-tier","name":"Native tier","description":"Potential billing"}],"defaultServiceTier":"native-tier"})
}
fn usage() -> Value {
    json!({"accountId":"native-account","ordinaryUsageAllowed":false,"rateLimits":{"limitId":"codex","primary":{"usedPercent":23,"windowDurationMins":300,"resetsAt":1790000000}},"rateLimitsByLimitId":{"codex":{"limitId":"codex","primary":{"usedPercent":23,"windowDurationMins":300,"resetsAt":1790000000},"secondary":{"usedPercent":41,"windowDurationMins":10080}}}})
}
#[test]
fn actual_plan_and_native_effort_survive_without_model_name_rules() {
    let report = parse_metadata(
        "connection",
        "route",
        &init(),
        &account(),
        &[model()],
        Some(&usage()),
    )
    .unwrap();
    assert_eq!(report.plan.fresh_value().unwrap().native_id, "future-plan");
    assert_eq!(report.auth.fresh_value(), Some(&AuthState::Authenticated));
    assert_eq!(
        report.billing.fresh_value(),
        Some(&BillingMode::Subscription)
    );
    let m = &report.models[0];
    assert_eq!(m.native_id, "native/model-with-suffix");
    let option = m
        .options
        .iter()
        .find(|o| o.id == "reasoningEffort")
        .unwrap();
    assert!(option.validate(&json!("future-effort")).is_ok());
    assert!(option.validate(&json!("high")).is_err());
    assert!(
        m.options
            .iter()
            .find(|o| o.id == "serviceTier")
            .unwrap()
            .billing_sensitive
    );
}
#[test]
fn known_plan_codes_keep_the_native_id_and_gain_the_current_label() {
    let mut saved = account();
    saved["account"]["planType"] = json!("self_serve_business_prolite");
    let report = parse_metadata("connection", "route", &init(), &saved, &[model()], None).unwrap();
    let plan = report.plan.fresh_value().unwrap();
    assert_eq!(plan.native_id, "self_serve_business_prolite");
    assert_eq!(plan.label, "ChatGPT Business Premium Subscription");
    saved["account"]["planType"] = json!("pro");
    let report = parse_metadata("connection", "route", &init(), &saved, &[model()], None).unwrap();
    assert_eq!(
        report.plan.fresh_value().unwrap().label,
        "ChatGPT Pro 200 Subscription"
    );
    saved["account"]["planType"] = json!("future-plan");
    let report = parse_metadata("connection", "route", &init(), &saved, &[model()], None).unwrap();
    assert_eq!(report.plan.fresh_value().unwrap().label, "future-plan");
}
#[test]
fn configured_service_tier_selects_only_an_advertised_choice() {
    let mut report =
        parse_metadata("connection", "route", &init(), &account(), &[model()], None).unwrap();
    apply_configured_service_tier(&mut report.models, "fast");
    let option = report.models[0]
        .options
        .iter()
        .find(|option| option.id == "serviceTier")
        .unwrap();
    assert!(option.current_value.is_none());
    apply_configured_service_tier(&mut report.models, "native-tier");
    let option = report.models[0]
        .options
        .iter()
        .find(|option| option.id == "serviceTier")
        .unwrap();
    assert_eq!(option.current_value.as_ref(), Some(&json!("native-tier")));
}
#[test]
fn enterprise_monthly_credits_are_a_separate_window() {
    let mut rates = usage();
    rates["rateLimitsByLimitId"]["codex"]["individualLimit"] =
        json!({"remainingPercent": 60, "resetsAt": 1790000000});
    let report = parse_metadata(
        "connection",
        "route",
        &init(),
        &account(),
        &[model()],
        Some(&rates),
    )
    .unwrap();
    let credit = report.quota_pools[0]
        .windows
        .iter()
        .find(|window| window.label == "Monthly credit limit")
        .unwrap();
    assert_eq!(credit.reading.fresh_value().unwrap().used.as_str(), "40");
    assert!(credit.duration_seconds.is_none());
    assert_eq!(report.quota_pools[0].windows.len(), 3);
}
#[test]
fn secrets_private_paths_and_unknown_fields_do_not_escape() {
    let report = parse_metadata(
        "connection",
        "route",
        &init(),
        &account(),
        &[model()],
        Some(&usage()),
    )
    .unwrap();
    let text = serde_json::to_string(&report).unwrap();
    assert!(!text.contains("DO-NOT-EXPORT"));
    assert!(!text.contains("PRIVATE-HOME"));
    assert!(!text.contains("native-account"));
}
#[test]
fn multiple_clients_do_not_merge_accounts_based_on_same_email() {
    let a = parse_metadata("a", "route-a", &init(), &account(), &[model()], None).unwrap();
    let b = parse_metadata("b", "route-b", &init(), &account(), &[model()], None).unwrap();
    assert_ne!(a.account.unwrap().id, b.account.unwrap().id);
    assert_ne!(a.models[0].id, b.models[0].id);
}
#[test]
fn unavailable_usage_is_not_an_empty_remaining_allowance() {
    let report =
        parse_metadata("connection", "route", &init(), &account(), &[model()], None).unwrap();
    assert!(report.quota_pools.is_empty());
    assert!(matches!(
        report.included_usage_allowed,
        Observation::Unknown { .. }
    ));
    assert!(report.diagnostics.iter().any(|d| d == "USAGE_UNAVAILABLE"));
}
#[test]
fn legacy_and_multi_bucket_views_are_not_double_counted() {
    let report = parse_metadata(
        "connection",
        "route",
        &init(),
        &account(),
        &[model()],
        Some(&usage()),
    )
    .unwrap();
    assert_eq!(report.quota_pools.len(), 1);
    assert_eq!(report.quota_pools[0].windows.len(), 2);
    assert_eq!(report.quota_pools[0].windows[0].label, "Codex · primary");
    let reading = report.quota_pools[0].windows[0]
        .reading
        .fresh_value()
        .unwrap();
    assert_eq!(reading.unit, "percent");
    assert_eq!(reading.used.as_str(), "23");
    assert_eq!(reading.remaining.as_ref().unwrap().as_str(), "77");
    assert_eq!(report.included_usage_allowed.fresh_value(), Some(&false));
}
#[test]
fn no_account_does_not_claim_active_plan_or_model_access() {
    let report = parse_metadata(
        "connection",
        "route",
        &init(),
        &json!({"account":null,"requiresOpenaiAuth":true}),
        &[model()],
        Some(&usage()),
    )
    .unwrap();
    assert_eq!(report.auth.fresh_value(), Some(&AuthState::Unauthenticated));
    assert!(report.models.is_empty());
    assert!(report.account.is_none());
    assert!(report.quota_pools.is_empty());
}
#[test]
fn api_key_auth_is_not_a_chatgpt_subscription() {
    let report = parse_metadata(
        "connection",
        "route",
        &init(),
        &json!({"account":{"type":"apiKey"},"requiresOpenaiAuth":false}),
        &[model()],
        None,
    )
    .unwrap();
    assert_eq!(report.billing.fresh_value(), Some(&BillingMode::ApiMetered));
    assert!(report.plan.fresh_value().is_none());
}
#[test]
fn absent_effort_and_context_are_not_invented() {
    let mut m = model();
    m["supportedReasoningEfforts"] = json!([]);
    m["serviceTiers"] = json!([]);
    let report = parse_metadata("connection", "route", &init(), &account(), &[m], None).unwrap();
    assert!(report.models[0].options.is_empty());
    assert!(matches!(
        report.models[0].context_window,
        Observation::Unknown { .. }
    ));
}
#[test]
fn malformed_native_records_fail_closed() {
    for m in [
        json!({"model":null}),
        json!({"model":"","displayName":"invalid"}),
    ] {
        assert!(parse_metadata("connection", "route", &init(), &account(), &[m], None).is_err());
    }
    assert!(
        parse_metadata(
            "connection",
            "route",
            &init(),
            &json!({"account":{"type":"unknown-auth"}}),
            &[model()],
            None
        )
        .is_err()
    );
}
#[test]
fn duplicate_model_ids_do_not_create_ambiguous_routes() {
    assert!(
        parse_metadata(
            "connection",
            "route",
            &init(),
            &account(),
            &[model(), model()],
            None
        )
        .is_err()
    );
}
#[test]
fn model_input_capability_is_not_image_generation() {
    let r = parse_metadata("connection", "route", &init(), &account(), &[model()], None).unwrap();
    assert!(
        r.models[0]
            .capabilities
            .iter()
            .any(|c| c.id == "input.image")
    );
    assert!(
        !r.models[0]
            .capabilities
            .iter()
            .any(|c| c.id == "generation.image")
    );
}
#[test]
fn changed_identity_changes_catalog_revision() {
    let first =
        parse_metadata("connection", "route", &init(), &account(), &[model()], None).unwrap();
    let mut other = account();
    other["account"]["email"] = json!("other@example.test");
    let second = parse_metadata("connection", "route", &init(), &other, &[model()], None).unwrap();
    assert_ne!(first.account.unwrap().id, second.account.unwrap().id);
    assert_ne!(first.models[0].revision, second.models[0].revision);
}
#[test]
fn unchanged_catalog_keeps_model_revisions_across_probes() {
    let first = parse_metadata("c", "r", &init(), &account(), &[model()], None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    // A later probe (and even a client upgrade that changes evidence sources)
    // must not re-pin the host's selection when the catalog itself is unchanged.
    let mut later = init();
    later["userAgent"] = json!("codex_cli_rs/0.157.1");
    let second = parse_metadata("c", "r", &later, &account(), &[model()], None).unwrap();
    let revisions = |report: &ConnectionMetadata| {
        report
            .models
            .iter()
            .map(|m| (m.id.clone(), m.revision.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(revisions(&first), revisions(&second));
    assert!(!first.models.is_empty());
    let mut changed = model();
    changed["supportedReasoningEfforts"] =
        json!([{"reasoningEffort":"other-effort","description":"Provider option"}]);
    changed["defaultReasoningEffort"] = json!("other-effort");
    let third = parse_metadata("c", "r", &init(), &account(), &[changed], None).unwrap();
    assert_ne!(first.models[0].revision, third.models[0].revision);
}
