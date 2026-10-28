use originlm_platform::watcher::MetadataWatcher;
use std::{
    collections::BTreeMap,
    fs,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn wait(events: &Arc<Mutex<Vec<String>>>) {
    let start = Instant::now();
    while events.lock().unwrap().is_empty() && start.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn watches_only_explicit_metadata_names_without_reading_contents() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("profile");
    fs::create_dir(&root).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    let watch = MetadataWatcher::start(
        BTreeMap::from([("connection".into(), root.clone())]),
        Arc::new(move |ids| copy.lock().unwrap().extend(ids)),
    )
    .unwrap();
    assert!(events.lock().unwrap().is_empty());
    fs::write(root.join("session.log"), "private activity; not watched").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert!(events.lock().unwrap().is_empty());
    fs::write(root.join("auth.json"), "a deliberately unread secret").unwrap();
    wait(&events);
    assert_eq!(events.lock().unwrap()[0], "connection");
    assert!(!format!("{:?}", events.lock().unwrap()).contains("secret"));
    drop(watch);
}
#[test]
fn atomic_replacement_and_deletion_invalidate_the_profile() {
    let temp = tempfile::tempdir().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let copy = events.clone();
    fs::write(temp.path().join("settings.json"), "old").unwrap();
    let watch = MetadataWatcher::start(
        BTreeMap::from([("a".into(), temp.path().into())]),
        Arc::new(move |ids| copy.lock().unwrap().extend(ids)),
    )
    .unwrap();
    fs::remove_file(temp.path().join("settings.json")).unwrap();
    wait(&events);
    assert!(!events.lock().unwrap().is_empty());
    drop(watch);
    events.lock().unwrap().clear();
    fs::write(temp.path().join("settings.json"), "new").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert!(events.lock().unwrap().is_empty());
}
#[test]
fn invalid_roots_fail_without_creating_or_following_them() {
    assert!(
        MetadataWatcher::start(
            BTreeMap::from([("a".into(), "relative".into())]),
            Arc::new(|_| {})
        )
        .is_err()
    );
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("absent");
    assert!(
        MetadataWatcher::start(
            BTreeMap::from([("a".into(), missing.clone())]),
            Arc::new(|_| {})
        )
        .is_err()
    );
    assert!(!missing.exists());
}
