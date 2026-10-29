//! Executable-only Unix link resolution. Data/credential reads never use this path.
#[cfg(unix)]
use crate::FileIdentity;
use crate::{LocalFilesystem, NativeFilesystem};
use originlm_contracts::{DomainError, ErrorCode, Result};
use std::path::{Path, PathBuf};

/// Private native location evidence, never a public account/provider attestation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableIdentity {
    pub command: PathBuf,
    pub resolved: PathBuf,
    links: Vec<(PathBuf, PathBuf)>,
    #[cfg(unix)]
    stamp: Stamp,
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    identity: FileIdentity,
    length: u64,
    modified: Option<std::time::SystemTime>,
    mode: u32,
    changed: (i64, i64),
}

fn unavailable() -> DomainError {
    DomainError::new(
        ErrorCode::RuntimeMissing,
        "Executable link is unavailable or changed",
    )
}

impl ExecutableIdentity {
    /// Recheck the complete link binding and target metadata immediately before use.
    pub fn validate(&self) -> Result<()> {
        if NativeFilesystem.resolve_executable(&self.command)? == *self {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
}

#[cfg(not(unix))]
pub fn resolve_executable(_: &Path) -> Result<ExecutableIdentity> {
    Err(DomainError::new(
        ErrorCode::CapabilityUnsupported,
        "Executable link resolution is Unix-only",
    ))
}

#[cfg(unix)]
pub fn resolve_executable(command: &Path) -> Result<ExecutableIdentity> {
    use std::{
        collections::{BTreeSet, VecDeque},
        ffi::OsString,
        fs,
        os::unix::fs::MetadataExt,
        path::Component,
    };
    const MAX_LINKS: usize = 32;
    const MAX_STEPS: usize = 512;
    const MAX_PATH_BYTES: usize = 32_768;
    if !command.is_absolute()
        || command.as_os_str().len() > MAX_PATH_BYTES
        || command.as_os_str().as_encoded_bytes().starts_with(b"//")
        || command
            .components()
            .any(|p| matches!(p, Component::ParentDir))
    {
        return Err(unavailable());
    }
    let mut pending: VecDeque<OsString> = command
        .components()
        .map(|p| p.as_os_str().to_owned())
        .collect();
    let mut current = PathBuf::new();
    let mut links = Vec::new();
    // A symlink may legitimately be revisited after a file symlink is expanded
    // (for example macOS `/var` -> `/private/var`). A true loop repeats the
    // same link with the same remaining path, so key the guard on both.
    let mut seen = BTreeSet::new();
    let mut steps = 0;
    while let Some(part) = pending.pop_front() {
        steps += 1;
        if steps > MAX_STEPS || pending.len() > MAX_STEPS {
            return Err(unavailable());
        }
        if part == "/" {
            current = PathBuf::from("/");
            continue;
        }
        if part == "." {
            continue;
        }
        if part == ".." {
            current.pop();
            continue;
        }
        current.push(part);
        if current.as_os_str().len() > MAX_PATH_BYTES {
            return Err(unavailable());
        }
        let metadata = fs::symlink_metadata(&current).map_err(|_| unavailable())?;
        if metadata.file_type().is_symlink() {
            let state = (
                current.clone(),
                pending.iter().cloned().collect::<Vec<OsString>>(),
            );
            if links.len() == MAX_LINKS || !seen.insert(state) {
                return Err(unavailable());
            }
            let target = fs::read_link(&current).map_err(|_| unavailable())?;
            if target.as_os_str().len() > MAX_PATH_BYTES
                || target.as_os_str().as_encoded_bytes().starts_with(b"//")
            {
                return Err(unavailable());
            }
            links.push((current.clone(), target.clone()));
            current.pop();
            for component in target.components().rev() {
                pending.push_front(component.as_os_str().to_owned());
            }
        } else if pending.is_empty() {
            if !metadata.is_file() || metadata.mode() & 0o111 == 0 {
                return Err(unavailable());
            }
            return Ok(ExecutableIdentity {
                command: command.to_path_buf(),
                resolved: current,
                links,
                stamp: Stamp {
                    identity: FileIdentity {
                        volume: u128::from(metadata.dev()),
                        file: u128::from(metadata.ino()),
                        created: metadata.created().ok(),
                    },
                    length: metadata.len(),
                    modified: metadata.modified().ok(),
                    mode: metadata.mode(),
                    changed: (metadata.ctime(), metadata.ctime_nsec()),
                },
            });
        } else if !metadata.is_dir() {
            return Err(unavailable());
        }
    }
    Err(unavailable())
}
