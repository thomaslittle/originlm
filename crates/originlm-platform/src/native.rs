use crate::{
    DirectoryListing, EntryKind, FileIdentity, LocalFilesystem, NativeFilesystem, ReadChunk,
};
use originlm_contracts::{DomainError, ErrorCode, Result};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};

fn io_error(error: std::io::Error) -> DomainError {
    let code = match error.kind() {
        std::io::ErrorKind::NotFound => ErrorCode::NotFound,
        std::io::ErrorKind::PermissionDenied => ErrorCode::LocalAccessDenied,
        _ => ErrorCode::ProviderUnavailable,
    };
    DomainError::new(code, "Local filesystem operation could not be completed")
}
fn invalid(message: &str) -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, message)
}
fn reject_symlink_components(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid("Path must not contain dot segments"));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(io_error)?.join(path)
    };
    for component in absolute.ancestors() {
        let metadata = fs::symlink_metadata(component).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("Symlink paths are not readable"));
        }
    }
    Ok(())
}
impl LocalFilesystem for NativeFilesystem {
    fn resolve_executable(&self, path: &Path) -> Result<crate::ExecutableIdentity> {
        crate::resolve_executable(path)
    }
    fn probe(&self, path: &Path) -> Result<EntryKind> {
        match fs::symlink_metadata(path) {
            Ok(m) => Ok(if m.file_type().is_symlink() {
                EntryKind::Symlink
            } else if m.is_file() {
                EntryKind::File
            } else if m.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::Other
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EntryKind::Missing),
            Err(e) => Err(io_error(e)),
        }
    }
    fn children(&self, path: &Path, max_entries: usize) -> Result<DirectoryListing> {
        if max_entries == 0 || max_entries > 4096 {
            return Err(invalid("Directory budget must be 1..4096"));
        }
        if self.probe(path)? != EntryKind::Directory {
            return Err(invalid("Expected a regular directory"));
        }
        let mut entries = Vec::new();
        let mut truncated = false;
        for entry in fs::read_dir(path).map_err(io_error)? {
            if entries.len() == max_entries {
                truncated = true;
                break;
            }
            entries.push(entry.map_err(io_error)?.path());
        }
        entries.sort();
        Ok(DirectoryListing { entries, truncated })
    }
    fn read_chunk(&self, path: &Path, offset: u64, max_bytes: usize) -> Result<ReadChunk> {
        if max_bytes == 0 || max_bytes > 16 * 1024 * 1024 {
            return Err(invalid("Read budget must be 1..16777216 bytes"));
        }
        reject_symlink_components(path)?;
        if self.probe(path)? != EntryKind::File {
            return Err(invalid("Only regular files may be read"));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
        }
        let mut file = options.open(path).map_err(io_error)?;
        let metadata = file.metadata().map_err(io_error)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(invalid("Only regular files may be read"));
        }
        let total_size = metadata.len();
        #[cfg(unix)]
        let native_id = {
            use std::os::unix::fs::MetadataExt;
            // Use the opened file, not a second path lookup that may name a replacement.
            Some(file_id::FileId::new_inode(metadata.dev(), metadata.ino()))
        };
        #[cfg(not(unix))]
        let native_id = file_id::get_file_id(path).ok();
        let created = metadata.created().ok();
        let identity = native_id.map(|id| match id {
            file_id::FileId::Inode {
                device_id,
                inode_number,
            } => FileIdentity {
                volume: u128::from(device_id),
                file: u128::from(inode_number),
                created,
            },
            file_id::FileId::LowRes {
                volume_serial_number,
                file_index,
            } => FileIdentity {
                volume: u128::from(volume_serial_number),
                file: u128::from(file_index),
                created,
            },
            file_id::FileId::HighRes {
                volume_serial_number,
                file_id,
            } => FileIdentity {
                volume: u128::from(volume_serial_number),
                file: file_id,
                created,
            },
        });
        if offset > total_size {
            return Err(invalid("Read offset exceeds current file size"));
        }
        file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
        let mut bytes = Vec::with_capacity(
            max_bytes.min((total_size - offset).min(usize::MAX as u64) as usize),
        );
        file.take(max_bytes as u64)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        let next_offset = offset + bytes.len() as u64;
        Ok(ReadChunk {
            bytes,
            next_offset,
            total_size,
            truncated: next_offset < total_size,
            identity,
        })
    }
}
