use originlm_adapters::billing_declarations::BillingDeclarations;
use originlm_contracts::*;

fn report(connection: &str, billing: Observation<BillingMode>) -> ConnectionMetadata {
    ConnectionMetadata {
        connection_id: connection.into(),
        route_id: "route_a".into(),
        version: Observation::unknown("NOT_REPORTED"),
        auth: Observation::unknown("NOT_REPORTED"),
        plan: Observation::unknown("NOT_REPORTED"),
        billing,
        account: None,
        models: vec![],
        additional_routes: vec![AccessRoute {
            id: "route_b".into(),
            connection_id: connection.into(),
            provider_id: "anthropic".into(),
            display_name: "OpenCode / Anthropic".into(),
            billing: Observation::unknown("UPSTREAM_BILLING_NOT_REPORTED"),
            capabilities: vec![],
            catalog_revision: "r".into(),
        }],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("NOT_REPORTED"),
        diagnostics: vec![],
    }
}
const DOC: &str = r#"{"declarations":[
 {"connectionId":"connection_g","providerId":"grok","billing":"subscription"},
 {"connectionId":"connection_o","providerId":"opencode","billing":"api_metered"}]}"#;

#[test]
fn rejects_unknown_mixed_duplicate_local_native_and_extra_fields() {
    for bad in [
        r#"{"declarations":[{"connectionId":"c","providerId":"grok","billing":"unknown"}]}"#,
        r#"{"declarations":[{"connectionId":"c","providerId":"grok","billing":"mixed"}]}"#,
        r#"{"declarations":[{"connectionId":"c","providerId":"grok","billing":"local"}]}"#,
        r#"{"declarations":[{"connectionId":"c","providerId":"grok","billing":"subscription","token":"x"}]}"#,
        r#"{"declarations":[{"connectionId":"c/../x","providerId":"grok","billing":"subscription"}]}"#,
        r#"{"declarations":[{"connectionId":"c","providerId":"grok","billing":"subscription"},{"connectionId":"c","providerId":"grok","billing":"subscription"}]}"#,
    ] {
        assert!(BillingDeclarations::parse(bad).is_err(), "{bad}");
    }
    assert!(
        BillingDeclarations::parse(
            r#"{"declarations":[{"connectionId":"c","providerId":"opencode","billing":"local"}]}"#
        )
        .is_ok()
    );
}

#[test]
fn fills_only_unknown_billing_as_user_supplied_and_needs_matching_provider() {
    let d = BillingDeclarations::parse(DOC).unwrap();
    let mut r = report(
        "connection_g",
        Observation::unknown("NATIVE_MODEL_ROUTE_BILLING_NOT_REPORTED"),
    );
    d.apply("grok", &mut r);
    assert_eq!(r.billing.value(), Some(&BillingMode::Subscription));
    assert!(
        matches!(&r.billing, Observation::Known { source, verification: Verification::Reported, .. } if source.kind == EvidenceKind::UserSupplied)
    );
    assert_eq!(
        r.additional_routes[0].billing.value(),
        Some(&BillingMode::Subscription)
    );

    let mut other = report("connection_g", Observation::unknown("X"));
    d.apply("gemini_cli", &mut other);
    assert!(
        other.billing.value().is_none(),
        "provider mismatch must not inherit"
    );

    let reported = Observation::reported(
        BillingMode::ApiMetered,
        EvidenceSource::new(EvidenceKind::OfficialProtocol, "grok", "status"),
    );
    let mut known = report("connection_g", reported.clone());
    known.additional_routes[0].billing = reported.clone();
    d.apply("grok", &mut known);
    assert_eq!(
        known.billing, reported,
        "reported billing is never replaced"
    );
    assert!(
        !known
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "BILLING_DECLARED_BY_HOST"),
        "a declaration that changed nothing must not add its marker"
    );
    let mut classified = report(
        "connection_go",
        Observation::reported(
            BillingMode::Subscription,
            EvidenceSource::new(EvidenceKind::OfficialProtocol, "opencode", "reported"),
        ),
    );
    classified.additional_routes[0].billing = Observation::reported(
        BillingMode::Subscription,
        EvidenceSource::new(EvidenceKind::Derived, "opencode", "Go provider id"),
    );
    let go = BillingDeclarations::parse(
        r#"{"declarations":[{"connectionId":"connection_go","providerId":"opencode","billing":"subscription"}]}"#,
    )
    .unwrap();
    go.apply("opencode", &mut classified);
    assert!(
        !classified
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "BILLING_DECLARED_BY_HOST"),
        "an already-classified Go route must not look host-declared"
    );

    assert_eq!(
        d.native_execution_connections()
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["connection_g".to_string()]
    );
}
