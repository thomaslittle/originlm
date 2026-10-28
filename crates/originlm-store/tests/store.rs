use originlm_contracts::*;
use originlm_store::{DurableMetadata, RecoveryNotice, SCHEMA_VERSION, StateStore, StoreError};
use std::path::Path;
use tempfile::TempDir;

fn snapshot() -> Snapshot {
    let mut snapshot = Snapshot::empty(DataMode::Live);
    snapshot.observed_at = Timestamp::now();
    snapshot
}

fn change(id: &str) -> ResourceChange {
    ResourceChange {
        resource: "connection".into(),
        id: id.into(),
        change: ResourceChangeKind::Updated,
        reason: Some("fixture".into()),
    }
}

fn open(directory: &TempDir) -> StateStore {
    let (store, notice) = StateStore::open(directory.path().join("state.db")).unwrap();
    assert!(notice.is_none());
    store
}

#[test]
fn commit_reloads_snapshot_and_replays_the_exact_journal() {
    let directory = TempDir::new().unwrap();
    let store = open(&directory);
    let mut first = snapshot();
    let committed = store
        .commit_snapshot(&mut first, None, "scan", vec![change("personal")], false)
        .unwrap();
    assert_eq!(committed.revision, "1");
    assert_eq!(first.revision, "1");
    assert_eq!(first.cursor, committed.cursor);

    let mut second = snapshot();
    let second_commit = store
        .commit_snapshot(
            &mut second,
            None,
            "metadata",
            vec![change("personal")],
            false,
        )
        .unwrap();
    assert_eq!(second_commit.previous_revision, "1");
    assert_eq!(second_commit.revision, "2");

    let page = store.read_journal(&first.cursor).unwrap();
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].revision, "2");
    assert_eq!(page.events[0].changes[0].id, "personal");
    assert_eq!(page.cursor, second.cursor);
    assert!(!page.has_more);

    drop(store);
    let reloaded = open(&directory).load().unwrap();
    assert_eq!(reloaded.revision, 2);
    assert_eq!(reloaded.snapshot.unwrap().revision, "2");
    assert_eq!(reloaded.tail_sequence, 2);
}

#[test]
fn an_old_epoch_or_a_future_cursor_is_an_explicit_gap() {
    let directory = TempDir::new().unwrap();
    let store = open(&directory);
    let mut first = snapshot();
    store
        .commit_snapshot(&mut first, None, "scan", vec![], false)
        .unwrap();
    assert!(matches!(
        store.read_journal("deadbeef:0"),
        Err(StoreError::EventGap)
    ));
    assert!(matches!(
        store.read_journal(&format!("{}:99", store.epoch())),
        Err(StoreError::EventGap)
    ));
}

#[test]
fn journal_is_bounded_and_evicts_old_cursors_without_losing_the_snapshot() {
    let directory = TempDir::new().unwrap();
    let store = open(&directory);
    let mut first = snapshot();
    let earliest = store
        .commit_snapshot(&mut first, None, "scan", vec![], false)
        .unwrap()
        .cursor;
    for index in 0..2100 {
        let mut next = snapshot();
        store
            .commit_snapshot(
                &mut next,
                None,
                "metadata",
                vec![change(&index.to_string())],
                false,
            )
            .unwrap();
    }
    assert!(matches!(
        store.read_journal(&earliest),
        Err(StoreError::EventGap)
    ));
    let latest = store.load().unwrap();
    assert_eq!(latest.tail_sequence, 2101);
    let page = store
        .read_journal(&format!("{}:2050", store.epoch()))
        .unwrap();
    assert!(page.events.len() <= 256);
    assert_eq!(page.revision, "2101");
}

#[test]
fn a_corrupt_database_is_preserved_not_deleted() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.db");
    std::fs::write(&path, b"this is not sqlite").unwrap();
    let (store, notice) = StateStore::open(&path).unwrap();
    let notice: RecoveryNotice = notice.expect("recovery must be visible");
    assert!(
        notice
            .backup_path
            .as_ref()
            .is_some_and(|backup| backup.exists())
    );
    assert!(Path::new(store.path()).exists());
    assert_eq!(store.load().unwrap().revision, 0);
}

#[test]
fn a_future_schema_fails_closed() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.db");
    let store = StateStore::open(&path).unwrap().0;
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            [format!("{}", SCHEMA_VERSION + 1)],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        StateStore::open(&path),
        Err(StoreError::UnsupportedSchema(_))
    ));
}

#[test]
fn read_only_diagnostics_do_not_create_or_modify_state() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.db");
    {
        let store = StateStore::open(&path).unwrap().0;
        let mut snapshot = snapshot();
        store
            .commit_snapshot(&mut snapshot, None, "scan", vec![], false)
            .unwrap();
    }
    let store = StateStore::open_read_only(&path).unwrap();
    assert_eq!(store.load().unwrap().revision, 1);
    let missing = directory.path().join("missing.db");
    assert!(StateStore::open_read_only(missing).is_err());
}

#[test]
fn persisted_metadata_never_contains_account_identity_text() {
    let directory = TempDir::new().unwrap();
    let store = open(&directory);
    let mut report = ConnectionMetadata {
        connection_id: "personal".into(),
        route_id: "route".into(),
        version: Observation::unknown("TEST"),
        auth: Observation::unknown("TEST"),
        plan: Observation::unknown("TEST"),
        billing: Observation::unknown("TEST"),
        account: Some(Account {
            id: "account-1".into(),
            provider_id: "fixture".into(),
            display_label: Observation::reported(
                "private@example.test".into(),
                EvidenceSource::new(EvidenceKind::Synthetic, "test", "fixture"),
            ),
            identity_source: Observation::unknown("TEST"),
            scopes: vec![],
        }),
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("TEST"),
        diagnostics: vec![],
    };
    let mut snapshot = snapshot();
    store
        .commit_snapshot(
            &mut snapshot,
            Some(DurableMetadata {
                observed_at_ms: 1,
                report: report.clone(),
            }),
            "metadata",
            vec![],
            false,
        )
        .unwrap();
    drop(store);
    let bytes = std::fs::read(directory.path().join("state.db")).unwrap();
    assert!(
        !bytes
            .windows(20)
            .any(|window| window == b"private@example.test")
    );
    let reloaded = open(&directory).load().unwrap();
    let account = reloaded.metadata[0].report.account.as_ref().unwrap();
    assert_eq!(account.display_label, Observation::unknown("NOT_PERSISTED"));
    // The in-memory copy may still carry the live identity; only durable state is masked.
    report.account.as_mut().unwrap().display_label = Observation::unknown("NOT_PERSISTED");
    assert_eq!(report.account.unwrap().display_label, account.display_label);
}

#[test]
fn notification_dedupe_state_survives_a_restart() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.db");
    {
        let store = StateStore::open(&path).unwrap().0;
        assert!(!store.notification_seen("issue", "key").unwrap());
        store
            .record_notification("issue", "key", "warning", 42)
            .unwrap();
        assert_eq!(
            store.record_notification_time("issue", "key").unwrap(),
            Some(42)
        );
    }
    let store = StateStore::open(&path).unwrap().0;
    assert!(store.notification_seen("issue", "key").unwrap());
}
