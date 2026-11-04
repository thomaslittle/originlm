use originlm_adapters::{codex::parse_metadata, execution::*, process::LaunchSpec};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
fn config() -> CodexExecution {
    let model = json!({"id":"native-id","model":"test-native","displayName":"Test native","description":"Fixture","hidden":false,"isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"future","description":"Future"}],"defaultReasoningEffort":"future","inputModalities":["text"]});
    let report=parse_metadata("c","r",&json!({"userAgent":"codex/0.156.1"}),&json!({"account":{"type":"chatgpt","email":"test@example.test","planType":"test"},"requiresOpenaiAuth":true}),&[model],Some(&json!({"rateLimits":{}}))).unwrap();
    CodexExecution {
        model: report.models[0].clone(),
        connection: "c".into(),
        route: "r".into(),
        writable: false,
        review_requests: true,
        native_policy: false,
        host_mcp_servers: vec![],
        persistent: false,
        resume_thread: None,
    }
}
fn spec(mode: &str) -> LaunchSpec {
    let python = std::env::var_os("LAI_TEST_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                PathBuf::from("C:/Python313/python.exe")
            } else {
                PathBuf::from("/usr/bin/python3")
            }
        });
    LaunchSpec {
        executable: python,
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/execution_peer.py")
                .into(),
            mode.into(),
        ],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}
async fn setup(mode: &str) -> (Runtime, Session) {
    let r = Runtime::new();
    let c = config();
    let agent = open_codex(spec(mode), c.clone()).await.unwrap();
    let s = r
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: c.model.id,
                revision: c.model.revision,
                options: BTreeMap::from([("reasoningEffort".into(), json!("future"))]),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    (r, s)
}
async fn finish(r: &Runtime, s: &Session) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if matches!(
                r.get("owner", &s.id).unwrap().state,
                SessionState::Completed | SessionState::Error | SessionState::Cancelled
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn native_text_streams_including_events_before_ack() {
    let (r, s) = setup("early").await;
    r.start("owner", &s.id, "hi".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    let e = r.events("owner", None, None).unwrap().to_string();
    assert!(e.contains("early "));
    assert!(e.contains("native reply"));
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn changed_model_catalog_cannot_start_generation() {
    let mut c = config();
    c.model.revision = "old".into();
    assert!(
        matches!(open_codex(spec("normal"),c).await,Err(e) if e.code==ErrorCode::SelectionStale)
    );
}
#[tokio::test]
async fn unsafe_sandbox_response_fails_before_turn() {
    let (r, s) = setup("unsafe").await;
    r.start("owner", &s.id, "hi".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
    assert!(
        !r.events("owner", None, None)
            .unwrap()
            .to_string()
            .contains("native reply")
    );
}
#[tokio::test]
async fn native_cancel_interrupts_without_a_second_turn() {
    let (r, s) = setup("hang").await;
    let t = r
        .start("owner", &s.id, "hi".into(), "1".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    r.cancel("owner", t["turnId"].as_str().unwrap())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Cancelled
    );
}
#[tokio::test]
async fn native_approval_denial_reaches_client() {
    let (r, s) = setup("approval").await;
    r.start("owner", &s.id, "hi".into(), "1".into())
        .await
        .unwrap();
    let approval = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let e = r.events("owner", None, None).unwrap();
            if let Some(x) = e["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["payload"]["type"] == "approval_request")
            {
                break x["payload"]["approvalId"].as_str().unwrap().to_owned();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    r.approve("owner", &approval, false).unwrap();
    finish(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn account_switch_aborts_without_retry() {
    let (r, s) = setup("account-switch").await;
    r.start("owner", &s.id, "hi".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
}

#[tokio::test]
async fn token_refresh_mid_turn_keeps_the_session() {
    let (r, s) = setup("token-refresh").await;
    for i in ["1", "2"] {
        r.start("owner", &s.id, "hi".into(), i.into())
            .await
            .unwrap();
        finish(&r, &s).await;
        assert_eq!(
            r.get("owner", &s.id).unwrap().state,
            SessionState::Completed
        );
    }
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn account_is_rechecked_before_submitting_each_turn() {
    let (r, s) = setup("account-changed-before-turn").await;
    r.start("owner", &s.id, "hi".into(), "first".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
    assert!(
        !r.events("owner", None, None)
            .unwrap()
            .to_string()
            .contains("native reply")
    );
}

fn fixture_image() -> originlm_runtime::ImageInput {
    originlm_runtime::ImageInput {
        media_type: InputMediaType::Png,
        bytes: vec![1u8, 2, 3].into(),
        width: 1,
        height: 1,
    }
}
#[tokio::test]
async fn images_are_sent_as_app_server_image_items_after_the_text() {
    let (r, s) = setup("image").await;
    r.start_with_inputs(
        "owner",
        &s.id,
        "describe".into(),
        "1".into(),
        vec![fixture_image(), fixture_image()],
    )
    .await
    .unwrap();
    finish(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    assert!(
        !r.events("owner", None, None)
            .unwrap()
            .to_string()
            .contains("AQID")
    );
    r.close("owner", &s.id).await.unwrap();
}

fn usage_units(r: &Runtime, _s: &Session) -> BTreeMap<String, String> {
    r.events("owner", None, None).unwrap()["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["payload"]["type"] == "usage")
        .map(|event| {
            (
                event["payload"]["unit"].as_str().unwrap().to_owned(),
                event["payload"]["used"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn codex_thread_totals_become_this_turn_s_delta() {
    let (r, s) = setup("tokens").await;
    r.start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    let usage = usage_units(&r, &s);
    assert_eq!(usage.get("input_tokens").map(String::as_str), Some("30"));
    assert_eq!(usage.get("output_tokens").map(String::as_str), Some("10"));
    assert_eq!(
        usage.get("cached_input_tokens").map(String::as_str),
        Some("20")
    );
    assert_eq!(
        usage.get("cache_creation_tokens").map(String::as_str),
        Some("2")
    );
    assert_eq!(usage.get("reasoning_tokens").map(String::as_str), Some("2"));
    assert!(
        !usage.contains_key("session_tokens"),
        "a per-turn delta must not be labelled as a thread-wide session total"
    );
    r.close("owner", &s.id).await.unwrap();
}

#[tokio::test]
async fn codex_counter_reset_uses_the_reported_last_response() {
    let (r, s) = setup("tokens-reset").await;
    r.start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    let usage = usage_units(&r, &s);
    assert_eq!(usage.get("input_tokens").map(String::as_str), Some("40"));
    assert_eq!(usage.get("output_tokens").map(String::as_str), Some("8"));
    assert_eq!(
        usage.get("cached_input_tokens").map(String::as_str),
        Some("10")
    );
    r.close("owner", &s.id).await.unwrap();
}

#[tokio::test]
async fn a_late_prior_turn_token_event_fails_closed_without_counting_it() {
    let (r, s) = setup("tokens-stale").await;
    r.start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Error,
        "a delayed event from another turn is not folded into this turn"
    );
    assert!(
        !usage_units(&r, &s).contains_key("input_tokens"),
        "the stale turn's tokens must not reach this turn"
    );
}
