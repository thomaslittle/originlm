use originlm_contracts::ErrorCode;
use originlm_runtime::Runtime;
use serde_json::json;
#[test]
fn a_new_clients_event_page_has_a_valid_tail_cursor() {
    let runtime = Runtime::new();
    let page = runtime.events("new-owner", None, None).unwrap();
    assert_eq!(
        page,
        json!({"events":[],"cursor":"0","tailCursor":"0","hasMore":false})
    );
}
#[test]
fn a_cursor_ahead_of_an_empty_journal_is_a_gap_not_silent_success() {
    let runtime = Runtime::new();
    assert_eq!(
        runtime.events("new-owner", None, Some(1)).unwrap_err().code,
        ErrorCode::EventGap
    );
}
