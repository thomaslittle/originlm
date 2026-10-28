use originlm_platform::*;
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};
/// Scratch directory for a test. macOS keeps its temp directory behind the
/// `/var` -> `/private/var` symlink, which the filesystem layer correctly
/// refuses to follow, so resolve it there. Other platforms keep the plain path
/// (Windows canonicalization adds a `\?\` prefix).
fn tempdir() -> tempfile::TempDir {
    let base = if cfg!(target_os = "macos") {
        std::fs::canonicalize(std::env::temp_dir()).unwrap()
    } else {
        std::env::temp_dir()
    };
    tempfile::Builder::new().tempdir_in(base).unwrap()
}

#[test]
fn file_identity_uses_birth_time_when_available_and_survives_append() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, b"one\n").unwrap();
    let first = NativeFilesystem
        .read_chunk(&path, 0, 4)
        .unwrap()
        .identity
        .unwrap();
    assert_eq!(first.created, fs::metadata(&path).unwrap().created().ok());
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"two\n")
        .unwrap();
    let second = NativeFilesystem
        .read_chunk(&path, 4, 4)
        .unwrap()
        .identity
        .unwrap();
    assert_eq!(first, second);
}
#[test]
fn detects_missing_file_directory_without_reading_content() {
    let tmp = tempdir();
    let fsys = NativeFilesystem;
    assert_eq!(fsys.probe(tmp.path()).unwrap(), EntryKind::Directory);
    assert_eq!(
        fsys.probe(&tmp.path().join("missing")).unwrap(),
        EntryKind::Missing
    );
    fs::write(tmp.path().join("file"), b"synthetic content").unwrap();
    assert_eq!(
        fsys.probe(&tmp.path().join("file")).unwrap(),
        EntryKind::File
    );
}
#[test]
fn reads_only_requested_byte_window_from_sparse_gigabyte_file() {
    let tmp = tempdir();
    let path = tmp.path().join("large.jsonl");
    let mut f = fs::File::create(&path).unwrap();
    f.set_len(1_073_741_824).unwrap();
    f.seek(SeekFrom::Start(1_073_741_820)).unwrap();
    f.write_all(b"tail").unwrap();
    drop(f);
    let chunk = NativeFilesystem
        .read_chunk(&path, 1_073_741_820, 8)
        .unwrap();
    assert_eq!(chunk.bytes, b"tail");
    assert_eq!(chunk.next_offset, 1_073_741_824);
    assert!(!chunk.truncated);
    let first = NativeFilesystem.read_chunk(&path, 0, 16).unwrap();
    assert_eq!(first.bytes.len(), 16);
    assert!(first.truncated);
}
#[test]
fn invalid_byte_budgets_and_offsets_fail_instead_of_unbounded_read() {
    let tmp = tempdir();
    let path = tmp.path().join("file");
    fs::write(&path, b"abc").unwrap();
    assert!(NativeFilesystem.read_chunk(&path, 0, 0).is_err());
    assert!(
        NativeFilesystem
            .read_chunk(&path, 0, 17 * 1024 * 1024)
            .is_err()
    );
    assert!(NativeFilesystem.read_chunk(&path, 4, 5).is_err());
    assert!(NativeFilesystem.read_chunk(tmp.path(), 0, 5).is_err());
}
#[test]
fn bounded_directory_listing_is_explicitly_partial() {
    let tmp = tempdir();
    for i in 0..10 {
        fs::write(tmp.path().join(i.to_string()), b"").unwrap();
    }
    let listing = NativeFilesystem.children(tmp.path(), 3).unwrap();
    assert_eq!(listing.entries.len(), 3);
    assert!(listing.truncated);
    assert!(NativeFilesystem.children(tmp.path(), 0).is_err());
}
#[test]
fn unicode_paths_are_not_lossily_reconstructed() {
    let tmp = tempdir();
    let path = child_path(tmp.path(), "work space-世界").unwrap();
    fs::write(&path, b"ok").unwrap();
    assert_eq!(
        NativeFilesystem.read_chunk(&path, 0, 8).unwrap().bytes,
        b"ok"
    );
}
#[test]
fn traversal_and_absolute_child_names_are_rejected() {
    let tmp = tempdir();
    for name in [
        "..",
        "../escape",
        "/root",
        "a/b",
        r"a\b",
        r"C:\secret",
        ".",
        "",
    ] {
        assert!(child_path(tmp.path(), name).is_err(), "{name}");
    }
}
#[test]
fn diagnostic_errors_never_embed_private_path() {
    let tmp = tempdir();
    let path = tmp.path().join("private-account-name");
    let e = NativeFilesystem.read_chunk(&path, 0, 4).unwrap_err();
    assert!(!format!("{e:?}").contains("private-account-name"));
}
#[cfg(unix)]
#[test]
fn symlinks_are_identified_and_not_read_by_default() {
    let tmp = tempdir();
    let path = tmp.path().join("file");
    fs::write(&path, b"secret").unwrap();
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert_eq!(NativeFilesystem.probe(&link).unwrap(), EntryKind::Symlink);
    assert!(NativeFilesystem.read_chunk(&link, 0, 8).is_err());
}
