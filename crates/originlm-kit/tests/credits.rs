use originlm_kit::Kit;
#[test]
fn credits_are_default_on_but_hosts_can_hide_them() {
    let kit = Kit::builder().build().unwrap();
    let credits = kit.credits().expect("default credits");
    assert_eq!(credits.label, "Runs on OriginLM");
    assert_eq!(credits.product_name, "OriginLM");
    assert_eq!(credits.mark_reference, "originlm-mark");
    assert_eq!(credits.url, "https://originlm.livestreaming.tools");
    let hidden = Kit::builder().show_attribution(false).build().unwrap();
    assert!(hidden.credits().is_none());
}
