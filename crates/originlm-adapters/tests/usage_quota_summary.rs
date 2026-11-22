use chrono::{TimeZone, Utc};
use originlm_adapters::{claude::parse_metadata, plan_usage::summary_windows};
use originlm_contracts::*;
use serde_json::{Value, json};

fn source() -> EvidenceSource {
    EvidenceSource::new(EvidenceKind::CommunityEndpoint, "antigravity", "test")
}

fn used(window: &UsageWindow) -> String {
    match window.reading.value() {
        Some(reading) => serde_json::to_value(&reading.used)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        None => panic!("window has no reading"),
    }
}

/// Shapes the daily and production hosts return for one login.
fn daily() -> Value {
    json!({"groups":[
        {"displayName":"Gemini Models","buckets":[
            {"bucketId":"gemini-weekly","window":"weekly","remainingFraction":0.2,"resetTime":"2026-11-28T14:32:55Z"},
            {"bucketId":"gemini-5h","window":"5h","remainingFraction":0.5,"resetTime":"2026-11-22T13:13:41Z"}]},
        {"displayName":"Claude and GPT models","buckets":[
            {"bucketId":"3p-5h","window":"5h","remainingFraction":1}]},
        {"displayName":"Gemini 4 Argon","buckets":[
            {"bucketId":"gemini-argon-wtu-weekly","window":"weekly","remainingFraction":1,"resetTime":"2026-11-29T09:47:57Z"}]}]})
}

fn production() -> Value {
    json!({"groups":[{"displayName":"Gemini Models","buckets":[
        {"bucketId":"gemini-weekly","window":"weekly","remainingFraction":1,"resetTime":"2026-11-29T09:47:58Z"},
        {"bucketId":"gemini-5h","window":"5h","remainingFraction":1,"resetTime":"2026-11-22T14:47:58Z"}]}]})
}

#[test]
fn the_daily_reading_wins_and_windows_state_their_length() {
    let now = Utc.with_ymd_and_hms(2026, 11, 22, 9, 47, 58).unwrap();
    let windows = summary_windows("pool", &[daily(), production()], now, &source());
    let by_label = |label: &str| windows.iter().find(|w| w.label == label).unwrap();
    assert_eq!(windows.len(), 4);
    assert_eq!(used(by_label("Gemini · 5-hour")), "50");
    assert_eq!(by_label("Gemini · 5-hour").duration_seconds, Some(18_000));
    assert_eq!(used(by_label("Gemini · Weekly")), "80");
    assert_eq!(by_label("Gemini · Weekly").duration_seconds, Some(604_800));
    assert_eq!(used(by_label("Claude & GPT (shared) · 5-hour")), "0");
    assert_eq!(used(by_label("Gemini 4 Argon · Weekly")), "0");
}

#[test]
fn a_window_never_drawn_on_has_no_invented_reset() {
    // Reset exactly one window length after the request: not a real reset.
    let now = Utc.with_ymd_and_hms(2026, 11, 22, 9, 47, 58).unwrap();
    let fresh = json!({"buckets":[
        {"bucketId":"3p-5h","window":"5h","remainingFraction":1,"resetTime":"2026-11-22T14:47:58Z"}]});
    let windows = summary_windows("pool", &[fresh], now, &source());
    assert_eq!(windows.len(), 1);
    assert!(windows[0].resets_at.is_none());
}

#[test]
fn disabled_and_unreadable_buckets_are_skipped_not_zero() {
    let now = Utc.with_ymd_and_hms(2026, 11, 22, 9, 47, 58).unwrap();
    let summary = json!({"buckets":[
        {"bucketId":"gemini-5h","window":"5h","disabled":true,"remainingFraction":0.5},
        {"bucketId":"gemini-weekly","window":"weekly"}]});
    assert!(summary_windows("pool", &[summary], now, &source()).is_empty());
}

#[test]
fn claude_oauth_usage_payload_yields_session_weekly_and_scoped_windows() {
    let payload = json!({
        "five_hour":{"utilization":27.0,"resets_at":"2026-11-22T12:00:00.250985+00:00"},
        "seven_day":{"utilization":25.0,"resets_at":"2026-11-28T21:00:00.251004+00:00"},
        "spend":{"enabled":false},
        "extra_usage":{"is_enabled":false},
        "limits":[
            {"kind":"session","percent":27,"resets_at":"2026-11-22T12:00:00.250985+00:00","scope":null},
            {"kind":"weekly_all","percent":25,"resets_at":"2026-11-28T21:00:00.251004+00:00","scope":null},
            {"kind":"weekly_scoped","percent":6,"resets_at":"2026-11-28T21:00:00.251137+00:00","scope":{"model":{"id":null,"display_name":"Fable"}}}]});
    let usage = json!({"rate_limits_available":true,"rate_limits":payload});
    let init = json!({"account":{"subscriptionType":"max"},"models":[],"current_permission_mode":"dontAsk"});
    let report = parse_metadata("c", "r", &init, Some(&usage)).unwrap();
    let windows = &report.quota_pools[0].windows;
    let labels: Vec<_> = windows.iter().map(|w| w.label.as_str()).collect();
    assert_eq!(labels, ["Session", "Weekly", "Weekly · Fable"]);
    assert_eq!(used(&windows[2]), "6");
}
