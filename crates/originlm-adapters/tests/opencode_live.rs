//! Opt-in live check against an installed OpenCode. Skipped unless
//! `LAI_LIVE_OPENCODE=<opencode shim or exe>`; `LAI_LIVE_OPENCODE_MODEL` names
//! the native model for the single text turn (use a free one).
use originlm_contracts::*;
use originlm_runtime::Runtime;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
#[tokio::test]
async fn live_managed_server_runs_one_text_turn() {
    let (Some(path), Some(wanted)) = (
        std::env::var_os("LAI_LIVE_OPENCODE"),
        std::env::var("LAI_LIVE_OPENCODE_MODEL").ok(),
    ) else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let report = originlm_adapters::opencode::server::probe(&path, "c", "r")
        .await
        .unwrap();
    assert_eq!(report.auth.value(), Some(&AuthState::Authenticated));
    let model = report
        .models
        .iter()
        .find(|m| m.native_id == wanted)
        .expect("model listed")
        .clone();
    let provider = report
        .additional_routes
        .iter()
        .find(|r| r.id == model.route_id)
        .map(|r| r.provider_id.clone())
        .expect("route");
    let workspace = tempfile::tempdir().unwrap();
    let agent = originlm_adapters::opencode::server::open(
        &path,
        model.clone(),
        provider,
        workspace.path().canonicalize().unwrap(),
    )
    .await
    .unwrap();
    let runtime = Runtime::new();
    let session = runtime
        .insert(
            "owner",
            "c".into(),
            Selection {
                route_id: model.route_id.clone(),
                model_id: model.id.clone(),
                revision: model.revision.clone(),
                options: BTreeMap::new(),
            },
            agent,
            Arc::new(|| Ok(())),
        )
        .unwrap();
    runtime
        .start(
            "owner",
            &session.id,
            "Reply with the single word OK.".into(),
            "1".into(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(120), async {
        while !matches!(
            runtime.get("owner", &session.id).unwrap().state,
            SessionState::Completed | SessionState::Error | SessionState::Cancelled
        ) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let state = runtime.get("owner", &session.id).unwrap().state;
    let events = runtime.events("owner", None, None).unwrap().to_string();
    eprintln!("LIVE state={state:?} {events}");
    runtime.close("owner", &session.id).await.unwrap();
    assert_eq!(state, SessionState::Completed);
    assert!(!events.contains("\"type\":\"tool\""));
}
