use originlm_adapters::native_clients::{
    client::profile_evidence,
    config::{NativeProfile, NativeProvider},
};
use originlm_contracts::*;
use std::path::PathBuf;
fn profile(billing: Option<BillingMode>) -> NativeProfile {
    NativeProfile {
        id: "p".into(),
        label: "P".into(),
        provider: NativeProvider::Grok,
        executable: PathBuf::from("C:/grok.exe"),
        profile_home: PathBuf::from("C:/.grok"),
        auth_method: None,
        billing,
    }
}
#[test]
fn successful_native_session_proves_auth_but_not_plan() {
    let evidence = profile_evidence(&profile(None));
    assert_eq!(evidence.auth.fresh_value(), Some(&AuthState::Authenticated));
    assert!(evidence.plan.value().is_none());
    assert!(evidence.billing.value().is_none());
    if let Observation::Known { source, .. } = evidence.auth {
        assert_eq!(source.kind, EvidenceKind::OfficialProtocol)
    } else {
        panic!("auth evidence missing")
    }
}
#[test]
fn host_billing_declaration_stays_user_supplied() {
    let evidence = profile_evidence(&profile(Some(BillingMode::Subscription)));
    assert_eq!(
        evidence.billing.fresh_value(),
        Some(&BillingMode::Subscription)
    );
    if let Observation::Known { source, .. } = evidence.billing {
        assert_eq!(source.kind, EvidenceKind::UserSupplied)
    } else {
        panic!("billing evidence missing")
    }
}
