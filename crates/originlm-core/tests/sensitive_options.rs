#[allow(dead_code)]
#[path = "common/mod.rs"]
mod common;
use originlm_contracts::*;
use originlm_core::validate_selection;
use serde_json::json;
fn configure(billing: bool, permission: bool) -> (Snapshot, Selection) {
    let mut snapshot = common::fixture();
    let mut selection = common::selection();
    let model = snapshot
        .models
        .iter_mut()
        .find(|m| m.id == selection.model_id)
        .unwrap();
    model.options.push(OptionDescriptor {
        id: "nativeMode".into(),
        label: "Native mode".into(),
        scope: OptionScope::Turn,
        revision: model.revision.clone(),
        control: OptionControl::Select {
            choices: vec![Choice {
                value: "special".into(),
                label: "Special".into(),
                disabled_reason: None,
            }],
            default_value: None,
        },
        current_value: None,
        category: None,
        source: None,
        billing_sensitive: billing,
        permission_sensitive: permission,
    });
    selection
        .options
        .insert("nativeMode".into(), json!("special"));
    (snapshot, selection)
}
#[test]
fn subscription_does_not_authorize_billing_sensitive_option() {
    let (snapshot, selection) = configure(true, false);
    assert_eq!(
        validate_selection(&snapshot, &selection, false)
            .unwrap_err()
            .code,
        ErrorCode::BillingConsentRequired
    );
    assert!(validate_selection(&snapshot, &selection, true).is_ok());
}
#[test]
fn permission_sensitive_option_needs_a_separate_approval_path() {
    let (snapshot, selection) = configure(false, true);
    assert_eq!(
        validate_selection(&snapshot, &selection, true)
            .unwrap_err()
            .code,
        ErrorCode::ApprovalRequired
    );
}
