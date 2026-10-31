use super::config::{NativeProfile, NativeProvider, local_path};
use crate::process::LaunchSpec;
use originlm_contracts::*;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
};
/// Grok flags that remove every native tool for text-only sessions.
pub const GROK_TEXT_ONLY_ARGS: [&str; 4] = [
    "--tools=",
    "--disable-web-search",
    "--permission-mode=dontAsk",
    "--no-subagents",
];
/// Process-scoped switch Grok documents for suppressing its CLI updater.
///
/// Reimplemented from T3 Code `packages/provider-grok/src/server/probeEnvironment.ts`
/// at <https://github.com/pingdotgg/t3code/commit/2a7bd1673c7bdad4304b7771944e73c29ecd3c01>
/// (MIT, Copyright (c) 2026 T3 Tools Inc.). No upstream source is copied.
pub fn apply_grok_probe_environment(env: &mut BTreeMap<OsString, OsString>) {
    env.insert("GROK_DISABLE_AUTOUPDATER".into(), "1".into());
}
pub struct PreparedLaunch {
    pub spec: LaunchSpec,
    pub scratch: tempfile::TempDir,
    pub auth_method: Option<String>,
}
fn error(code: ErrorCode) -> DomainError {
    DomainError::new(
        code,
        "A compatible native client, profile, or browser-blocking helper is unavailable",
    )
}
fn file(path: &Path) -> bool {
    path.is_absolute() && path.metadata().is_ok_and(|m| m.is_file())
}
fn name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
fn cached_auth(profile: &Path) -> Option<String> {
    let path = profile.join("settings.json");
    if path.symlink_metadata().ok()?.file_type().is_symlink() {
        return None;
    }
    let f = std::fs::File::open(path).ok()?;
    if !f.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    f.take(65537).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 65536 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value["auth"]["type"]
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
        .map(str::to_owned)
}
fn browser_command_any(path: &Path) -> Result<OsString> {
    if !file(path) {
        return Err(error(ErrorCode::IntegrationRestricted));
    }
    let text = path
        .to_str()
        .ok_or_else(|| error(ErrorCode::IntegrationRestricted))?;
    if text.contains(['"', '%', ';']) || text.chars().any(char::is_control) {
        return Err(error(ErrorCode::IntegrationRestricted));
    }
    Ok(format!("\"{text}\" %s").into())
}
fn browser_command(path: &Path) -> Result<OsString> {
    if !file(path) || !name(path).starts_with("originlm-auth-blocker") {
        return Err(error(ErrorCode::IntegrationRestricted));
    }
    let text = path
        .to_str()
        .ok_or_else(|| error(ErrorCode::IntegrationRestricted))?;
    if text.contains(['"', '%', ';'])
        || (!cfg!(windows) && text.contains(':'))
        || text.chars().any(char::is_control)
    {
        return Err(error(ErrorCode::IntegrationRestricted));
    }
    Ok(format!("\"{text}\" %s").into())
}
fn node() -> Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or_else(|| error(ErrorCode::RuntimeMissing))?;
    for root in std::env::split_paths(&path)
        .take(128)
        .filter(|p| local_path(p))
    {
        let candidate = root.join(if cfg!(windows) { "node.exe" } else { "node" });
        if file(&candidate) {
            return Ok(candidate);
        }
    }
    Err(error(ErrorCode::RuntimeMissing))
}
fn gemini_executable(candidate: &Path) -> Result<(PathBuf, Vec<OsString>)> {
    let label = name(candidate);
    if ![
        "gemini",
        "gemini.cmd",
        "gemini.ps1",
        "gemini.exe",
        "index.js",
    ]
    .contains(&label.as_str())
    {
        return Err(error(ErrorCode::RuntimeMissing));
    }
    if label == "gemini.exe" && file(candidate) {
        return Ok((candidate.into(), vec![]));
    }
    let parent = candidate
        .parent()
        .ok_or_else(|| error(ErrorCode::RuntimeMissing))?;
    let mut entries = vec![
        parent.join("node_modules/@google/gemini-cli/dist/index.js"),
        parent.join("../lib/node_modules/@google/gemini-cli/dist/index.js"),
        parent.join("node_modules/@google/gemini-cli/bundle/gemini.js"),
        parent.join("../lib/node_modules/@google/gemini-cli/bundle/gemini.js"),
    ];
    if let Ok(real) = candidate.canonicalize()
        && matches!(name(&real).as_str(), "index.js" | "gemini.js")
    {
        entries.push(real);
    }
    for entry in entries {
        let Some(package) = entry.parent().and_then(Path::parent) else {
            continue;
        };
        if name(package) != "gemini-cli"
            || package.parent().is_none_or(|p| name(p) != "@google")
            || !file(&entry)
        {
            continue;
        }
        let manifest = package.join("package.json");
        let Ok(f) = std::fs::File::open(manifest) else {
            continue;
        };
        let mut bytes = Vec::new();
        if f.take(65537).read_to_end(&mut bytes).is_err() || bytes.len() > 65536 {
            continue;
        }
        if serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .is_some_and(|v| v["name"] == "@google/gemini-cli")
        {
            return Ok((node()?, vec![entry.into_os_string()]));
        }
    }
    // A directly installed Unix native/shebang entry is an explicit trusted client, never a shell command string.
    if !cfg!(windows) && label == "gemini" && file(candidate) {
        return Ok((candidate.into(), vec![]));
    }
    Err(error(ErrorCode::RuntimeMissing))
}
pub fn prepare(
    profile: &NativeProfile,
    workspace: &Path,
    browser: Option<&Path>,
) -> Result<PreparedLaunch> {
    prepare_for(profile, workspace, browser, true, false)
}
pub fn prepare_execution(
    profile: &NativeProfile,
    workspace: &Path,
    browser: Option<&Path>,
) -> Result<PreparedLaunch> {
    prepare_for(profile, workspace, browser, false, false)
}
/// Prepare an interactive sign-in launch. `browser` is a capture script that
/// receives the authorization URL; it must not be the metadata blocker.
pub fn prepare_auth(
    profile: &NativeProfile,
    workspace: &Path,
    browser: &Path,
) -> Result<PreparedLaunch> {
    prepare_for(profile, workspace, Some(browser), true, true)
}
fn prepare_for(
    profile: &NativeProfile,
    workspace: &Path,
    browser: Option<&Path>,
    metadata_probe: bool,
    auth: bool,
) -> Result<PreparedLaunch> {
    if !local_path(&profile.executable)
        || !local_path(&profile.profile_home)
        || !workspace.is_absolute()
        || !workspace.is_dir()
    {
        return Err(error(ErrorCode::IntegrationRestricted));
    }
    let scratch = tempfile::Builder::new()
        .prefix("originlm-native-")
        .tempdir()
        .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
    let mut env = BTreeMap::<OsString, OsString>::new();
    let mut auth_method = None;
    let (executable, args) = match profile.provider {
        NativeProvider::GeminiCli => {
            let (executable, mut args) = gemini_executable(&profile.executable)?;
            args.push("--acp".into());
            env.insert(
                "GEMINI_CLI_HOME".into(),
                profile.profile_home.clone().into_os_string(),
            );
            if auth {
                env.insert(
                    "BROWSER".into(),
                    browser_command_any(
                        browser.ok_or_else(|| error(ErrorCode::IntegrationRestricted))?,
                    )?,
                );
            } else {
                env.insert("NO_BROWSER".into(), "true".into());
                if let Some(browser) = browser {
                    env.insert("BROWSER".into(), browser_command(browser)?);
                }
            }
            (executable, args)
        }
        NativeProvider::Antigravity => {
            let expected = if cfg!(windows) {
                "agy_acp_server.exe"
            } else {
                "agy_acp_server.par"
            };
            if name(&profile.executable) != expected || !file(&profile.executable) {
                return Err(error(ErrorCode::RuntimeMissing));
            }
            let harness = profile
                .executable
                .parent()
                .ok_or_else(|| error(ErrorCode::RuntimeMissing))?
                .join(if cfg!(windows) {
                    "localharness_external.exe"
                } else {
                    "localharness_external"
                });
            if !file(&harness) {
                return Err(error(ErrorCode::RuntimeMissing));
            }
            if name(&profile.profile_home) != "antigravity-acp" {
                return Err(error(ErrorCode::IntegrationRestricted));
            }
            if auth {
                env.insert(
                    "BROWSER".into(),
                    browser_command_any(
                        browser.ok_or_else(|| error(ErrorCode::IntegrationRestricted))?,
                    )?,
                );
            } else {
                env.insert(
                    "BROWSER".into(),
                    browser_command(
                        browser.ok_or_else(|| error(ErrorCode::IntegrationRestricted))?,
                    )?,
                );
            }
            env.insert(
                "GEMINI_HOME".into(),
                profile
                    .profile_home
                    .parent()
                    .ok_or_else(|| error(ErrorCode::IntegrationRestricted))?
                    .as_os_str()
                    .into(),
            );
            env.insert("ANTIGRAVITY_HARNESS_PATH".into(), harness.into_os_string());
            env.insert("PYTHONUNBUFFERED".into(), "1".into());
            env.insert("ELECTRON_RUN_AS_NODE".into(), "1".into());
            // Metadata probes must not start an interactive OAuth flow. Use
            // only a host-declared method or one already recorded by the
            // provider; otherwise the probe reports AUTH_REQUIRED and the host
            // can start sign-in deliberately.
            auth_method = if auth {
                Some(
                    profile
                        .auth_method
                        .clone()
                        .or_else(|| cached_auth(&profile.profile_home))
                        .unwrap_or_else(|| "oauth-personal".into()),
                )
            } else {
                profile
                    .auth_method
                    .clone()
                    .or_else(|| cached_auth(&profile.profile_home))
            };
            (
                profile.executable.clone(),
                if cfg!(target_os = "linux") {
                    vec!["--uid=".into()]
                } else {
                    vec![]
                },
            )
        }
        NativeProvider::Grok => {
            let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(PathBuf::from)
                .ok_or_else(|| error(ErrorCode::IntegrationRestricted))?;
            if profile.profile_home != home.join(".grok") {
                return Err(error(ErrorCode::IntegrationRestricted));
            }
            if name(&profile.executable) != if cfg!(windows) { "grok.exe" } else { "grok" }
                || !file(&profile.executable)
            {
                return Err(error(ErrorCode::RuntimeMissing));
            }
            let mut args = vec![
                "--leader-socket".into(),
                scratch.path().join("leader.sock").into_os_string(),
            ];
            // Metadata probes and text sessions alike run with no built-in tools,
            // no web search/fetch, no subagents, and a deny-by-default
            // permission mode. Host design tools are executed by the host from
            // the model's text; Grok's own tools must never run.
            args.extend(GROK_TEXT_ONLY_ARGS.iter().map(OsString::from));
            args.extend(["agent".into(), "stdio".into()]);
            // Health probes must not start Grok's updater. Text sessions leave
            // the variable unset so an update still follows the user's configuration.
            if metadata_probe {
                apply_grok_probe_environment(&mut env);
            }
            (profile.executable.clone(), args)
        }
    };
    if cfg!(windows) {
        env.insert("TEMP".into(), scratch.path().as_os_str().into());
        env.insert("TMP".into(), scratch.path().as_os_str().into());
    } else {
        env.insert("TMPDIR".into(), scratch.path().as_os_str().into());
    }
    Ok(PreparedLaunch {
        spec: LaunchSpec {
            executable,
            args,
            cwd: workspace.into(),
            env,
        },
        scratch,
        auth_method,
    })
}
