//! Muse metadata normalization (`initialize`, `account/read`, `model/list`,
//! `usage/read`). Shapes follow `muse schema generate-json-schema` (1.4.7).
use originlm_adapters::muse::parse_metadata;
use originlm_contracts::*;
use serde_json::{Value, json};

fn init() -> Value {
    json!({"serverInfo":{"name":"muse","version":"1.4.7"},"schema":{"version":1,"fingerprint":"sha256:00"},"grantedCapabilities":[]})
}
fn row(id: &str) -> Value {
    json!({"modelId":id,"displayLabel":"Muse Spark","providerId":"meta","profileId":null,"releaseDate":null,
        "description":null,"contextLimit":131072,"outputLimit":null,"cost":null,"isActive":false,"isDefault":true,
        "variants":["minimal","low","medium","high","xhigh"],"defaultReasoningEffort":"medium","reasoningEffortVariants":[]})
}
fn catalog(rows: Vec<Value>) -> Value {
    json!({"providerId":"meta","profileId":"tbh","source":"providerCatalog","models":rows})
}
fn login(label: &str) -> Value {
    json!({"state":"accountLogin","label":label,"credentialRequired":true})
}

#[test]
fn an_account_login_is_an_authenticated_subscription_with_native_efforts() {
    let report = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("person@example.test")),
        &catalog(vec![row("muse-spark-1.3")]),
        None,
    )
    .unwrap();
    assert_eq!(report.auth.value(), Some(&AuthState::Authenticated));
    assert_eq!(report.billing.value(), Some(&BillingMode::Subscription));
    assert_eq!(report.version.value().map(String::as_str), Some("1.4.7"));
    let source = match &report.auth {
        Observation::Known { source, .. } => source.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(source.kind, EvidenceKind::OfficialProtocol);
    assert_eq!(source.client_version.as_deref(), Some("1.4.7"));
    let account = report.account.as_ref().unwrap();
    assert_eq!(account.provider_id, "meta");
    assert_eq!(
        account.display_label.value().map(String::as_str),
        Some("person@example.test")
    );
    let model = &report.models[0];
    assert_eq!(model.native_id, "muse-spark-1.3");
    assert_eq!(model.id, route_model_key("r", "muse-spark-1.3"));
    assert_eq!(
        model.context_window.value().map(String::as_str),
        Some("131072")
    );
    match &model.options[0].control {
        OptionControl::Select {
            choices,
            default_value,
        } => {
            assert_eq!(choices.len(), 5);
            assert_eq!(choices[4].label, "Extra high");
            assert_eq!(default_value.as_deref(), Some("medium"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(model.options[0].revision, model.revision);
    assert!(
        report
            .diagnostics
            .contains(&"USAGE_NOT_YET_OBSERVED".to_owned())
    );
}

#[test]
fn api_keys_are_metered_and_signed_out_muse_is_unauthenticated() {
    for lane in ["apiKey", "envKey"] {
        let report = parse_metadata(
            "c",
            "r",
            &init(),
            Some(&json!({"state":lane,"credentialRequired":true})),
            &catalog(vec![row("m")]),
            None,
        )
        .unwrap();
        assert_eq!(report.auth.value(), Some(&AuthState::Authenticated));
        assert_eq!(report.billing.value(), Some(&BillingMode::ApiMetered));
        assert!(report.account.is_none());
    }
    let out = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&json!({"state":"loggedOut","credentialRequired":true})),
        &catalog(vec![row("m")]),
        None,
    )
    .unwrap();
    assert_eq!(out.auth.value(), Some(&AuthState::Unauthenticated));
    assert!(out.billing.value().is_none());
    let keyless = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&json!({"state":"loggedOut","credentialRequired":false})),
        &catalog(vec![row("m")]),
        None,
    )
    .unwrap();
    assert_eq!(keyless.auth.value(), Some(&AuthState::Authenticated));
    assert!(keyless.billing.value().is_none());
}

#[test]
fn without_account_read_auth_stays_unknown_not_assumed() {
    for account in [
        None,
        Some(json!({"state":"futureLane","credentialRequired":true})),
    ] {
        let report = parse_metadata(
            "c",
            "r",
            &init(),
            account.as_ref(),
            &catalog(vec![row("m")]),
            None,
        )
        .unwrap();
        assert!(report.auth.value().is_none());
        assert!(report.billing.value().is_none());
    }
}

