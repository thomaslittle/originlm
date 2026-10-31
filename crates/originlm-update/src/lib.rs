//! Authenticated, staged executable updates. Checksums detect corruption;
//! Ed25519 verifies the publisher. This crate never downloads, publishes, or
//! creates signing credentials.
use originlm_contracts::{DomainError, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const UPDATE_SCHEMA: u32 = 1;
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateManifest {
    pub schema_version: u32,
    pub version: String,
    pub channel: String,
    pub platform: String,
    pub protocol_version: String,
    pub archive: String,
    pub archive_bytes: u64,
    pub archive_sha256: String,
    pub published_at: String,
    #[serde(default)]
    pub notes_url: Option<String>,
    /// Base64 Ed25519 signature over canonical JSON without this field.
    pub signature: String,
}

fn fail(code: ErrorCode, message: &'static str) -> DomainError {
    DomainError::new(code, message)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn sha256_file(path: &Path) -> Result<String, DomainError> {
    let mut file = File::open(path).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update archive is unreadable",
        )
    })?;
    let mut digest = Sha256::new();
    let mut block = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut block).map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Update archive is unreadable",
            )
        })?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > MAX_ARCHIVE_BYTES {
            return Err(fail(
                ErrorCode::FrameTooLarge,
                "Update payload exceeds the bound",
            ));
        }
        digest.update(&block[..count]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

pub fn canonical_manifest(manifest: &UpdateManifest) -> Result<Vec<u8>, DomainError> {
    let value = serde_json::to_value(manifest)
        .map_err(|_| fail(ErrorCode::InvalidRequest, "Update manifest is invalid"))?;
    let Value::Object(mut map) = value else {
        return Err(fail(
            ErrorCode::InvalidRequest,
            "Update manifest is invalid",
        ));
    };
    map.remove("signature");
    // BTreeMap serializes keys in sorted order, matching the signer's contract.
    let ordered: BTreeMap<String, Value> = map.into_iter().collect();
    serde_json::to_vec(&ordered)
        .map_err(|_| fail(ErrorCode::InvalidRequest, "Update manifest is invalid"))
}

pub fn platform_id() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = std::env::consts::ARCH;
    format!("{os}-{arch}")
}

fn parse_version(value: &str) -> Option<semver::Version> {
    if value.len() > 128 {
        return None;
    }
    semver::Version::parse(value).ok()
}

pub fn verify_manifest_signature(
    manifest: &UpdateManifest,
    public_key: &[u8],
) -> Result<(), DomainError> {
    use base64::Engine;
    if manifest.schema_version != UPDATE_SCHEMA {
        return Err(fail(
            ErrorCode::ProtocolIncompatible,
            "Update manifest schema is not supported",
        ));
    }
    if public_key.len() != 32 {
        return Err(fail(
            ErrorCode::LocalAccessDenied,
            "Update publisher key is invalid",
        ));
    }
    let signature = base64::engine::general_purpose::STANDARD
        .decode(&manifest.signature)
        .map_err(|_| fail(ErrorCode::LocalAccessDenied, "Update signature is invalid"))?;
    let message = canonical_manifest(manifest)?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(&message, &signature)
        .map_err(|_| {
            fail(
                ErrorCode::LocalAccessDenied,
                "Update publisher signature did not verify",
            )
        })
}

