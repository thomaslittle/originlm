//! Locating the Muse binary and building `muse serve` launches.
use crate::process::LaunchSpec;
use originlm_contracts::*;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

/// The host-wide posture `muse serve` is started with. MSP fixes sandbox,
/// shell and write posture for the host's lifetime; it cannot be widened over
/// the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    /// No shell, no non-shell writes, no web search/fetch, no artifacts and
    /// no workflow launches. Muse's own read-only workspace tools remain.
    ReadOnly,
    /// Muse may edit the workspace and run commands under its own sandbox
    /// and approval policy.
    Workspace,
}

fn missing() -> DomainError {
    DomainError::new(
        ErrorCode::RuntimeMissing,
        "A native Muse Code executable is unavailable",
    )
}

/// `1.4.7-R6101.1`: the release form the official launcher records.
pub fn valid_release(version: &str) -> bool {
    let Some((semver, release)) = version.split_once("-R") else {
        return false;
    };
    let numeric = |part: &str| {
        !part.is_empty() && part.len() <= 9 && part.bytes().all(|b| b.is_ascii_digit())
    };
    let parts: Vec<_> = semver.split('.').collect();
    let release: Vec<_> = release.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| numeric(p))
        && (1..=2).contains(&release.len())
        && release.iter().all(|p| numeric(p))
}

/// The native Muse binary behind a discovered command.
///
/// The official Windows install puts a `muse.cmd` launcher next to versioned
/// `muse-bin-<release>.exe` binaries and records the active release in
/// `.muse-version`. The launcher runs PowerShell and may self-update over the
/// network, so the kit resolves it to the active binary (as the launcher
/// itself does) instead of running a shell. A configured binary
/// (`muse-bin-*.exe`, `muse.exe`) is used as is. On other platforms the
/// discovered `muse` command is executed directly with auto-update disabled.
pub fn resolve_executable(candidate: &Path) -> Result<PathBuf> {
    if !candidate.is_absolute() || !candidate.is_file() {
        return Err(missing());
    }
    let name = candidate
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if cfg!(windows) {
        if name == "muse.exe" || (name.starts_with("muse-bin-") && name.ends_with(".exe")) {
            return Ok(candidate.to_owned());
        }
        if matches!(name.as_str(), "muse.cmd" | "muse.ps1" | "muse.bat" | "muse") {
            let base = candidate.parent().ok_or_else(missing)?;
            return active_binary(base);
        }
        return Err(missing());
    }
    if name == "muse" || name.starts_with("muse-bin-") {
        return Ok(candidate.to_owned());
    }
    Err(missing())
}

/// `<dir>/muse-bin-<.muse-version>.exe`, exactly as the official launcher picks it.
pub fn active_binary(dir: &Path) -> Result<PathBuf> {
    let state = dir.join(".muse-version");
    let meta = std::fs::symlink_metadata(&state).map_err(|_| missing())?;
    if !meta.is_file() || meta.len() > 64 {
        return Err(missing());
    }
    let version = std::fs::read_to_string(&state).map_err(|_| missing())?;
    let version = version.trim();
    if !valid_release(version) {
        return Err(missing());
    }
    let binary = dir.join(format!("muse-bin-{version}.exe"));
    if binary.is_file() {
        Ok(binary)
    } else {
        Err(missing())
    }
}

/// The `muse serve` arguments for a posture. A non-persistent host keeps
/// sessions in memory only (`--no-session-log`). Muse settings are passed as
/// `-c KEY=JSON` and refuse the run if Muse does not know them, so an
/// incompatible Muse fails closed instead of silently keeping a tool.
pub fn serve_args(posture: Posture, persistent: bool) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["serve".into()];
    if !persistent {
        args.push("--no-session-log".into());
    }
    if posture == Posture::ReadOnly {
        args.extend(["--disable-shell".into(), "--disable-write".into()]);
        for setting in [
            "tools.web_search.mode=\"off\"",
            "tools.web_fetch.enabled=false",
            "tools.artifact.enabled=false",
            "run.workflow_trigger_mode=\"off\"",
        ] {
            args.extend(["-c".into(), setting.into()]);
        }
    }
    args
}

/// A `muse serve` launch in `cwd`. Only non-secret switches are added to the
/// kit's scrubbed environment: auto-update stays off (the launcher would
/// otherwise contact the network), and in host-reviewed sessions Muse's
/// LLM approval judge is disabled so every approval reaches the host.
pub fn serve_spec(
    executable: PathBuf,
    cwd: PathBuf,
    posture: Posture,
    persistent: bool,
    host_reviews: bool,
) -> LaunchSpec {
    let mut env: BTreeMap<OsString, OsString> =
        BTreeMap::from([("MUSE_NO_AUTO_UPDATE".into(), "1".into())]);
    if host_reviews {
        env.insert("MUSE_DISABLE_APPROVAL_JUDGE".into(), "1".into());
    }
    LaunchSpec {
        executable,
        args: serve_args(posture, persistent),
        cwd,
        env,
    }
}

