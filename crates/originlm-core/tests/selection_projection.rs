mod common;
use common::*;
use originlm_contracts::*;
use originlm_core::*;
use std::collections::BTreeSet;
#[test]
fn accountless_local_runtime_requires_matching_provider_and_fresh_local_evidence() {
    for provider in ["ollama", "lmstudio"] {
        let mut s = fixture();
        s.connections[0].provider_id = provider.into();
        s.routes[0].provider_id = provider.into();
        s.connections[0].auth = Observation::unknown("LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH");
        s.routes[0].billing = Observation::reported(
            BillingMode::Local,
            EvidenceSource::new(EvidenceKind::Synthetic, provider, "fixture"),
        );
        assert!(validate_selection(&s, &selection(), false).is_ok());
        s.routes[0].billing.mark_stale();
        assert!(validate_selection(&s, &selection(), false).is_err());
    }
    for provider in ["test", "ollama"] {
        let mut s = fixture();
        s.connections[0].provider_id = provider.into();
        s.connections[0].auth = Observation::unknown("LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH");
        s.routes[0].billing = Observation::reported(
            BillingMode::Local,
            EvidenceSource::new(EvidenceKind::Synthetic, provider, "fixture"),
        );
        assert!(validate_selection(&s, &selection(), false).is_err());
    }
}
#[test]
fn denied_client_sees_no_account_model_pool_or_profile() {
    let s = project_snapshot(&fixture(), &ClientPolicy::denied());
    assert!(s.connections.is_empty());
    assert!(s.models.is_empty());
    assert!(s.accounts.is_empty());
    assert!(s.quota_pools.is_empty());
    assert!(s.profiles.is_empty());
    assert!(s.installations.is_empty());
}
#[test]
fn projection_filters_linked_routes_not_just_top_level_connections() {
    let policy = ClientPolicy {
        connection_ids: Some(BTreeSet::from(["personal".into()])),
        ..ClientPolicy::denied()
    };
    let s = project_snapshot(&fixture(), &policy);
    assert_eq!(s.connections.len(), 1);
    assert_eq!(s.models.len(), 1);
    assert_eq!(s.quota_pools[0].route_ids, vec!["route-personal"]);
    assert_eq!(s.quota_pools[0].connection_ids, vec!["personal"]);
    assert!(!serde_json::to_string(&s).unwrap().contains("work"));
}
#[test]
fn identity_needs_separate_permission() {
    let s = project_snapshot(&fixture(), &ClientPolicy::passive());
    assert!(
        s.accounts
            .iter()
            .all(|a| a.display_label.value().is_none() && a.identity_source.value().is_none())
    );
    let p = ClientPolicy {
        reveal_identity: true,
        ..ClientPolicy::passive()
    };
    assert!(
        project_snapshot(&fixture(), &p).accounts[0]
            .display_label
            .value()
            .is_some()
    );
}
#[test]
fn native_model_names_do_not_erase_account_routes() {
    let c = resolve(&fixture(), &Requirements::default());
    assert_eq!(c.len(), 2);
    assert_ne!(c[0].selection.route_id, c[1].selection.route_id);
    assert!(c.iter().all(|c| c.eligible));
}
#[test]
fn stale_sign_in_stands_until_the_client_reports_otherwise() {
    let mut s = fixture();
    s.connections[0].auth.mark_stale();
    assert!(validate_selection(&s, &selection(), false).is_ok());
}
#[test]
fn signed_out_identity_blocks_runtime_selection() {
    let mut s = fixture();
    s.connections[0].auth = Observation::reported(
        AuthState::Unauthenticated,
        EvidenceSource::new(EvidenceKind::Synthetic, "synthetic", "core test"),
    );
    assert_eq!(
        validate_selection(&s, &selection(), false)
            .unwrap_err()
            .code,
        ErrorCode::AuthRequired
    );
}
#[test]
fn changed_catalog_revision_cannot_execute_saved_selection() {
    let mut s = fixture();
    s.models[0].revision = "2".into();
    assert_eq!(
        validate_selection(&s, &selection(), false)
            .unwrap_err()
            .code,
        ErrorCode::SelectionStale
    );
}
#[test]
fn matching_model_name_from_another_route_is_not_interchangeable() {
    let mut selected = selection();
    selected.route_id = "route-work".into();
    assert_eq!(
        validate_selection(&fixture(), &selected, false)
            .unwrap_err()
            .code,
        ErrorCode::SessionRouteMismatch
    );
}
#[test]
fn unknown_or_conditional_capability_is_not_confirmed() {
    for support in [Support::Unknown, Support::Conditional] {
        let mut s = fixture();
        s.models[0].capabilities[0].support = support;
        let c = resolve(
            &s,
            &Requirements {
                required_capabilities: vec!["generation.text".into()],
                ..Requirements::default()
            },
        );
        assert!(!c[0].eligible);
        assert!(c[1].eligible);
    }
}
#[test]
fn api_billing_does_not_inherit_subscription_consent() {
    let mut s = fixture();
    s.routes[0].billing = Observation::reported(
        BillingMode::ApiMetered,
        EvidenceSource::new(EvidenceKind::Synthetic, "test", "test"),
    );
    assert_eq!(
        validate_selection(&s, &selection(), false)
            .unwrap_err()
            .code,
        ErrorCode::BillingConsentRequired
    );
    assert!(validate_selection(&s, &selection(), true).is_ok());
}
#[test]
fn unadvertised_options_are_rejected_before_execution() {
    let mut selected = selection();
    selected
        .options
        .insert("effort".into(), serde_json::json!("high"));
    assert_eq!(
        validate_selection(&fixture(), &selected, false)
            .unwrap_err()
            .code,
        ErrorCode::OptionUnsupported
    );
}
#[test]
fn future_native_effort_is_preserved_and_validated() {
    let mut s = fixture();
    s.models[0].options.push(serde_json::from_value(serde_json::json!({"id":"effort","label":"Effort","revision":"1","scope":"turn","control":{"type":"select","choices":[{"value":"future-9","label":"Future 9"}]}})).unwrap());
    let mut selected = selection();
    selected
        .options
        .insert("effort".into(), serde_json::json!("future-9"));
    assert!(validate_selection(&s, &selected, false).is_ok());
}
#[test]
fn projections_do_not_mutate_source() {
    let s = fixture();
    let before = serde_json::to_value(&s).unwrap();
    project_snapshot(&s, &ClientPolicy::denied());
    assert_eq!(serde_json::to_value(&s).unwrap(), before);
}