/// Verify a complete update before any executable is staged.
pub fn verify_update(
    manifest: &UpdateManifest,
    archive: &Path,
    public_key: &[u8],
    current_version: &str,
    protocol_version: &str,
    allow_downgrade: bool,
) -> Result<(), DomainError> {
    verify_manifest_signature(manifest, public_key)?;
    if manifest.platform != platform_id() {
        return Err(fail(
            ErrorCode::ProtocolIncompatible,
            "Update was built for a different platform",
        ));
    }
    if !manifest.protocol_version.starts_with("1.") || !protocol_version.starts_with("1.") {
        return Err(fail(
            ErrorCode::ProtocolIncompatible,
            "Update protocol is incompatible with this runtime",
        ));
    }
    let next = parse_version(&manifest.version)
        .ok_or_else(|| fail(ErrorCode::InvalidRequest, "Update version is invalid"))?;
    let alpha = next.pre.as_str().strip_prefix("alpha.");
    if (next.major, next.minor, next.patch) != (0, 1, 0)
        || !next.build.is_empty()
        || !alpha.is_some_and(|value| {
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(fail(
            ErrorCode::ProtocolIncompatible,
            "Update version is outside the supported release family",
        ));
    }
    let current = parse_version(current_version)
        .ok_or_else(|| fail(ErrorCode::InvalidRequest, "Current version is invalid"))?;
    if !allow_downgrade && next.cmp_precedence(&current).is_lt() {
        return Err(fail(
            ErrorCode::Conflict,
            "Update would downgrade the shared runtime",
        ));
    }
    let metadata = std::fs::metadata(archive).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update archive is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.len() != manifest.archive_bytes {
        return Err(fail(
            ErrorCode::InvalidRequest,
            "Update archive size does not match",
        ));
    }
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(fail(
            ErrorCode::FrameTooLarge,
            "Update archive exceeds the bound",
        ));
    }
    if sha256_file(archive)? != manifest.archive_sha256.to_ascii_lowercase() {
        return Err(fail(
            ErrorCode::InvalidRequest,
            "Update archive checksum does not match",
        ));
    }
    Ok(())
}

fn copy_verified_payload(
    manifest: &UpdateManifest,
    source_path: &Path,
    target_path: &Path,
) -> Result<(), DomainError> {
    let mut created = false;
    let copy = (|| -> Result<(), DomainError> {
        let mut source = File::open(source_path)
            .map_err(|_| {
                fail(
                    ErrorCode::StorageUnavailable,
                    "Staged executable is unreadable",
                )
            })?
            .take(manifest.archive_bytes + 1);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o755);
        }
        let mut output = options.open(target_path).map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Replacement could not be created",
            )
        })?;
        created = true;
        let bytes = std::io::copy(&mut source, &mut output)
            .map_err(|_| fail(ErrorCode::StorageUnavailable, "Replacement copy failed"))?;
        output.sync_all().map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Replacement could not be flushed",
            )
        })?;
        drop(output);
        if bytes != manifest.archive_bytes
            || sha256_file(target_path)? != manifest.archive_sha256.to_ascii_lowercase()
        {
            return Err(fail(
                ErrorCode::LocalAccessDenied,
                "Replacement does not match the signed executable",
            ));
        }
        Ok(())
    })();
    if let Err(error) = copy {
        // Only remove a file this activation created (create_new rejects collisions).
        if created {
            let _ = std::fs::remove_file(target_path);
        }
        return Err(error);
    }
    Ok(())
}

pub struct StagedUpdate {
    pub directory: PathBuf,
    pub archive: PathBuf,
    pub manifest: PathBuf,
}

/// Copy a verified archive into a staging directory. No installed executable is
/// touched and no activation occurs.
pub fn stage_update(
    manifest: &UpdateManifest,
    archive: &Path,
    staging_root: &Path,
    public_key: &[u8],
    current_version: &str,
    protocol_version: &str,
) -> Result<StagedUpdate, DomainError> {
    verify_update(
        manifest,
        archive,
        public_key,
        current_version,
        protocol_version,
        false,
    )?;
    let name = Path::new(&manifest.archive)
        .file_name()
        .filter(|name| *name != "manifest.json" && *name != "READY")
        .ok_or_else(|| fail(ErrorCode::InvalidRequest, "Update payload name is invalid"))?;
    std::fs::create_dir_all(staging_root).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update staging is unavailable",
        )
    })?;
    let directory = staging_root.join(&manifest.version);
    std::fs::create_dir(&directory).map_err(|error| {
        fail(
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ErrorCode::Conflict
            } else {
                ErrorCode::StorageUnavailable
            },
            "Update staging directory could not be created",
        )
    })?;
    let result = (|| {
        let staged_archive = directory.join(name);
        copy_verified_payload(manifest, archive, &staged_archive)?;
        let manifest_path = directory.join("manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(manifest)
                .map_err(|_| fail(ErrorCode::InvalidRequest, "Update manifest is invalid"))?,
        )
        .map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Update manifest could not be staged",
            )
        })?;
        std::fs::write(directory.join("READY"), b"verified\n").map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Update staging is unavailable",
            )
        })?;
        Ok(StagedUpdate {
            directory: directory.clone(),
            archive: staged_archive,
            manifest: manifest_path,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&directory);
    }
    result
}

