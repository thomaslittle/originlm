use originlm_contracts::Result;
use originlm_discovery::*;
use originlm_platform::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

struct MetadataOnly;

impl LocalFilesystem for MetadataOnly {
    fn probe(&self, path: &Path) -> Result<EntryKind> {
        NativeFilesystem.probe(path)
    }
    fn children(&self, path: &Path, limit: usize) -> Result<DirectoryListing> {
        NativeFilesystem.children(path, limit)
    }
    fn read_chunk(&self, _: &Path, _: u64, _: usize) -> Result<ReadChunk> {
        panic!("passive scan read file content")
    }
}

fn scan(home: &Path) -> ScanResult {
    let env = ScanEnvironment::for_home(home.into(), HostPlatform::Linux);
    Scanner::new(MetadataOnly)
        .scan(&env, &AtomicBool::new(false), |_| {})
        .unwrap()
}

#[test]
fn kiro_configuration_is_a_provider_even_without_a_binary() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".kiro")).unwrap();
    let result = scan(home.path());
    let connection = result
        .snapshot
        .connections
        .iter()
        .find(|connection| connection.provider_id == "kiro")
        .expect("Kiro configuration was not detected");
    assert_eq!(connection.status, "configuration_only");
    assert_eq!(connection.display_name, "Kiro · Default profile");
}

#[test]
fn kiro_cli_is_found_through_a_path_independent_fallback() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join(".local/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("kiro-cli"), b"fixture").unwrap();
    let result = scan(home.path());
    let located = result
        .locations
        .values()
        .find(|located| located.provider_id == "kiro")
        .expect("Kiro CLI was not detected");
    assert_eq!(located.executable, Some(bin.join("kiro-cli")));
}

#[test]
fn kiro_session_files_are_known_but_not_opened() {
    let root = PathBuf::from("/home/person/.kiro");
    assert_eq!(
        session_stores("kiro", &root),
        vec![root.join("sessions/cli")]
    );
}
