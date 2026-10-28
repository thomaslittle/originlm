use originlm_adapters::{acp::client, process::LaunchSpec};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
fn spec(mode: &str) -> LaunchSpec {
    LaunchSpec {
        executable: std::env::var_os("LAI_TEST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(if cfg!(windows) {
                    "C:/Python313/python.exe"
                } else {
                    "/usr/bin/python3"
                })
            }),
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/acp_peer.py")
                .into(),
            mode.into(),
        ],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}
async fn setup(mode: &str) -> (Runtime, Session) {
    setup_with_review(mode, true).await
}
async fn setup_with_review(mode: &str, review: bool) -> (Runtime, Session) {
    let c = client::inspect(spec("normal"), "fixture", "r")
        .await
        .unwrap();
    let model = c.models[0].clone();
    let agent = client::open(
        spec(mode),
        "fixture".into(),
        "r".into(),
        model.clone(),
        review,
    )
    .await
    .unwrap();
    let r = Runtime::new();
    let s = r
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: model.id,
                revision: model.revision,
                options: BTreeMap::from([("thought".into(), json!("future"))]),
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
                SessionState::Completed | SessionState::Cancelled | SessionState::Error
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
async fn actual_acp_exchange_preserves_native_config_without_generation() {
    let c = client::inspect(spec("normal"), "fixture", "r")
        .await
        .unwrap();
    assert_eq!(c.models.len(), 2);
    assert_eq!(c.config_options[1].id, "thought");
}
#[tokio::test]
async fn negotiation_mismatch_is_rejected_before_session() {
    assert!(
        matches!(client::inspect(spec("version"),"fixture","r").await,Err(e) if e.code==ErrorCode::ProtocolIncompatible)
    );
}
#[tokio::test]
async fn native_stream_arrives_before_prompt_response_and_session_reuses_context() {
    let (r, s) = setup("normal").await;
    for i in 1..=2 {
        r.start("owner", &s.id, "hello".into(), i.to_string())
            .await
            .unwrap();
        finish(&r, &s).await;
        assert_eq!(
            r.get("owner", &s.id).unwrap().state,
            SessionState::Completed
        );
    }
    let text = r.events("owner", None, None).unwrap().to_string();
    assert!(text.contains("native reply 1") && text.contains("native reply 2"));
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn session_notice_is_not_assistant_text() {
    let (runtime, session) = setup("notice").await;
    runtime
        .start("owner", &session.id, "hello".into(), "1".into())
        .await
        .unwrap();
    finish(&runtime, &session).await;
    assert_eq!(
        runtime.get("owner", &session.id).unwrap().state,
        SessionState::Completed
    );
    let events = runtime.events("owner", Some(&session.id), None).unwrap();
    let payloads: Vec<_> = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| &event["payload"])
        .collect();
    let text: String = payloads
        .iter()
        .filter(|payload| payload["type"] == "text_delta")
        .map(|payload| payload["text"].as_str().unwrap())
        .collect();
    assert_eq!(text, "native reply");
    let notices: Vec<_> = payloads
        .iter()
        .filter(|payload| payload["code"] == "ACP_SESSION_NOTICE")
        .collect();
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0]["message"],
        "[warning] Quota refresh\n\nremaining"
    );
    assert!(
        payloads
            .iter()
            .any(|payload| payload["code"] == "UNKNOWN_ACP_UPDATE")
    );
    assert!(!events.to_string().contains("secret-notice-sentinel"));
}
#[tokio::test]
async fn foreign_session_cannot_inject_output() {
    let (r, s) = setup("foreign").await;
    r.start("owner", &s.id, "hello".into(), "1".into())
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
async fn provider_error_message_reaches_diagnostics_with_secrets_redacted() {
    let (runtime, session) = setup("prompt_error").await;
    runtime
        .start("owner", &session.id, "hello".into(), "1".into())
        .await
        .unwrap();
    finish(&runtime, &session).await;
    let events = runtime.events("owner", Some(&session.id), None).unwrap();
    let error = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["payload"]["type"] == "error")
        .unwrap();
    assert_eq!(error["payload"]["code"], "PROVIDER_UNAVAILABLE");
    assert!(
        error["payload"]["message"]
            .as_str()
            .unwrap()
            .contains("HTTP 401")
    );
    assert!(
        error["payload"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid account")
    );
    assert!(
        error["payload"]["message"]
            .as_str()
            .unwrap()
            .contains("Bearer [REDACTED]")
    );
    assert!(!events.to_string().contains("private-peer-token"));
}
#[tokio::test]
async fn native_cancel_completes_without_prompt_retry() {
    let (r, s) = setup("hang").await;
    let t = r
        .start("owner", &s.id, "hello".into(), "1".into())
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
async fn file_callbacks_not_advertised_are_denied() {
    let (r, s) = setup("fs").await;
    r.start("owner", &s.id, "hello".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
    assert!(
        !r.events("owner", None, None)
            .unwrap()
            .to_string()
            .contains("private-sentinel")
    );
}
#[tokio::test]
async fn provider_switch_during_turn_fails_without_silent_model_fallback() {
    let (r, s) = setup("changed").await;
    r.start("owner", &s.id, "hello".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
}
#[tokio::test]
async fn approval_is_scoped_and_rejection_reaches_native_agent() {
    let (r, s) = setup("permission").await;
    r.start("owner", &s.id, "hello".into(), "1".into())
        .await
        .unwrap();
    let id = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let events = r.events("owner", None, None).unwrap();
            if let Some(e) = events["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["payload"]["type"] == "approval_request")
            {
                break e["payload"]["approvalId"].as_str().unwrap().to_owned();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(r.approve("other", &id, true).is_err());
    r.approve("owner", &id, false).unwrap();
    finish(&r, &s).await;
    assert!(
        r.events("owner", None, None)
            .unwrap()
            .to_string()
            .contains("denied")
    );
}
fn fixture_image() -> originlm_runtime::ImageInput {
    originlm_runtime::ImageInput {
        media_type: InputMediaType::Jpeg,
        bytes: b"\xff\xd8fixture".as_slice().into(),
        width: 1,
        height: 1,
    }
}
#[tokio::test]
async fn image_blocks_reach_peers_that_advertise_prompt_image_support() {
    let (r, s) = setup("image").await;
    r.start_with_inputs(
        "owner",
        &s.id,
        "describe".into(),
        "1".into(),
        vec![fixture_image()],
    )
    .await
    .unwrap();
    finish(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    let text = r.events("owner", None, None).unwrap().to_string();
    assert!(
        text.contains("text=describe images=image/jpeg:/9hmaXh0dXJl"),
        "{text}"
    );
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn peers_without_prompt_image_support_reject_image_turns_before_prompting() {
    let (r, s) = setup("normal").await;
    let error = r
        .start_with_inputs(
            "owner",
            &s.id,
            "describe".into(),
            "1".into(),
            vec![fixture_image()],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Ready);
    // The session remains usable for text turns.
    r.start("owner", &s.id, "hello".into(), "2".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn text_only_session_fails_the_turn_when_the_client_uses_a_native_tool() {
    let (r, s) = setup_with_review("tool", false).await;
    r.start("owner", &s.id, "hello".into(), "1".into())
        .await
        .unwrap();
    finish(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
    let events = r.events("owner", None, None).unwrap().to_string();
    assert!(events.contains("INTEGRATION_RESTRICTED"));
    assert!(!events.contains("tool-output-sentinel"));
}