pub struct SessionGuard {
    _file: File,
    marker: PathBuf,
}
impl SessionGuard {
    /// A running consumer/runtime holds a shared lease. Activation needs the
    /// exclusive lease, so it cannot replace a binary another session is using.
    /// A visible marker also lets an operator see why activation is blocked.
    pub fn begin(root: &Path) -> Result<Self, DomainError> {
        std::fs::create_dir_all(root)
            .map_err(|_| fail(ErrorCode::StorageUnavailable, "Update state is unavailable"))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("sessions.lock"))
            .map_err(|_| fail(ErrorCode::StorageUnavailable, "Update lease is unavailable"))?;
        fs4::fs_std::FileExt::lock_shared(&file).map_err(|_| {
            fail(
                ErrorCode::LocalAccessDenied,
                "Another update holds the session lease",
            )
        })?;
        let marker = root.join(format!("session-{}-{}.json", std::process::id(), unix_ms()));
        let _ = std::fs::write(
            &marker,
            serde_json::to_vec(&serde_json::json!({
                "pid": std::process::id(),
                "startedAt": unix_ms(),
            }))
            .unwrap_or_default(),
        );
        Ok(Self {
            _file: file,
            marker,
        })
    }
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.marker);
    }
}

fn session_markers(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry.file_name().to_string_lossy().starts_with("session-")
                && entry.file_name().to_string_lossy().ends_with(".json")
                && !entry.file_name().to_string_lossy().ends_with(".stale.json")
        })
        .map(|entry| entry.path())
        .filter_map(|path| {
            let started = read_bounded_file(&path, 4096)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|value| value["startedAt"].as_u64());
            if started
                .is_some_and(|started| unix_ms().saturating_sub(started) > 24 * 60 * 60 * 1000)
            {
                // A crashed consumer cannot release its marker. Preserve it as
                // evidence but do not block updates forever.
                let stale = path.with_extension("stale.json");
                let _ = std::fs::rename(&path, stale);
                None
            } else {
                Some(path)
            }
        })
        .collect()
}

// Held under activation.lock. The separate floor survives a process crash
// between executable replacement and the informational state.json write.
fn activation_floor(root: &Path) -> Result<String, DomainError> {
    let invalid = || {
        fail(
            ErrorCode::StorageUnavailable,
            "Update version state is invalid; restore it before activating",
        )
    };
    let mut highest = parse_version(env!("CARGO_PKG_VERSION")).ok_or_else(invalid)?;
    for (name, field) in [
        ("state.json", "installedVersion"),
        ("version-floor.json", "highestVerifiedVersion"),
    ] {
        let bytes = match read_bounded_file(&root.join(name), MAX_MANIFEST_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(invalid()),
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        let version = value[field]
            .as_str()
            .and_then(parse_version)
            .ok_or_else(invalid)?;
        if version.cmp_precedence(&highest).is_gt() {
            highest = version;
        }
    }
    Ok(highest.to_string())
}

fn persist_version_floor(root: &Path, version: &str) -> Result<(), DomainError> {
    let temporary = root.join(format!(
        "version-floor-{}-{}.tmp",
        std::process::id(),
        unix_ms()
    ));
    let mut created = false;
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        created = true;
        serde_json::to_writer(
            &mut file,
            &serde_json::json!({"highestVerifiedVersion": version}),
        )?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, root.join("version-floor.json"))?;
        #[cfg(unix)]
        File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() && created {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update version floor could not be saved; executable was not replaced",
        )
    })
}

pub struct Activation {
    pub backup: PathBuf,
    pub installed: PathBuf,
}