/// The workspace root as `session/start` accepts it. On Windows Muse rejects
/// anything but the verbatim canonical form (`\\?\C:\...`), which is what
/// `std::fs::canonicalize` returns there.
pub fn workspace_root(workspace: &Path) -> Result<String> {
    let canonical = std::fs::canonicalize(workspace).map_err(|_| {
        DomainError::new(
            ErrorCode::LocalAccessDenied,
            "The session workspace is unavailable",
        )
    })?;
    canonical.into_os_string().into_string().map_err(|_| {
        DomainError::new(
            ErrorCode::LocalAccessDenied,
            "The session workspace path is not valid Unicode",
        )
    })
}

/// A UUIDv7, the idempotency handle MSP requires for every command
/// (`commandId`) and the form of a client-minted session id.
pub fn uuid_v7() -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| {
        DomainError::new(
            ErrorCode::ProviderUnavailable,
            "Secure command id generation failed",
        )
    })?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    b[..6].copy_from_slice(&millis.to_be_bytes()[2..]);
    b[6] = (b[6] & 0x0f) | 0x70;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// Lowercase canonical UUID text; native session ids are checked against it.
pub fn valid_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_versions_match_the_launcher_pattern() {
        assert!(valid_release("1.4.7-R6101.1"));
        assert!(valid_release("1.4.6-R6000"));
        assert!(!valid_release("1.4.7"));
        assert!(!valid_release("1.4-R1.1"));
        assert!(!valid_release("1.4.7-R6101.1.2"));
        assert!(!valid_release("../../evil-R1"));
        assert!(!valid_release("1.4.7-R61 01"));
    }
    #[cfg(windows)]
    #[test]
    fn the_windows_launcher_resolves_to_its_active_binary() {
        let dir = tempfile::tempdir().unwrap();
        let launcher = dir.path().join("muse.cmd");
        std::fs::write(&launcher, b"@echo off").unwrap();
        assert!(resolve_executable(&launcher).is_err());
        std::fs::write(dir.path().join(".muse-version"), b"1.4.7-R6101.1\n").unwrap();
        assert!(resolve_executable(&launcher).is_err());
        let binary = dir.path().join("muse-bin-1.4.7-R6101.1.exe");
        std::fs::write(&binary, b"MZ").unwrap();
        assert_eq!(resolve_executable(&launcher).unwrap(), binary);
        assert_eq!(resolve_executable(&binary).unwrap(), binary);
        std::fs::write(dir.path().join(".muse-version"), b"..\\..\\evil-R1").unwrap();
        assert!(resolve_executable(&launcher).is_err());
        let other = dir.path().join("node.exe");
        std::fs::write(&other, b"MZ").unwrap();
        assert!(resolve_executable(&other).is_err());
        assert!(resolve_executable(Path::new("muse.cmd")).is_err());
    }
    #[cfg(not(windows))]
    #[test]
    fn unix_runs_the_discovered_muse_command() {
        let dir = tempfile::tempdir().unwrap();
        let muse = dir.path().join("muse");
        std::fs::write(&muse, b"#!/bin/sh").unwrap();
        assert_eq!(resolve_executable(&muse).unwrap(), muse);
        let other = dir.path().join("python3");
        std::fs::write(&other, b"").unwrap();
        assert!(resolve_executable(&other).is_err());
    }
    #[test]
    fn uuid_v7_has_version_and_variant_bits() {
        let id = uuid_v7().unwrap();
        assert!(valid_uuid(&id));
        assert_eq!(&id[14..15], "7");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        assert_ne!(id, uuid_v7().unwrap());
    }
    #[test]
    fn read_only_hosts_remove_shell_writes_and_network_tools() {
        let args: Vec<String> = serve_args(Posture::ReadOnly, false)
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect();
        assert_eq!(
            &args[..4],
            [
                "serve",
                "--no-session-log",
                "--disable-shell",
                "--disable-write"
            ]
        );
        assert!(args.contains(&"tools.web_search.mode=\"off\"".to_owned()));
        assert!(args.contains(&"tools.web_fetch.enabled=false".to_owned()));
        assert!(args.contains(&"run.workflow_trigger_mode=\"off\"".to_owned()));
        let workspace: Vec<String> = serve_args(Posture::Workspace, true)
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect();
        assert_eq!(workspace, ["serve"]);
    }
}
