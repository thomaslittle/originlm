use originlm_adapters::{claude::client::*, process::LaunchSpec};
use originlm_contracts::*;
use originlm_runtime::Runtime;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
fn spec(mode: &str) -> LaunchSpec {
    LaunchSpec {
        executable: std::env::var_os("LAI_TEST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    "C:/Python313/python.exe".into()
                } else {
                    "/usr/bin/python3".into()
                }
            }),
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/support/claude_peer.py")
                .into(),
            mode.into(),
        ],
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    }
}
async fn setup(mode: &str) -> (Runtime, Session) {
    let report = probe_spec(spec("normal"), "c", "r").await.unwrap();
    let model = report.models[0].clone();
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: "r".into(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: BTreeMap::from([("reasoningEffort".into(), json!("future-effort"))]),
            },
            executor(spec(mode), model, "c".into(), "r".into()),
            Arc::new(|| Ok(())),
        )
        .unwrap();
    (runtime, session)
}
async fn done(r: &Runtime, s: &Session) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while matches!(
            r.get("owner", &s.id).unwrap().state,
            SessionState::Running | SessionState::AwaitingApproval
        ) {
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn metadata_control_never_needs_a_user_prompt() {
    let r = probe_spec(spec("normal"), "c", "r").await.unwrap();
    assert_eq!(r.plan.value().unwrap().native_id, "Claude Test Plan");
    assert_eq!(r.models.len(), 1);
}
#[tokio::test]
async fn native_stream_and_whole_message_do_not_duplicate_text() {
    let (r, s) = setup("normal").await;
    r.start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    done(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    let events = r.events("owner", None, None).unwrap();
    assert_eq!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["payload"]["type"] == "text_delta")
            .count(),
        1
    );
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn text_only_mode_denies_unexpected_native_tool_requests() {
    let (r, s) = setup("tool").await;
    r.start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    done(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
}
#[tokio::test]
async fn cancellation_terminates_without_restarting_native_turn() {
    let (r, s) = setup("hang").await;
    let t = r
        .start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    r.cancel("owner", t["turnId"].as_str().unwrap())
        .await
        .unwrap();
    done(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Cancelled
    );
}
#[tokio::test]
async fn effort_change_requires_a_new_native_session() {
    let (r, s) = setup("normal").await;
    r.start("owner", &s.id, "test".into(), "1".into())
        .await
        .unwrap();
    done(&r, &s).await;
    r.update_options("owner", &s.id, BTreeMap::new()).unwrap();
    r.start("owner", &s.id, "test".into(), "2".into())
        .await
        .unwrap();
    done(&r, &s).await;
    assert_eq!(r.get("owner", &s.id).unwrap().state, SessionState::Error);
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
async fn images_are_sent_as_base64_content_blocks_before_the_text() {
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
    done(&r, &s).await;
    assert_eq!(
        r.get("owner", &s.id).unwrap().state,
        SessionState::Completed
    );
    let events = r.events("owner", None, None).unwrap().to_string();
    assert!(
        !events.contains("AQID"),
        "image payload must not reach events"
    );
    r.close("owner", &s.id).await.unwrap();
}
#[tokio::test]
async fn claude_catalog_reports_image_input() {
    let report = probe_spec(spec("normal"), "c", "r").await.unwrap();
    assert!(
        report.models[0]
            .capabilities
            .iter()
            .any(|c| c.id == "input.image" && c.support == Support::Supported)
    );
}
