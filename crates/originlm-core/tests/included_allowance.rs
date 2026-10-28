#[path = "common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::*;
fn snapshot() -> Snapshot {
    let mut s = common::fixture();
    s.routes[0].capabilities.push(Capability {
        id: "usage.included_allowed".into(),
        support: Support::Unsupported,
        origin: CapabilityOrigin::Runtime,
        evidence: Observation::reported(
            "Provider denied ordinary included usage".into(),
            EvidenceSource::new(EvidenceKind::Synthetic, "test", "explicit backend denial"),
        ),
        conditions: vec![],
    });
    s
}
#[test]
fn explicit_included_usage_denial_blocks_selection_even_with_paid_option_consent() {
    let s = snapshot();
    for paid in [false, true] {
        assert_eq!(
            validate_selection(&s, &common::selection(), paid)
                .unwrap_err()
                .code,
            ErrorCode::RateLimited
        );
        let choices = resolve(
            &s,
            &Requirements {
                allow_paid_fallback: paid,
                ..Default::default()
            },
        );
        assert!(
            !choices
                .iter()
                .find(|c| c.connection_id == "personal")
                .unwrap()
                .eligible
        );
    }
}
#[test]
fn passed_time_or_stale_denial_is_not_automatic_quota_recovery() {
    let mut s = snapshot();
    s.routes[0].capabilities[0].evidence.mark_stale();
    assert_eq!(
        validate_selection(&s, &common::selection(), false)
            .unwrap_err()
            .code,
        ErrorCode::RateLimited
    );
}
#[test]
fn fresh_backend_allowance_restoration_is_honored() {
    let mut s = snapshot();
    s.routes[0].capabilities[0].support = Support::Supported;
    assert!(validate_selection(&s, &common::selection(), false).is_ok());
}
#[test]
fn separate_metered_route_still_requires_its_own_billing_consent() {
    let mut s = snapshot();
    s.routes[0].billing = Observation::reported(
        BillingMode::ApiMetered,
        EvidenceSource::new(EvidenceKind::Synthetic, "test", "explicit API route"),
    );
    assert_eq!(
        validate_selection(&s, &common::selection(), false)
            .unwrap_err()
            .code,
        ErrorCode::BillingConsentRequired
    );
    assert!(validate_selection(&s, &common::selection(), true).is_ok());
}
