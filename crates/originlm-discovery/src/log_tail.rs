use originlm_contracts::{DomainError, ErrorCode, Result};
use originlm_platform::{FileIdentity, LocalFilesystem, ReadChunk};
use std::path::Path;
const MAX_READ: usize = 1024 * 1024;
const MAX_PENDING: usize = 1024 * 1024;
const ANCHOR: usize = 64;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogTailState {
    pub offset: u64,
    identity: Option<FileIdentity>,
    prefix: Vec<u8>,
    anchor: Vec<u8>,
    pending: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTailBatch {
    pub lines: Vec<Vec<u8>>,
    pub truncated: bool,
    pub rotated: bool,
    pub has_more: bool,
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Invalid incremental log-tail request",
    )
}
fn ensure_identity(expected: &Option<FileIdentity>, actual: &Option<FileIdentity>) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(DomainError::new(
            ErrorCode::ProviderUnavailable,
            "Metadata file changed during incremental read",
        ))
    }
}
impl LogTailState {
    pub fn reset(&mut self) {
        self.offset = 0;
        self.identity = None;
        self.prefix.clear();
        self.anchor.clear();
        self.pending.clear();
    }
    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }
}
pub fn read_lines<F: LocalFilesystem>(
    fs: &F,
    path: &Path,
    state: &mut LogTailState,
    max_bytes: usize,
    max_lines: usize,
) -> Result<LogTailBatch> {
    read_lines_with_cancel(fs, path, state, max_bytes, max_lines, || false)
}
fn read_checked<F: LocalFilesystem, C: FnMut() -> bool>(
    fs: &F,
    path: &Path,
    offset: u64,
    max_bytes: usize,
    cancelled: &mut C,
) -> Result<ReadChunk> {
    if cancelled() {
        return Err(DomainError::new(
            ErrorCode::Cancelled,
            "Log-tail read was cancelled",
        ));
    }
    fs.read_chunk(path, offset, max_bytes)
}
pub fn read_lines_with_cancel<F, C>(
    fs: &F,
    path: &Path,
    state: &mut LogTailState,
    max_bytes: usize,
    max_lines: usize,
    mut cancelled: C,
) -> Result<LogTailBatch>
where
    F: LocalFilesystem,
    C: FnMut() -> bool,
{
    if max_bytes == 0 || max_bytes > MAX_READ || max_lines == 0 || max_lines > 4096 {
        return Err(invalid());
    }
    let mut next_state = state.clone();
    let head = read_checked(fs, path, 0, ANCHOR, &mut cancelled)?;
    let total = head.total_size;
    let mut truncated = false;
    let mut rotated = false;
    if next_state.offset > total {
        next_state.reset();
        truncated = true
    } else if next_state.offset > 0 {
        if next_state.identity != head.identity || !head.bytes.starts_with(&next_state.prefix) {
            next_state.reset();
            rotated = true;
        } else if !next_state.anchor.is_empty() {
            let start = next_state
                .offset
                .checked_sub(next_state.anchor.len() as u64)
                .ok_or_else(invalid)?;
            let check = read_checked(fs, path, start, next_state.anchor.len(), &mut cancelled)?;
            ensure_identity(&head.identity, &check.identity)?;
            if check.bytes != next_state.anchor {
                next_state.reset();
                rotated = true
            }
        }
    }
    let current = read_checked(fs, path, 0, 1, &mut cancelled)?;
    ensure_identity(&head.identity, &current.identity)?;
    let current_total = current.total_size;
    if next_state.offset > current_total {
        next_state.reset();
        truncated = true
    }
    let buffered_lines = next_state
        .pending
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    if buffered_lines < max_lines && next_state.offset < current_total {
        let chunk = read_checked(fs, path, next_state.offset, max_bytes, &mut cancelled)?;
        ensure_identity(&head.identity, &chunk.identity)?;
        if next_state.pending.len().saturating_add(chunk.bytes.len()) > MAX_PENDING {
            return Err(DomainError::new(
                ErrorCode::FrameTooLarge,
                "Incremental log record exceeds the bounded buffer",
            ));
        }
        next_state.offset = chunk.next_offset;
        next_state.pending.extend_from_slice(&chunk.bytes);
    }
    if next_state.offset > 0 {
        let len = ANCHOR.min(next_state.offset as usize);
        let start = next_state.offset - len as u64;
        let anchor = read_checked(fs, path, start, len, &mut cancelled)?;
        ensure_identity(&head.identity, &anchor.identity)?;
        next_state.anchor = anchor.bytes;
    } else {
        next_state.anchor.clear()
    }
    next_state.identity = head.identity.clone();
    // Remember only bytes already consumed; append to a short file is not rotation.
    next_state.prefix = head.bytes[..head.bytes.len().min(next_state.offset as usize)].to_vec();
    let mut lines = Vec::new();
    let mut consumed = 0usize;
    for (index, byte) in next_state.pending.iter().enumerate() {
        if *byte == b'\n' {
            let mut end = index;
            if end > consumed && next_state.pending[end - 1] == b'\r' {
                end -= 1
            }
            lines.push(next_state.pending[consumed..end].to_vec());
            consumed = index + 1;
            if lines.len() == max_lines {
                break;
            }
        }
    }
    if consumed > 0 {
        next_state.pending.drain(..consumed);
    }
    let latest_chunk = read_checked(fs, path, 0, 1, &mut cancelled)?;
    ensure_identity(&head.identity, &latest_chunk.identity)?;
    let latest = latest_chunk.total_size;
    let has_complete = next_state.pending.contains(&b'\n');
    let batch = LogTailBatch {
        lines,
        truncated,
        rotated,
        has_more: next_state.offset < latest || has_complete,
    };
    *state = next_state;
    Ok(batch)
}
