use originlm_contracts::Result;
use originlm_discovery::log_tail::{LogTailState, read_lines};
use originlm_platform::{
    DirectoryListing, EntryKind, LocalFilesystem, NativeFilesystem, ReadChunk,
};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Scratch directory for a test. macOS keeps its temp directory behind the
/// `/var` -> `/private/var` symlink, which the tailer correctly refuses to
/// follow, so resolve it there. Other platforms keep the plain path (Windows
/// canonicalization adds a `\?\` prefix).
fn tempdir() -> tempfile::TempDir {
    let base = if cfg!(target_os = "macos") {
        fs::canonicalize(std::env::temp_dir()).unwrap()
    } else {
        std::env::temp_dir()
    };
    tempfile::Builder::new().tempdir_in(base).unwrap()
}

#[test]
fn append_and_partial_records_are_incremental() {
    let tmp = tempdir();
    let path = tmp.path().join("events.jsonl");
    fs::write(&path, b"one\npar").unwrap();
    let mut state = LogTailState::default();
    let first = read_lines(&NativeFilesystem, &path, &mut state, 1024, 20).unwrap();
    assert_eq!(first.lines, vec![b"one".to_vec()]);
    assert_eq!(state.pending_bytes(), 3);
    assert!(!first.rotated && !first.truncated);
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"tial\ntwo\r\n")
        .unwrap();
    let second = read_lines(&NativeFilesystem, &path, &mut state, 1024, 20).unwrap();
    assert_eq!(second.lines, vec![b"partial".to_vec(), b"two".to_vec()]);
    assert_eq!(state.pending_bytes(), 0);
}
#[test]
fn truncation_restarts_without_replaying_old_records() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, b"a\nb\n").unwrap();
    let mut state = LogTailState::default();
    assert_eq!(
        read_lines(&NativeFilesystem, &path, &mut state, 1024, 20)
            .unwrap()
            .lines
            .len(),
        2
    );
    fs::write(&path, b"z\n").unwrap();
    let next = read_lines(&NativeFilesystem, &path, &mut state, 1024, 20).unwrap();
    assert!(next.truncated);
    assert!(!next.rotated);
    assert_eq!(next.lines, vec![b"z".to_vec()]);
}
#[test]
fn replacement_rotation_is_detected_even_when_new_file_is_larger() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, b"old\n").unwrap();
    let mut state = LogTailState::default();
    read_lines(&NativeFilesystem, &path, &mut state, 1024, 20).unwrap();
    fs::rename(&path, tmp.path().join("events.1")).unwrap();
    fs::write(&path, b"new-long\n").unwrap();
    let next = read_lines(&NativeFilesystem, &path, &mut state, 1024, 20).unwrap();
    assert!(next.rotated);
    assert!(!next.truncated);
    assert_eq!(next.lines, vec![b"new-long".to_vec()]);
}
#[test]
fn replacement_with_same_anchor_bytes_is_detected_by_file_identity() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    let suffix = b"k\n".repeat(32);
    let original = [b"a\n".repeat(32), suffix.clone()].concat();
    fs::write(&path, original).unwrap();
    let mut state = LogTailState::default();
    read_lines(&NativeFilesystem, &path, &mut state, 1024, 100).unwrap();
    fs::remove_file(&path).unwrap();
    let replacement = [b"b\n".repeat(32), suffix, b"new\n".to_vec()].concat();
    fs::write(&path, replacement).unwrap();
    let batch = read_lines(&NativeFilesystem, &path, &mut state, 1024, 100).unwrap();
    assert!(batch.rotated);
    assert_eq!(batch.lines.first().unwrap(), b"b");
    assert_eq!(batch.lines.last().unwrap(), b"new");
}
#[test]
fn line_limit_buffers_complete_records_without_duplicate_file_reads() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, b"a\nb\nc\n").unwrap();
    let mut state = LogTailState::default();
    let a = read_lines(&NativeFilesystem, &path, &mut state, 1024, 1).unwrap();
    assert_eq!(a.lines, vec![b"a".to_vec()]);
    assert!(a.has_more);
    let b = read_lines(&NativeFilesystem, &path, &mut state, 1024, 1).unwrap();
    assert_eq!(b.lines, vec![b"b".to_vec()]);
    assert!(b.has_more);
    let c = read_lines(&NativeFilesystem, &path, &mut state, 1024, 1).unwrap();
    assert_eq!(c.lines, vec![b"c".to_vec()]);
    assert!(!c.has_more);
}
struct CountingFs {
    bytes: AtomicUsize,
}
struct ReusedIdentityFs;
impl LocalFilesystem for ReusedIdentityFs {
    fn probe(&self, path: &Path) -> Result<EntryKind> {
        NativeFilesystem.probe(path)
    }
    fn children(&self, path: &Path, limit: usize) -> Result<DirectoryListing> {
        NativeFilesystem.children(path, limit)
    }
    fn read_chunk(&self, path: &Path, offset: u64, limit: usize) -> Result<ReadChunk> {
        let mut chunk = NativeFilesystem.read_chunk(path, offset, limit)?;
        // Some filesystems reuse an inode immediately and do not expose birth time.
        chunk.identity = None;
        Ok(chunk)
    }
}
#[test]
fn changed_prefix_detects_rotation_when_identity_and_tail_anchor_are_reused() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    let suffix = b"same\n".repeat(32);
    fs::write(&path, [b"old\n".repeat(32), suffix.clone()].concat()).unwrap();
    let mut state = LogTailState::default();
    read_lines(&ReusedIdentityFs, &path, &mut state, 1024, 100).unwrap();
    fs::write(
        &path,
        [b"new\n".repeat(32), suffix, b"last\n".to_vec()].concat(),
    )
    .unwrap();
    let batch = read_lines(&ReusedIdentityFs, &path, &mut state, 1024, 100).unwrap();
    assert!(batch.rotated);
    assert_eq!(batch.lines.first().unwrap(), b"new");
    assert_eq!(batch.lines.last().unwrap(), b"last");
    let empty = read_lines(&ReusedIdentityFs, &path, &mut state, 1024, 100).unwrap();
    assert!(empty.lines.is_empty());
    assert!(!empty.rotated);
}
impl LocalFilesystem for CountingFs {
    fn probe(&self, p: &Path) -> Result<EntryKind> {
        NativeFilesystem.probe(p)
    }
    fn children(&self, p: &Path, n: usize) -> Result<DirectoryListing> {
        NativeFilesystem.children(p, n)
    }
    fn read_chunk(&self, p: &Path, o: u64, n: usize) -> Result<ReadChunk> {
        let c = NativeFilesystem.read_chunk(p, o, n)?;
        self.bytes.fetch_add(c.bytes.len(), Ordering::SeqCst);
        Ok(c)
    }
}
struct SyntheticLargeFs {
    bytes: AtomicUsize,
}
impl LocalFilesystem for SyntheticLargeFs {
    fn probe(&self, _: &Path) -> Result<EntryKind> {
        Ok(EntryKind::File)
    }
    fn children(&self, _: &Path, _: usize) -> Result<DirectoryListing> {
        Ok(DirectoryListing {
            entries: Vec::new(),
            truncated: false,
        })
    }
    fn read_chunk(&self, _: &Path, offset: u64, max_bytes: usize) -> Result<ReadChunk> {
        let total_size = 1_073_741_824u64;
        let size = total_size.saturating_sub(offset).min(max_bytes as u64) as usize;
        self.bytes.fetch_add(size, Ordering::SeqCst);
        Ok(ReadChunk {
            bytes: vec![0; size],
            next_offset: offset + size as u64,
            total_size,
            truncated: offset + (size as u64) < total_size,
            identity: Some(originlm_platform::FileIdentity {
                volume: 7,
                file: 11,
                created: None,
            }),
        })
    }
}
#[test]
fn synthetic_gigabyte_log_reads_only_the_bounded_window() {
    let fs = SyntheticLargeFs {
        bytes: AtomicUsize::new(0),
    };
    let mut state = LogTailState::default();
    let batch = read_lines(&fs, Path::new("synthetic-large-log"), &mut state, 4096, 32).unwrap();
    assert!(batch.lines.is_empty());
    assert!(batch.has_more);
    assert!(fs.bytes.load(Ordering::SeqCst) < 5000);
}
#[test]
fn cancellation_is_checked_before_any_file_read() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, b"one\n").unwrap();
    let fs = CountingFs {
        bytes: AtomicUsize::new(0),
    };
    let mut state = LogTailState::default();
    let error = originlm_discovery::log_tail::read_lines_with_cancel(
        &fs,
        &path,
        &mut state,
        1024,
        20,
        || true,
    )
    .unwrap_err();
    assert_eq!(error.code, originlm_contracts::ErrorCode::Cancelled);
    assert_eq!(fs.bytes.load(Ordering::SeqCst), 0);
    assert_eq!(state.offset, 0);
}
#[test]
fn cancellation_during_a_poll_preserves_the_callers_saved_offset_and_buffer() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, b"one\npartial").unwrap();
    let fs = CountingFs {
        bytes: AtomicUsize::new(0),
    };
    let mut state = LogTailState::default();
    let mut checks = 0;
    let error = originlm_discovery::log_tail::read_lines_with_cancel(
        &fs,
        &path,
        &mut state,
        1024,
        20,
        || {
            checks += 1;
            checks == 3
        },
    )
    .unwrap_err();
    assert_eq!(error.code, originlm_contracts::ErrorCode::Cancelled);
    assert_eq!(state.offset, 0);
    assert_eq!(state.pending_bytes(), 0);
}
#[cfg(unix)]
#[test]
fn tailer_rejects_symlink_paths() {
    let tmp = tempdir();
    let target = tmp.path().join("target");
    let link = tmp.path().join("events");
    fs::write(&target, b"private\n").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let mut state = LogTailState::default();
    assert!(read_lines(&NativeFilesystem, &link, &mut state, 1024, 20).is_err());
    assert_eq!(state.offset, 0);
}
#[cfg(unix)]
#[test]
fn tailer_rejects_paths_that_traverse_a_symlinked_directory() {
    let tmp = tempdir();
    let target_dir = tmp.path().join("real");
    let linked_dir = tmp.path().join("linked");
    fs::create_dir(&target_dir).unwrap();
    fs::write(target_dir.join("events"), b"private\n").unwrap();
    std::os::unix::fs::symlink(&target_dir, &linked_dir).unwrap();
    let mut state = LogTailState::default();
    assert!(
        read_lines(
            &NativeFilesystem,
            &linked_dir.join("events"),
            &mut state,
            1024,
            20
        )
        .is_err()
    );
    assert_eq!(state.offset, 0);
}
#[test]
fn invalid_budgets_and_oversized_single_records_fail_closed() {
    let tmp = tempdir();
    let path = tmp.path().join("events");
    fs::write(&path, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let mut state = LogTailState::default();
    assert!(read_lines(&NativeFilesystem, &path, &mut state, 0, 1).is_err());
    assert!(read_lines(&NativeFilesystem, &path, &mut state, 1024, 0).is_err());
    let first = read_lines(&NativeFilesystem, &path, &mut state, 1024 * 1024, 1).unwrap();
    assert!(first.lines.is_empty());
    assert!(read_lines(&NativeFilesystem, &path, &mut state, 1, 1).is_err());
}
