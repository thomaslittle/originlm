//! Injected operating-system boundaries. Importing this crate performs no I/O.
mod executable;
mod native;
mod paths;
mod private_file;
pub use executable::{ExecutableIdentity, resolve_executable};
use originlm_contracts::Result;
pub use paths::{child_path, default_state_dir};
pub use private_file::read_private_file;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    Missing,
    File,
    Directory,
    Symlink,
    Other,
}
#[derive(Debug, Clone)]
pub struct DirectoryListing {
    pub entries: Vec<PathBuf>,
    pub truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub volume: u128,
    pub file: u128,
    /// Birth time distinguishes reused file IDs when the filesystem supplies it.
    pub created: Option<std::time::SystemTime>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadChunk {
    pub bytes: Vec<u8>,
    pub next_offset: u64,
    pub total_size: u64,
    pub truncated: bool,
    pub identity: Option<FileIdentity>,
}
pub trait LocalFilesystem: Send + Sync {
    fn probe(&self, path: &Path) -> Result<EntryKind>;
    fn children(&self, path: &Path, max_entries: usize) -> Result<DirectoryListing>;
    fn read_chunk(&self, path: &Path, offset: u64, max_bytes: usize) -> Result<ReadChunk>;
    /// Separate opt-in boundary; injected filesystems do not follow links by default.
    fn resolve_executable(&self, _: &Path) -> Result<ExecutableIdentity> {
        Err(originlm_contracts::DomainError::new(
            originlm_contracts::ErrorCode::CapabilityUnsupported,
            "Executable link resolution unavailable",
        ))
    }
}
pub struct NativeFilesystem;

pub mod watcher;
