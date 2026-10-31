//! Managed Antigravity ACP server installation.
//!
//! The Antigravity desktop/CLI does not ship the ACP server or its Python
//! harness in a stable user location. Google publishes a versioned zip for each
//! platform; this module downloads it, verifies the archive and member sizes and
//! SHA-256, and stages the two files in the profile home the ACP launcher
//! expects. No credentials or account tokens are read here.
use crate::process::LaunchSpec;
use originlm_contracts::*;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 1200 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(45 * 60);

#[derive(Clone, Copy, Debug)]
pub struct Release {
    pub version: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub archive_bytes: u64,
    pub executable: &'static str,
    pub executable_bytes: u64,
    pub harness: &'static str,
    pub harness_bytes: u64,
}

/// Pinned Google release assets, matching T3 Code's `antigravityRelease.ts`.
/// The version is deliberately fixed: 1.4.2+ writes skills into every session
/// workspace, so a silent upgrade would change provider behavior.
pub const RELEASES: &[(&str, Release)] = &[
    (
        "linux-x64",
        Release {
            version: "1.4.1",
            url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.4.1-linux-x86_64.zip",
            sha256: "2e37012ceb2a5c7fc7309b6e6596234fb637ee3c62a5199f65d4cbe777871384",
            archive_bytes: 336_170_339,
            executable: "agy_acp_server.par",
            executable_bytes: 930_826_753,
            harness: "localharness_external",
            harness_bytes: 131_187_512,
        },
    ),
    (
        "linux-arm64",
        Release {
            version: "1.4.1",
            url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.4.1-linux-arm64.zip",
            sha256: "609336a1c97545487e3ddd55dbd153dc0f40f4e6534486cc1c5f4c0ebb85c2da",
            archive_bytes: 321_922_507,
            executable: "agy_acp_server.par",
            executable_bytes: 933_416_796,
            harness: "localharness_external",
            harness_bytes: 124_895_000,
        },
    ),
    (
        "darwin-arm64",
        Release {
            version: "1.4.1",
            url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.4.1-darwin-arm64.zip",
            sha256: "f9bdd68291ae78c38d2141e63bc86b84e6cc7b9f60f71652c68435a7943deb17",
            archive_bytes: 112_737_434,
            executable: "agy_acp_server.par",
            executable_bytes: 280_996_576,
            harness: "localharness_external",
            harness_bytes: 120_246_928,
        },
    ),
    (
        "darwin-x64",
        Release {
            version: "1.4.1",
            url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.4.1-darwin-x86_64.zip",
            sha256: "27e31f096874704f14167b07ef9e84cc77134615ee41d024f7b1c957add85307",
            archive_bytes: 118_349_036,
            executable: "agy_acp_server.par",
            executable_bytes: 285_186_624,
            harness: "localharness_external",
            harness_bytes: 124_910_048,
        },
    ),
    (
        "win32-x64",
        Release {
            version: "1.4.1",
            url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-1.4.1-windows-x86_64.zip",
            sha256: "5f5b7d0328df17c128b2be338d4567b0d24e56ace8196e88794fec37d1372554",
            archive_bytes: 129_599_308,
            executable: "agy_acp_server.exe",
            executable_bytes: 86_098_784,
            harness: "localharness_external.exe",
            harness_bytes: 146_036_888,
        },
    ),
    (
        "win32-arm64",
        Release {
            version: "1.4.1",
            url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-1.4.1-windows-arm64.zip",
            sha256: "24c52bbc12aae20f55eb53d373669412d95a9c44fd59be4ea4e355918b91fbf7",
            archive_bytes: 132_507_344,
            executable: "agy_acp_server.exe",
            executable_bytes: 93_197_744,
            harness: "localharness_external.exe",
            harness_bytes: 136_970_904,
        },
    ),
];

pub fn release_for(platform: &str, arch: &str) -> Option<Release> {
    let key = format!("{platform}-{arch}");
    RELEASES
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, release)| *release)
}

fn unavailable(reason: &str) -> DomainError {
    DomainError::new(ErrorCode::ProviderUnavailable, reason)
}

