use originlm_discovery::ScanResult;
use originlm_kit::*;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
#[tokio::test]
async fn building_and_reading_do_not_scan_or_probe() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let kit = Kit::builder()
        .inventory(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: Default::default(),
                metadata_probes: 0,
            })
        }))
        .build()
        .unwrap();
    let app = kit.connect("my-app", ClientPolicy::passive()).unwrap();
    assert!(app.snapshot().await.unwrap().connections.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    app.scan().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn one_import_provides_real_passive_discovery() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join(".codex")).unwrap();
    let kit = Kit::builder().home(temp.path().into()).build().unwrap();
    let app = kit.connect("demo", ClientPolicy::passive()).unwrap();
    let snapshot = app.scan().await.unwrap();
    assert_eq!(snapshot.mode, DataMode::Live);
    assert_eq!(snapshot.connections.len(), 1);
    assert!(snapshot.models.is_empty());
    assert!(snapshot.connections[0].plan.value().is_none());
}
#[tokio::test]
async fn denied_and_same_label_clients_are_isolated() {
    let tmp = tempfile::tempdir().unwrap();
    let kit = Kit::builder().home(tmp.path().into()).build().unwrap();
    let allowed = kit.connect("same-label", ClientPolicy::passive()).unwrap();
    let denied = kit.connect("same-label", ClientPolicy::denied()).unwrap();
    assert!(denied.scan().await.is_err());
    denied.close();
    assert!(allowed.scan().await.is_ok());
    allowed.close();
    assert!(allowed.snapshot().await.is_err());
}
#[test]
fn relative_home_is_rejected_without_creating_a_directory() {
    assert!(Kit::builder().home("relative-home".into()).build().is_err());
}
#[tokio::test]
async fn explicit_native_profiles_are_inert_until_scan_and_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    let document = serde_json::json!({"profiles":[{"id":"google","label":"Google CLI","provider":"gemini_cli","executable":root.path().join("gemini.exe"),"profileHome":root.path().join("gemini-home")},{"id":"grok","label":"Grok","provider":"grok","executable":root.path().join("grok.exe"),"profileHome":root.path().join(".grok"),"billing":"subscription"}]});
    let native = NativeClientRegistry::parse(&document.to_string(), "embedded-test").unwrap();
    let kit = Kit::builder()
        .home(root.path().into())
        .native_clients(native)
        .build()
        .unwrap();
    let mut policy = ClientPolicy::passive();
    policy.probe_provider_ids.insert("gemini_cli".into());
    let app = kit.connect("native-demo", policy).unwrap();
    assert!(app.snapshot().await.unwrap().connections.is_empty());
    let snapshot = app.scan().await.unwrap();
    assert_eq!(
        snapshot
            .connections
            .iter()
            .filter(|c| ["gemini_cli", "grok"].contains(&c.provider_id.as_str()))
            .count(),
        2
    );
    assert!(snapshot.models.is_empty());
    assert!(
        snapshot
            .connections
            .iter()
            .all(|c| c.auth.fresh_value() != Some(&AuthState::Authenticated))
    );
    let google = snapshot
        .connections
        .iter()
        .find(|c| c.provider_id == "gemini_cli")
        .unwrap();
    assert_eq!(
        app.probe(&google.id).await.unwrap_err().code,
        ErrorCode::RuntimeMissing
    );
    let adapters: serde_json::Value = app
        .request("adapters.list", serde_json::json!({}))
        .await
        .unwrap();
    for id in ["gemini_cli", "antigravity", "grok"] {
        let item = adapters
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap();
        assert_eq!(item["metadata"], true);
        assert_eq!(item["textSessions"], true);
    }
}
