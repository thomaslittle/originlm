use originlm_adapters::grok::{login_recheck_replaces, parse_status, probe_environment};
use originlm_adapters::native_clients::launch::apply_grok_probe_environment;
use originlm_contracts::*;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
};
#[test]
fn native_grok_login_line_reports_auth_but_not_unpublished_plan_or_costs() {
    let (auth, billing) =
        parse_status("You are logged in with grok.com.\nAvailable models:\n * native-model\n");
    assert_eq!(auth.fresh_value(), Some(&AuthState::Authenticated));
    assert!(billing.value().is_none());
}
#[test]
fn absent_status_and_auth_failure_are_not_active_accounts() {
    let (auth, _) = parse_status("not authenticated");
    assert_eq!(auth.fresh_value(), Some(&AuthState::Unauthenticated));
    let (auth, _) = parse_status("Default model: example\nAvailable models:\n * native");
    assert!(auth.value().is_none());
}
#[test]
fn health_probes_disable_the_autoupdater_even_when_the_caller_left_it_off() {
    let mut env = BTreeMap::new();
    env.insert(
        OsString::from("GROK_DISABLE_AUTOUPDATER"),
        OsString::from("0"),
    );
    apply_grok_probe_environment(&mut env);
    assert_eq!(
        env.get(OsStr::new("GROK_DISABLE_AUTOUPDATER"))
            .and_then(|value| value.to_str()),
        Some("1")
    );
    assert_eq!(
        probe_environment()
            .get(OsStr::new("GROK_DISABLE_AUTOUPDATER"))
            .and_then(|value| value.to_str()),
        Some("1")
    );
}
#[test]
fn a_logged_out_probe_is_replaced_only_by_a_definite_recheck() {
    let (logged_out, _) = parse_status("not logged in");
    let (logged_in, _) = parse_status("You are logged in with grok.com.\n");
    let (unknown, _) = parse_status("Available models:\n * native");
    let (still_out, _) = parse_status("not authenticated");
    assert!(login_recheck_replaces(&logged_out, &logged_in));
    assert!(login_recheck_replaces(&logged_out, &still_out));
    assert!(!login_recheck_replaces(&logged_out, &unknown));
    assert!(!login_recheck_replaces(&logged_in, &logged_out));
    assert!(!login_recheck_replaces(&unknown, &logged_in));
}
#[test]
fn unrelated_output_is_not_preserved() {
    let (auth, _) = parse_status("secret-sentinel");
    assert!(
        !serde_json::to_string(&auth)
            .unwrap()
            .contains("secret-sentinel")
    );
}
