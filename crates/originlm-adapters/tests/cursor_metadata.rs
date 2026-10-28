use originlm_adapters::cursor::read_cached;
use originlm_discovery::LocatedClient;
use rusqlite::Connection;
use std::{fs, path::Path};
fn located(home: &Path) -> LocatedClient {
    LocatedClient {
        executable_identity: None,
        provider_id: "cursor".into(),
        profile_home: home.into(),
        executable: None,
    }
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("User/globalStorage");
    fs::create_dir_all(&dir).unwrap();
    let c = Connection::open(dir.join("state.vscdb")).unwrap();
    c.execute_batch("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT);")
        .unwrap();
    for (k, v) in [
        ("cursorAuth/cachedEmail", "cached@example.test"),
        ("cursorAuth/stripeMembershipType", "pro"),
        ("cursorAuth/accessToken", "SECRET_SENTINEL_ACCESS"),
        ("cursorAuth/stripeMembershipAuthId", "SECRET_SENTINEL_AUTH"),
    ] {
        c.execute("INSERT INTO ItemTable VALUES(?1,?2)", [k, v])
            .unwrap();
    }
    drop(c);
    root
}
#[test]
fn cached_identity_and_plan_are_stale_and_never_claim_live_auth() {
    let root = fixture();
    let m = read_cached(located(root.path()), "c", "r").unwrap();
    assert!(m.auth.value().is_none());
    assert!(m.billing.value().is_none());
    assert!(m.models.is_empty());
    assert!(m.quota_pools.is_empty());
    assert_eq!(m.plan.value().unwrap().native_id, "pro");
    assert!(m.plan.fresh_value().is_none());
    let account = m.account.unwrap();
    assert_eq!(
        account.display_label.value().unwrap(),
        "cached@example.test"
    );
    assert!(account.display_label.fresh_value().is_none());
}
#[test]
fn tokens_and_paths_are_not_exported_from_the_allowlisted_cache() {
    let root = fixture();
    let m = read_cached(located(root.path()), "c", "r").unwrap();
    let json = serde_json::to_string(&m).unwrap();
    assert!(!json.contains("SECRET_SENTINEL"));
    assert!(!json.contains(&root.path().to_string_lossy().to_string()));
}
#[test]
fn read_does_not_write_database_sidecars_or_change_original_bytes() {
    let root = fixture();
    let dir = root.path().join("User/globalStorage");
    let path = dir.join("state.vscdb");
    let before = fs::read(&path).unwrap();
    let entries = fs::read_dir(&dir).unwrap().count();
    read_cached(located(root.path()), "c", "r").unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
    assert_eq!(fs::read_dir(dir).unwrap().count(), entries);
}
#[test]
fn absent_and_corrupt_cache_is_unavailable_not_an_authenticated_account() {
    let root = tempfile::tempdir().unwrap();
    assert!(read_cached(located(root.path()), "c", "r").is_err());
    let dir = root.path().join("User/globalStorage");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("state.vscdb"), "SECRET_SENTINEL_CORRUPTION").unwrap();
    let error = read_cached(located(root.path()), "c", "r").unwrap_err();
    assert!(!error.to_string().contains("SECRET_SENTINEL"));
}
#[test]
fn profiles_remain_distinct_even_when_cached_email_matches() {
    let a = fixture();
    let b = fixture();
    let x = read_cached(located(a.path()), "personal", "r1").unwrap();
    let y = read_cached(located(b.path()), "work", "r2").unwrap();
    assert_ne!(x.account.unwrap().id, y.account.unwrap().id);
}
#[test]
fn malformed_cached_values_do_not_become_plan_claims() {
    let root = fixture();
    let c = Connection::open(root.path().join("User/globalStorage/state.vscdb")).unwrap();
    c.execute(
        "UPDATE ItemTable SET value=?1 WHERE key='cursorAuth/stripeMembershipType'",
        ["bad\nplan"],
    )
    .unwrap();
    drop(c);
    let m = read_cached(located(root.path()), "c", "r").unwrap();
    assert!(m.plan.value().is_none());
}

#[test]
fn large_cursor_database_is_read_through_bounded_indexed_lookups() {
    let root = fixture();
    let path = root.path().join("User/globalStorage/state.vscdb");
    let database = fs::OpenOptions::new().write(true).open(&path).unwrap();
    database.set_len(64 * 1024 * 1024 + 1).unwrap();
    drop(database);

    let metadata = read_cached(located(root.path()), "c", "r").unwrap();
    assert_eq!(metadata.plan.value().unwrap().native_id, "pro");
    assert!(metadata.auth.value().is_none());
    assert!(metadata.models.is_empty());
}