#[test]
fn two_accounts_with_the_same_model_have_distinct_revisions() {
    let a = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &catalog(vec![row("m")]),
        None,
    )
    .unwrap();
    let b = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("b@example.test")),
        &catalog(vec![row("m")]),
        None,
    )
    .unwrap();
    assert_eq!(a.models[0].native_id, b.models[0].native_id);
    assert_ne!(a.models[0].revision, b.models[0].revision);
    assert_ne!(a.account.unwrap().id, b.account.unwrap().id);
    // Unchanged content keeps its revision across probes.
    let again = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &catalog(vec![row("m")]),
        None,
    )
    .unwrap();
    let first = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &catalog(vec![row("m")]),
        None,
    )
    .unwrap();
    assert_eq!(again.models[0].revision, first.models[0].revision);
}

#[test]
fn unknown_and_future_efforts_are_not_invented() {
    let mut unknown = row("unknown-efforts");
    unknown["variants"] = json!("unknown");
    let mut future = row("future-efforts");
    future["variants"] = json!(["high", "hyper"]);
    future["defaultReasoningEffort"] = json!("hyper");
    let report = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &catalog(vec![unknown, future]),
        None,
    )
    .unwrap();
    assert!(report.models[0].options.is_empty());
    match &report.models[1].options[0].control {
        OptionControl::Select {
            choices,
            default_value,
        } => {
            assert_eq!(
                choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
                ["high"]
            );
            assert_eq!(default_value, &None);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn fake_and_foreign_catalogs_are_not_live_models() {
    let mut fake = catalog(vec![row("m")]);
    fake["source"] = json!("fakeCatalog");
    let report = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &fake,
        None,
    )
    .unwrap();
    assert!(report.models.is_empty());
    assert!(
        report
            .diagnostics
            .contains(&"MUSE_FAKE_CATALOG_IGNORED".to_owned())
    );
    let mut local = catalog(vec![row("m")]);
    local["providerId"] = json!("local");
    let report = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &local,
        None,
    )
    .unwrap();
    assert!(report.models.is_empty());
    let mut mixed = catalog(vec![row("m"), row("m")]);
    mixed["models"][1]["providerId"] = json!("openai");
    let report = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &mixed,
        None,
    )
    .unwrap();
    assert_eq!(report.models.len(), 1);
}

#[test]
fn malformed_catalogs_are_protocol_errors() {
    for bad in [
        json!({"providerId":"meta","models":"nope"}),
        json!({"providerId":"meta","models":[{"modelId":"","providerId":"meta"}]}),
        json!({"providerId":"meta","models":[{"modelId":"a\u{7}b","providerId":"meta"}]}),
    ] {
        assert!(matches!(
            parse_metadata("c", "r", &init(), None, &bad, None),
            Err(e) if e.code == ErrorCode::ProtocolIncompatible
        ));
    }
}

#[test]
fn subscription_usage_becomes_plan_and_percent_windows() {
    let usage = json!({"usage":{"observedAtMs":1791444565334u64,"tier":"contributor",
        "window":{"usedPercent":42,"resetsAtMs":1791448165334u64,"windowDurationMins":300},
        "weekly":{"usedPercent":7,"resetsAtMs":1792049365334u64}}});
    let report = parse_metadata(
        "c",
        "r",
        &init(),
        Some(&login("a@example.test")),
        &catalog(vec![row("m")]),
        Some(&usage),
    )
    .unwrap();
    assert_eq!(
        report.plan.value().map(|p| p.native_id.as_str()),
        Some("contributor")
    );
    let pool = &report.quota_pools[0];
    assert_eq!(pool.windows.len(), 2);
    assert_eq!(pool.windows[0].duration_seconds, Some(18_000));
    let reading = pool.windows[0].reading.value().unwrap();
    assert_eq!(serde_json::to_value(reading).unwrap()["used"], json!("42"));
    assert!(pool.windows[0].resets_at.is_some());
    assert!(
        !report
            .diagnostics
            .contains(&"USAGE_NOT_YET_OBSERVED".to_owned())
    );
}