/// Atomically replace a target executable after verifying the staged archive.
/// `active_sessions` is represented by a shared lease; callers that need live
/// session coordination should also drain sessions before invoking this.
pub fn activate(
    state_root: &Path,
    staged: &StagedUpdate,
    manifest: &UpdateManifest,
    target: &Path,
    public_key: &[u8],
) -> Result<Activation, DomainError> {
    std::fs::create_dir_all(state_root)
        .map_err(|_| fail(ErrorCode::StorageUnavailable, "Update state is unavailable"))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state_root.join("activation.lock"))
        .map_err(|_| fail(ErrorCode::StorageUnavailable, "Update lock is unavailable"))?;
    fs4::fs_std::FileExt::try_lock_exclusive(&lock).map_err(|_| {
        fail(
            ErrorCode::Conflict,
            "Another update activation is already in progress",
        )
    })?;
    let sessions = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state_root.join("sessions.lock"))
        .map_err(|_| fail(ErrorCode::StorageUnavailable, "Update lease is unavailable"))?;
    fs4::fs_std::FileExt::try_lock_exclusive(&sessions).map_err(|_| {
        fail(
            ErrorCode::Conflict,
            "An active consumer/runtime session prevents activation",
        )
    })?;
    if !session_markers(state_root).is_empty() {
        return Err(fail(
            ErrorCode::Conflict,
            "An active consumer/runtime session marker prevents activation",
        ));
    }
    let floor = activation_floor(state_root)?;
    verify_update(manifest, &staged.archive, public_key, &floor, "1.0", false)?;
    let backup_dir = state_root.join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update backup is unavailable",
        )
    })?;
    let backup = backup_dir.join(format!("{}-{}.bak", manifest.version, unix_ms()));
    if target.exists() {
        std::fs::copy(target, &backup).map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Current executable could not be backed up",
            )
        })?;
    }
    // The signed update payload is one executable, not an archive requiring
    // a separately trusted extractor. Never install an unrelated staged file.
    let replacement = target.with_extension(format!("new-{}", unix_ms()));
    copy_verified_payload(manifest, &staged.archive, &replacement)?;
    if let Err(error) = persist_version_floor(state_root, &manifest.version) {
        let _ = std::fs::remove_file(&replacement);
        return Err(error);
    }
    if std::fs::rename(&replacement, target).is_err() {
        let _ = std::fs::remove_file(&replacement);
        return Err(fail(
            ErrorCode::StorageUnavailable,
            "Executable replacement failed; the previous binary is intact",
        ));
    }
    let _ = std::fs::write(
        state_root.join("state.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "installedVersion": manifest.version,
            "protocolVersion": manifest.protocol_version,
            "backup": backup,
            "installedAt": manifest.published_at,
        }))
        .unwrap_or_default(),
    );
    Ok(Activation {
        backup,
        installed: target.to_path_buf(),
    })
}

/// Restore the exact backup recorded by `activate`. The caller retains that
/// backup; this operation replaces the currently installed target.
pub fn rollback(target: &Path, backup: &Path) -> Result<(), DomainError> {
    if !backup.is_file() {
        return Err(fail(ErrorCode::NotFound, "Update backup is unavailable"));
    }
    let replacement = target.with_extension(format!("rollback-{}", unix_ms()));
    std::fs::copy(backup, &replacement)
        .map_err(|_| fail(ErrorCode::StorageUnavailable, "Rollback copy failed"))?;
    std::fs::rename(&replacement, target).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Rollback replacement failed; no file was deleted",
        )
    })?;
    Ok(())
}

fn read_bounded_file(path: &Path, max: u64) -> std::io::Result<Vec<u8>> {
    let metadata = path.symlink_metadata()?;
    if !metadata.is_file() || metadata.len() > max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Expected a bounded regular file",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "File exceeds the bound",
        ));
    }
    Ok(bytes)
}

pub fn read_manifest(path: &Path) -> Result<UpdateManifest, DomainError> {
    let metadata = std::fs::metadata(path).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update manifest is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(fail(
            ErrorCode::FrameTooLarge,
            "Update manifest exceeds the bound",
        ));
    }
    let bytes = read_bounded_file(path, MAX_MANIFEST_BYTES).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update manifest is unreadable",
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| fail(ErrorCode::InvalidRequest, "Update manifest is malformed"))
}

