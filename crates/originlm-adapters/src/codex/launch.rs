use crate::process::{LaunchSpec, codex_managed_metadata, codex_metadata};
use originlm_contracts::{ConnectionMetadata, DomainError, ErrorCode, Result};
use originlm_discovery::LocatedClient;
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
fn missing() -> DomainError {
    DomainError::new(
        ErrorCode::RuntimeMissing,
        "A supported native Codex executable is unavailable",
    )
}
fn incomplete_codex() -> DomainError {
    DomainError::new(
        ErrorCode::RuntimeMissing,
        "Codex needs the adjacent codex-code-mode-host executable",
    )
}
/// Native Codex ships `codex-code-mode-host` beside the binary. Launchers and
/// version-manager shims resolve their own payload, so a shebang, a non-`.exe`
/// Windows launcher, or a volta/mise/aqua shim is enough. Reimplemented from
/// T3 Code `CodexInstallation.ts` (`hasCodeModeHost`) at
/// [348888a767](https://github.com/pingdotgg/t3code/commit/348888a767).
fn codex_binary_is_launchable(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(&name);
    if matches!(stem, "volta-shim" | "mise" | "aqua-proxy") {
        return true;
    }
    if cfg!(windows) {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if extension != "exe" {
            return true;
        }
        if let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            let parent = path.parent().unwrap_or(Path::new("."));
            for marker in [".bunx", ".shim"] {
                if non_empty_file(&parent.join(format!("{file_stem}{marker}"))) {
                    return true;
                }
            }
        }
    } else if starts_with_shebang(path) {
        return true;
    }
    let host_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };
    let host = path.parent().unwrap_or(Path::new(".")).join(host_name);
    if !non_empty_file(&host) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(&host).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}
fn non_empty_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
}
fn starts_with_shebang(path: &Path) -> bool {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut header = [0_u8; 2];
    file.read_exact(&mut header).is_ok() && header == *b"#!"
}
fn ensure_launchable(path: PathBuf) -> Result<PathBuf> {
    if codex_binary_is_launchable(&path) {
        Ok(path)
    } else {
        Err(incomplete_codex())
    }
}

pub fn resolve_executable(candidate: &Path) -> Result<PathBuf> {
    if !candidate.is_absolute() {
        return Err(missing());
    }
    let name = candidate.file_name().and_then(|n| n.to_str()).unwrap_or("");
    // Check provider identity on the logical command, never a versioned target's name.
    // This is an executable candidate, not publisher/signature attestation.
    #[cfg(unix)]
    if std::fs::symlink_metadata(candidate).is_ok_and(|m| m.file_type().is_symlink()) {
        if name != "codex" {
            return Err(missing());
        }
        return originlm_discovery::resolve_executable(candidate)
            .map(|identity| identity.resolved)
            .map_err(|_| missing());
    }
    if !candidate.is_file() {
        return Err(missing());
    }
    if name.eq_ignore_ascii_case("codex.exe") {
        return ensure_launchable(candidate.to_owned());
    }
    let wrapper = name.eq_ignore_ascii_case("codex.cmd")
        || name.eq_ignore_ascii_case("codex.ps1")
        || (cfg!(windows) && name == "codex");
    if wrapper {
        let base = candidate.parent().ok_or_else(missing)?;
        let (package, target) = if cfg!(target_arch = "aarch64") {
            ("codex-win32-arm64", "aarch64-pc-windows-msvc")
        } else {
            ("codex-win32-x64", "x86_64-pc-windows-msvc")
        };
        let roots = [
            base.join(format!(
                "node_modules/@openai/codex/node_modules/@openai/{package}"
            )),
            base.join(format!("node_modules/@openai/{package}")),
            base.join("node_modules/@openai/codex"),
        ];
        for root in roots {
            let binary = root.join(format!("vendor/{target}/bin/codex.exe"));
            if binary.is_file() {
                return ensure_launchable(binary);
            }
        }
        return Err(missing());
    }
    if name == "codex" {
        // Do not execute a JavaScript/shell wrapper as if it were a native Windows binary.
        #[cfg(windows)]
        return Err(missing());
        #[cfg(not(windows))]
        return ensure_launchable(candidate.to_owned());
    }
    Err(missing())
}