fn invalid(reason: &str) -> DomainError {
    DomainError::new(ErrorCode::ProtocolIncompatible, reason)
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Download, verify and stage one release into `base_dir`.
///
/// `base_dir` must end in `antigravity-acp`; that is the profile home the
/// native launcher validates. Existing complete installs are reused.
pub async fn install(base_dir: &Path, release: Release) -> Result<PathBuf> {
    if base_dir.file_name().and_then(|name| name.to_str()) != Some("antigravity-acp") {
        return Err(invalid("Antigravity profile home must be named antigravity-acp"));
    }
    let record = base_dir.join(".install-complete.json");
    if let Ok(text) = std::fs::read_to_string(&record)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && value["version"].as_str() == Some(release.version)
        && value["executable"]["name"].as_str() == Some(release.executable)
        && value["executable"]["bytes"].as_u64() == Some(release.executable_bytes)
        && value["harness"]["name"].as_str() == Some(release.harness)
        && value["harness"]["bytes"].as_u64() == Some(release.harness_bytes)
    {
        return Ok(base_dir.to_path_buf());
    }
    std::fs::create_dir_all(base_dir).map_err(|_| unavailable("Antigravity profile home is unavailable"))?;
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|_| unavailable("Antigravity download client is unavailable"))?;
    let response = client
        .get(release.url)
        .send()
        .await
        .map_err(|_| unavailable("Antigravity release download failed"))?;
    if !response.status().is_success() {
        return Err(unavailable("Antigravity release download was rejected"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| unavailable("Antigravity release download was interrupted"))?;
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES
        || bytes.len() as u64 != release.archive_bytes
        || digest(&bytes) != release.sha256
    {
        return Err(invalid("Antigravity release checksum mismatch"));
    }
    let base = base_dir.to_path_buf();
    let release_copy = release;
    tokio::task::spawn_blocking(move || stage_archive(&base, &bytes, release_copy))
        .await
        .map_err(|_| unavailable("Antigravity extraction was interrupted"))??;
    Ok(base_dir.to_path_buf())
}

fn stage_archive(base_dir: &Path, bytes: &[u8], release: Release) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid("Antigravity archive is invalid"))?;
    let mut staged = std::collections::BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| invalid("Antigravity archive member is invalid"))?;
        let name = entry
            .enclosed_name()
            .and_then(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .ok_or_else(|| invalid("Antigravity archive member has no name"))?;
        let expected = if name == release.executable {
            release.executable_bytes
        } else if name == release.harness {
            release.harness_bytes
        } else {
            continue;
        };
        if entry.size() != expected || entry.size() > MAX_MEMBER_BYTES {
            return Err(invalid("Antigravity archive member size mismatch"));
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut data)
            .map_err(|_| invalid("Antigravity archive member is unreadable"))?;
        if data.len() as u64 != expected {
            return Err(invalid("Antigravity archive member size mismatch"));
        }
        staged.insert(name, data);
    }
    if !staged.contains_key(release.executable) || !staged.contains_key(release.harness) {
        return Err(invalid("Antigravity archive is missing a required member"));
    }
    let temporary = base_dir.join(".install-tmp");
    std::fs::create_dir_all(&temporary)
        .map_err(|_| unavailable("Antigravity staging directory is unavailable"))?;
    for (name, data) in &staged {
        let path = temporary.join(name);
        let mut file = std::fs::File::create(&path)
            .map_err(|_| unavailable("Antigravity member could not be written"))?;
        file.write_all(data)
            .map_err(|_| unavailable("Antigravity member could not be written"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| unavailable("Antigravity member permissions could not be set"))?;
        }
    }
    let record = serde_json::json!({
        "version": release.version,
        "source": release.url,
        "archiveSha256": release.sha256,
        "executable": { "name": release.executable, "bytes": release.executable_bytes },
        "harness": { "name": release.harness, "bytes": release.harness_bytes }
    });
    std::fs::write(
        temporary.join(".install-complete.json"),
        serde_json::to_vec_pretty(&record).map_err(|_| invalid("Antigravity record is invalid"))?,
    )
    .map_err(|_| unavailable("Antigravity record could not be written"))?;
    for entry in std::fs::read_dir(&temporary)
        .map_err(|_| unavailable("Antigravity staging directory is unreadable"))?
    {
        let entry = entry.map_err(|_| unavailable("Antigravity staging entry is unreadable"))?;
        let destination = base_dir.join(entry.file_name());
        std::fs::rename(entry.path(), destination)
            .map_err(|_| unavailable("Antigravity member could not be published"))?;
    }
    let _ = std::fs::remove_dir(&temporary);
    Ok(())
}

/// A tiny launch spec for a future auth flow; keeps the installer independent
/// of the ACP session implementation.
pub fn auth_launch(executable: &Path, profile_home: &Path, workspace: &Path) -> LaunchSpec {
    LaunchSpec {
        executable: executable.to_path_buf(),
        args: Vec::new(),
        cwd: workspace.to_path_buf(),
        env: [(
            std::ffi::OsString::from("GEMINI_HOME"),
            profile_home
                .parent()
                .unwrap_or(profile_home)
                .as_os_str()
                .to_owned(),
        )]
        .into_iter()
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_table_is_pinned_and_platform_specific() {
        assert_eq!(release_for("linux", "x64").unwrap().version, "1.4.1");
        assert_eq!(release_for("win32", "arm64").unwrap().executable, "agy_acp_server.exe");
        assert!(release_for("linux", "mips").is_none());
    }

    #[test]
    fn rejects_a_foreign_profile_home() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(install(
            Path::new("/tmp/not-antigravity"),
            RELEASES[0].1,
        ));
        assert!(result.is_err());
    }
}
