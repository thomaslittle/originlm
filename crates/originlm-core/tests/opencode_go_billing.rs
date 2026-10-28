use originlm_contracts::*;
use originlm_core::{Requirements, resolve, validate_selection};

fn evidence() -> EvidenceSource {
    EvidenceSource::new(
        EvidenceKind::Synthetic,
        "test",
        "OpenCode Go billing regression",
    )
}

fn capability() -> Capability {
    Capability {
        id: "generation.text".into(),
        support: Support::Supported,
        origin: CapabilityOrigin::Model,
        evidence: Observation::reported("native text".into(), evidence()),
        conditions: vec![],
    }
}

/// Route class as produced by the OpenCode adapter: inferred from the exact
/// `opencode-go` provider id, not from a billing declaration. Entitlement,
/// quota and extra usage remain unknown.
fn go_billing() -> Observation<BillingMode> {
    Observation::Known {
        value: BillingMode::Subscription,
        source: EvidenceSource::new(
            EvidenceKind::Derived,
            "opencode",
            "OpenCode Go provider id; subscription class; entitlement and quota not verified",
        ),
        observed_at: Timestamp::now(),
        freshness: Freshness::Fresh,
        verification: Verification::Inferred,
        expires_at: None,
    }
}

fn snapshot(auth: AuthState) -> Snapshot {
    let mut snapshot = Snapshot::empty(DataMode::Live);
    snapshot.connections.push(Connection {
        id: "connection".into(),
        provider_id: "opencode".into(),
        installation_id: "install".into(),
        profile_id: "profile".into(),
        display_name: "OpenCode".into(),
        account_id: None,
        auth: Observation::reported(auth, evidence()),
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        status: "ok".into(),
        diagnostics: vec![],
    });
    for (route_id, provider, billing) in [
        ("route-go", "opencode-go", go_billing()),
        (
            "route-payg",
            "opencode",
            Observation::reported(BillingMode::ApiMetered, evidence()),
        ),
        (
            "route-lookalike",
            "opencode-go-extra",
            Observation::unknown("UPSTREAM_BILLING_NOT_REPORTED"),
        ),
    ] {
        snapshot.routes.push(AccessRoute {
            id: route_id.into(),
            connection_id: "connection".into(),
            provider_id: provider.into(),
            display_name: provider.into(),
            billing,
            capabilities: vec![],
            catalog_revision: "r1".into(),
        });
        snapshot.models.push(Model {
            id: format!("model-{route_id}"),
            route_id: route_id.into(),
            native_id: "native".into(),
            name: format!("Model {route_id}"),
            revision: "m1".into(),
            is_default: false,
            capabilities: vec![capability()],
            options: vec![],
            context_window: Observation::unknown("NOT_REPORTED"),
        });
    }
    // Quota is unknown and then stale; neither is an access decision by
    // itself. A subscription route remains selectable without a paid opt-in.
    let mut reading = Observation::reported(
        UsageReading {
            unit: "usd".into(),
            used: DecimalString::try_from("0".to_string()).unwrap(),
            limit: Some(DecimalString::try_from("10".to_string()).unwrap()),
            remaining: Some(DecimalString::try_from("10".to_string()).unwrap()),
            used_fraction: Some(0.0),
        },
        evidence(),
    );
    reading.mark_stale();
    snapshot.quota_pools.push(QuotaPool {
        id: "go-pool".into(),
        account_id: None,
        connection_ids: vec!["connection".into()],
        route_ids: vec!["route-go".into(), "route-payg".into()],
        sharing: "unknown".into(),
        windows: vec![UsageWindow {
            id: "monthly".into(),
            label: "Monthly".into(),
            reading,
            resets_at: None,
            duration_seconds: None,
        }],
    });
    snapshot
}

fn requirements(allow_paid: bool) -> Requirements {
    Requirements {
        required_capabilities: vec!["generation.text".into()],
        allow_paid_fallback: allow_paid,
        eligible_only: false,
        billing_preference: Some("prefer-included".into()),
        query: None,
    }
}

#[test]
fn subscribed_go_route_is_eligible_without_declaration_or_paid_opt_in() {
    let snapshot = snapshot(AuthState::Authenticated);
    let candidates = resolve(&snapshot, &requirements(false));
    let go = candidates
        .iter()
        .find(|candidate| candidate.selection.route_id == "route-go")
        .unwrap();
    assert!(go.eligible, "Go subscription route was blocked: {go:?}");
    assert!(go.reasons.is_empty());
    assert!(
        candidates
            .iter()
            .find(|candidate| candidate.selection.route_id == "route-go")
            .is_some_and(|candidate| candidate.selection == candidates[0].selection)
    );
    assert_eq!(
        validate_selection(&snapshot, &go.selection, false)
            .expect("Go subscription selection")
            .id,
        "model-route-go"
    );
    // Unknown/stale quota is preserved, not converted into an access denial.
    assert!(matches!(
        snapshot.quota_pools[0].windows[0].reading,
        Observation::Known {
            freshness: Freshness::Stale,
            ..
        }
    ));
}

#[test]
fn metered_api_and_lookalike_routes_keep_their_safeguards() {
    let snapshot = snapshot(AuthState::Authenticated);
    let candidates = resolve(&snapshot, &requirements(false));
    let metered = candidates
        .iter()
        .find(|candidate| candidate.selection.route_id == "route-payg")
        .unwrap();
    assert!(!metered.eligible);
    assert!(
        metered
            .reasons
            .iter()
            .any(|reason| reason == "BILLING_CONSENT_REQUIRED")
    );
    assert_eq!(
        validate_selection(&snapshot, &metered.selection, false)
            .unwrap_err()
            .code,
        ErrorCode::BillingConsentRequired
    );

    let lookalike = candidates
        .iter()
        .find(|candidate| candidate.selection.route_id == "route-lookalike")
        .unwrap();
    assert!(!lookalike.eligible);
    assert!(
        lookalike
            .reasons
            .iter()
            .any(|reason| reason == "BILLING_CONSENT_REQUIRED"),
        "a lookalike provider id must not inherit Go subscription access"
    );

    // An explicit paid opt-in may unlock a genuinely metered route, but the
    // resolver never enables it silently.
    let paid = resolve(&snapshot, &requirements(true));
    assert!(
        paid.iter()
            .find(|candidate| candidate.selection.route_id == "route-payg")
            .is_some_and(|candidate| candidate.eligible)
    );
    assert!(
        !paid
            .iter()
            .find(|candidate| candidate.selection.route_id == "route-lookalike")
            .is_some_and(|candidate| candidate.eligible),
        "unknown billing still needs explicit evidence even with allowPaid"
    );
}

#[test]
fn expired_or_signed_out_auth_blocks_even_a_subscription_route() {
    for auth in [AuthState::Expired, AuthState::Unauthenticated] {
        let snapshot = snapshot(auth);
        let go = resolve(&snapshot, &requirements(false))
            .into_iter()
            .find(|candidate| candidate.selection.route_id == "route-go")
            .unwrap();
        assert!(!go.eligible);
        assert!(go.reasons.iter().any(|reason| reason == "AUTH_REQUIRED"));
        assert_eq!(
            validate_selection(&snapshot, &go.selection, false)
                .unwrap_err()
                .code,
            ErrorCode::AuthRequired
        );
    }
}