pub async fn probe(
    client: LocatedClient,
    connection: String,
    route: String,
) -> Result<ConnectionMetadata> {
    if client.provider_id != "codex" {
        return Err(DomainError::new(
            ErrorCode::CapabilityUnsupported,
            "Client metadata adapter is unavailable",
        ));
    }
    let executable = resolve_executable(client.executable.as_deref().ok_or_else(missing)?)?;
    client.validate_executable()?;
    if client
        .executable_identity
        .as_ref()
        .is_some_and(|identity| identity.resolved != executable)
    {
        return Err(missing());
    }
    let workspace = tempfile::Builder::new()
        .prefix("originlm-codex-metadata-")
        .tempdir()
        .map_err(|_| {
            DomainError::new(
                ErrorCode::LocalAccessDenied,
                "Neutral probe workspace unavailable",
            )
        })?;
    let mut args: Vec<std::ffi::OsString> = vec!["app-server".into(), "--stdio".into()];
    // Invocation-only overrides: do not modify the client's saved settings.
    for feature in [
        "hooks",
        "plugins",
        "remote_plugin",
        "apps",
        "code_mode_host",
        "shell_snapshot",
    ] {
        args.extend(["--disable".into(), feature.into()]);
    }
    let env = BTreeMap::from([("CODEX_HOME".into(), client.profile_home.into_os_string())]);
    codex_metadata(
        LaunchSpec {
            executable,
            args,
            cwd: workspace.path().into(),
            env,
        },
        &connection,
        &route,
        Duration::from_secs(20),
    )
    .await
}

pub async fn managed_metadata(
    spec: LaunchSpec,
    connection: &str,
    route: &str,
    identity: super::ManagedIdentity,
    deadline: Duration,
) -> Result<ConnectionMetadata> {
    codex_managed_metadata(spec, connection, route, identity, deadline).await
}

/// Invocation-only SIWC provider settings. No token or provider setting is
/// written to the user's normal Codex profile.
pub fn managed_launch_spec(
    candidate: &Path,
    profile_home: &Path,
    workspace: &Path,
    access_token: &str,
) -> Result<LaunchSpec> {
    if !profile_home.is_absolute()
        || !workspace.is_absolute()
        || access_token.is_empty()
        || access_token.chars().any(char::is_control)
    {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Invalid managed Codex launch",
        ));
    }
    let executable = resolve_executable(candidate)?;
    let mut args: Vec<std::ffi::OsString> =
        vec!["app-server".into(), "--listen".into(), "stdio://".into()];
    for feature in [
        "hooks",
        "plugins",
        "remote_plugin",
        "apps",
        "code_mode_host",
        "shell_snapshot",
    ] {
        args.extend(["--disable".into(), feature.into()]);
    }
    for setting in [
        "model_provider=\"openai_chatgpt_plan\"",
        "model_providers.openai_chatgpt_plan.name=\"ChatGPT plan\"",
        "model_providers.openai_chatgpt_plan.base_url=\"https://api.openai.com/v1\"",
        "model_providers.openai_chatgpt_plan.env_key=\"ACCESS_TOKEN\"",
        "model_providers.openai_chatgpt_plan.wire_api=\"responses\"",
        "model_providers.openai_chatgpt_plan.requires_openai_auth=false",
        "model_providers.openai_chatgpt_plan.supports_websockets=false",
    ] {
        args.extend(["-c".into(), setting.into()]);
    }
    Ok(LaunchSpec {
        executable,
        args,
        cwd: workspace.into(),
        env: BTreeMap::from([
            ("CODEX_HOME".into(), profile_home.as_os_str().to_owned()),
            ("ACCESS_TOKEN".into(), access_token.into()),
        ]),
    })
}