pub fn write_unsigned_manifest(
    path: &Path,
    version: &str,
    channel: &str,
    protocol_version: &str,
    archive: &Path,
    archive_name: &str,
    published_at: &str,
) -> Result<UpdateManifest, DomainError> {
    let bytes = std::fs::metadata(archive)
        .map_err(|_| {
            fail(
                ErrorCode::StorageUnavailable,
                "Release archive is unavailable",
            )
        })?
        .len();
    if bytes > MAX_ARCHIVE_BYTES {
        return Err(fail(
            ErrorCode::FrameTooLarge,
            "Release archive exceeds the bound",
        ));
    }
    let manifest = UpdateManifest {
        schema_version: UPDATE_SCHEMA,
        version: version.into(),
        channel: channel.into(),
        platform: platform_id(),
        protocol_version: protocol_version.into(),
        archive: archive_name.into(),
        archive_bytes: bytes,
        archive_sha256: sha256_file(archive)?,
        published_at: published_at.into(),
        notes_url: None,
        signature: String::new(),
    };
    let mut file = File::create(path).map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update manifest could not be written",
        )
    })?;
    file.write_all(
        &serde_json::to_vec_pretty(&manifest)
            .map_err(|_| fail(ErrorCode::InvalidRequest, "Update manifest is invalid"))?,
    )
    .map_err(|_| {
        fail(
            ErrorCode::StorageUnavailable,
            "Update manifest could not be written",
        )
    })?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    struct Signed {
        manifest: UpdateManifest,
        public_key: Vec<u8>,
        archive: PathBuf,
        _temp: tempfile::TempDir,
    }

    fn signed(version: &str) -> Signed {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("bundle.zip");
        std::fs::write(&archive, b"new-binary").unwrap();
        let mut manifest = write_unsigned_manifest(
            &temp.path().join("manifest.json"),
            version,
            "alpha",
            "1.0",
            &archive,
            "bundle.zip",
            "2026-10-22T00:00:00.000Z",
        )
        .unwrap();
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let message = canonical_manifest(&manifest).unwrap();
        use base64::Engine;
        manifest.signature =
            base64::engine::general_purpose::STANDARD.encode(pair.sign(&message).as_ref());
        Signed {
            manifest,
            public_key: pair.public_key().as_ref().to_vec(),
            archive,
            _temp: temp,
        }
    }

    #[test]
    fn activation_remembers_the_highest_version_across_calls_and_rollback() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("current");
        std::fs::write(&target, b"old").unwrap();
        let next = signed("0.1.0-alpha.22");
        let staged = stage_update(
            &next.manifest,
            &next.archive,
            &root.path().join("staging"),
            &next.public_key,
            "0.1.0-alpha.20",
            "1.0",
        )
        .unwrap();
        let activation = activate(
            root.path(),
            &staged,
            &next.manifest,
            &target,
            &next.public_key,
        )
        .unwrap();
        rollback(&target, &activation.backup).unwrap();
        // Model the crash window before informational metadata was written.
        std::fs::remove_file(root.path().join("state.json")).unwrap();
        let older = signed("0.1.0-alpha.21");
        let staged = stage_update(
            &older.manifest,
            &older.archive,
            &root.path().join("staging"),
            &older.public_key,
            "0.1.0-alpha.20",
            "1.0",
        )
        .unwrap();
        assert_eq!(
            activate(
                root.path(),
                &staged,
                &older.manifest,
                &target,
                &older.public_key
            )
            .err()
            .unwrap()
            .code,
            ErrorCode::Conflict
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
    }

    #[test]
    fn corrupt_version_state_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("version-floor.json"), b"{}").unwrap();
        assert_eq!(
            activation_floor(root.path()).unwrap_err().code,
            ErrorCode::StorageUnavailable
        );
    }

    #[test]
    fn prerelease_numbers_and_staging_names_follow_semver() {
        let update = signed("0.1.0-alpha.20");
        verify_update(
            &update.manifest,
            &update.archive,
            &update.public_key,
            "0.1.0-alpha.9",
            "1.0",
            false,
        )
        .unwrap();
        let older = signed("0.1.0-alpha.9");
        assert_eq!(
            verify_update(
                &older.manifest,
                &older.archive,
                &older.public_key,
                "0.1.0-alpha.20",
                "1.0",
                false
            )
            .unwrap_err()
            .code,
            ErrorCode::Conflict
        );
        for version in [
            "0.1.0-alpha/../../escape",
            "0.1.0-alphabet",
            "0.1.0-alpha.020",
            "0.1.0-alpha.20+metadata",
        ] {
            let update = signed(version);
            assert!(
                verify_update(
                    &update.manifest,
                    &update.archive,
                    &update.public_key,
                    "0.1.0-alpha.1",
                    "1.0",
                    false
                )
                .is_err()
            );
        }
    }

    #[test]
    fn signature_checksum_platform_and_downgrade_are_all_required() {
        let signed = signed("0.1.0-alpha.21");
        verify_update(
            &signed.manifest,
            &signed.archive,
            &signed.public_key,
            "0.1.0-alpha.17",
            "1.0",
            false,
        )
        .unwrap();
        let mut tampered = signed.manifest.clone();
        tampered.archive_sha256 = "0".repeat(64);
        assert!(
            verify_update(
                &tampered,
                &signed.archive,
                &signed.public_key,
                "0.1.0-alpha.17",
                "1.0",
                false
            )
            .is_err()
        );
        let mut downgrade = signed.manifest.clone();
        let message = canonical_manifest(&downgrade).unwrap();
        downgrade.signature = signed.manifest.signature.clone();
        assert!(
            verify_update(
                &downgrade,
                &signed.archive,
                &signed.public_key,
                "0.1.0-alpha.22",
                "1.0",
                false
            )
            .is_err()
        );
        let _ = message;
        let mut wrong_platform = signed.manifest.clone();
        wrong_platform.platform = "plan9-mips".into();
        assert!(
            verify_update(
                &wrong_platform,
                &signed.archive,
                &signed.public_key,
                "0.1.0-alpha.17",
                "1.0",
                false
            )
            .is_err()
        );
    }

    #[test]
    fn staged_activation_refuses_a_live_session_and_rolls_back() {
        let signed = signed("0.1.0-alpha.21");
        let root = tempfile::tempdir().unwrap();
        let staging = root.path().join("staged");
        let staged = stage_update(
            &signed.manifest,
            &signed.archive,
            &staging,
            &signed.public_key,
            "0.1.0-alpha.17",
            "1.0",
        )
        .unwrap();
        let executable = staged.directory.join(if cfg!(windows) {
            "originlm.exe"
        } else {
            "originlm"
        });
        std::fs::write(&executable, b"unsigned-staged-file").unwrap();
        let target = root.path().join(if cfg!(windows) {
            "current.exe"
        } else {
            "current"
        });
        std::fs::write(&target, b"old-binary").unwrap();
        {
            let _session = SessionGuard::begin(root.path()).unwrap();
            assert!(
                activate(
                    root.path(),
                    &staged,
                    &signed.manifest,
                    &target,
                    &signed.public_key
                )
                .is_err()
            );
            assert_eq!(std::fs::read(&target).unwrap(), b"old-binary");
        }
        let activation = activate(
            root.path(),
            &staged,
            &signed.manifest,
            &target,
            &signed.public_key,
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new-binary");
        rollback(&target, &activation.backup).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"old-binary");
    }

    #[test]
    fn an_old_crash_marker_is_preserved_but_does_not_block_activation() {
        let signed = signed("0.1.0-alpha.21");
        let root = tempfile::tempdir().unwrap();
        let staged = stage_update(
            &signed.manifest,
            &signed.archive,
            &root.path().join("staged"),
            &signed.public_key,
            "0.1.0-alpha.17",
            "1.0",
        )
        .unwrap();
        let executable = staged.directory.join(if cfg!(windows) {
            "originlm.exe"
        } else {
            "originlm"
        });
        std::fs::write(&executable, b"unsigned-staged-file").unwrap();
        let target = root.path().join("current");
        std::fs::write(&target, b"old-binary").unwrap();
        let marker = root.path().join("session-999.json");
        std::fs::write(&marker, br#"{"pid":999,"startedAt":0}"#).unwrap();
        activate(
            root.path(),
            &staged,
            &signed.manifest,
            &target,
            &signed.public_key,
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new-binary");
        assert!(!marker.exists());
        assert!(
            root.path()
                .read_dir()
                .unwrap()
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().ends_with(".stale.json"))
        );
    }

    #[test]
    fn a_failed_staging_copy_never_touches_the_installed_binary() {
        let signed = signed("0.1.0-alpha.21");
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("current");
        std::fs::write(&target, b"old-binary").unwrap();
        let staging = root.path().join("staged");
        let staged = stage_update(
            &signed.manifest,
            &signed.archive,
            &staging,
            &signed.public_key,
            "0.1.0-alpha.17",
            "1.0",
        )
        .unwrap();
        std::fs::remove_file(&staged.archive).unwrap();
        assert!(
            activate(
                root.path(),
                &staged,
                &signed.manifest,
                &target,
                &signed.public_key
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"old-binary");
    }
}
