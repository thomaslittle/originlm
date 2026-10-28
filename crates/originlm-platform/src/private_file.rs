//! Bounded reads of local grant descriptors; validates the opened file itself.
use std::{
    fs::OpenOptions,
    io::{self, Read},
    path::Path,
};

pub fn read_private_file(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected a bounded owner-only regular file",
        )
    };
    if limit == 0 || limit > 1024 * 1024 {
        return Err(invalid());
    }
    let before = path.symlink_metadata()?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(invalid());
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
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.file_type().is_symlink() || opened.len() > limit as u64 {
        return Err(invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened.mode() & 0o077 != 0
            || opened.dev() != before.dev()
            || opened.ino() != before.ino()
        {
            return Err(invalid());
        }
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(invalid());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_regular_descriptor_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("grant.json");
        std::fs::write(&path, b"{}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(read_private_file(&path, 16).is_err());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let link = directory.path().join("link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read_private_file(&link, 16).is_err());
        }
        assert_eq!(read_private_file(&path, 16).unwrap(), b"{}");
        assert!(read_private_file(&path, 1).is_err());
        assert!(read_private_file(directory.path(), 16).is_err());
    }
}
