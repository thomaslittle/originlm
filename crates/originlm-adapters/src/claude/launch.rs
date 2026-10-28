use crate::process::LaunchSpec;
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
pub fn launch(located: &LocatedClient, cwd: PathBuf) -> Result<LaunchSpec> {
    if located.provider_id != "claude_code" {
        return Err(DomainError::new(
            ErrorCode::CapabilityUnsupported,
            "Unsupported Claude route",
        ));
    }
    let executable = located
        .executable
        .clone()
        .filter(|p| {
            p.is_absolute()
                && p.is_file()
                && (!cfg!(windows) || p.extension().is_some_and(|v| v.eq_ignore_ascii_case("exe")))
        })
        .ok_or_else(|| {
            DomainError::new(
                ErrorCode::RuntimeMissing,
                "Native Claude executable required",
            )
        })?;
    let args = [
        "--safe-mode",
        "--restricted",
        "--tools",
        "",
        "--strict-mcp-config",
        "--no-chrome",
        "--disable-slash-commands",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
        "--no-session-persistence",
        "--print",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--verbose",
    ]
    .iter()
    .map(|s| (*s).into())
    .collect();
    Ok(LaunchSpec {
        executable,
        args,
        cwd,
        env: BTreeMap::from([
            (
                "CLAUDE_CONFIG_DIR".into(),
                located.profile_home.clone().into_os_string(),
            ),
            ("ENABLE_CLAUDEAI_MCP_SERVERS".into(), "false".into()),
            ("CLAUDE_CODE_AUTO_CONNECT_IDE".into(), "0".into()),
        ]),
    })
}
/// A per-session MCP config naming only the host's servers. The bearer token
/// lives in this file (never argv); the returned directory owns it and is
/// deleted when dropped.
pub struct HostMcpLaunch {
    pub spec: LaunchSpec,
    pub config_dir: Option<tempfile::TempDir>,
}
/// The `--mcp-config` document for the host's servers.
pub fn host_mcp_config(servers: &[HostMcpServer]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for server in servers {
        map.insert(
            server.name.clone(),
            serde_json::json!({
                "type": "http",
                "url": server.url,
                "headers": {"Authorization": format!("Bearer {}", server.bearer_token)},
            }),
        );
    }
    serde_json::json!({ "mcpServers": map })
}
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
/// [`launch`], plus the host's MCP servers: `--mcp-config` points at a private
/// per-session file, `--strict-mcp-config` and `--tools ""` stay, and
/// `--allowedTools` names exactly the allowed `mcp__<server>__<tool>` tools.
///
/// `--safe-mode` is dropped because Claude Code 2.1 ignores `--mcp-config`
/// servers in safe mode. `--restricted` (settings files, hooks and plugins
/// ignored), `--disable-slash-commands` and the empty tool list remain, and
/// the client enforces that only the host tools are advertised or called.
pub fn launch_with_host_mcp(
    located: &LocatedClient,
    cwd: PathBuf,
    servers: &[HostMcpServer],
) -> Result<HostMcpLaunch> {
    launch_with_host_mcp_policy(located, cwd, servers, false)
}
/// Host MCP attachment with optional provider-native tools. Native tools are
/// exposed only when the host has granted `native_policy` for this session.
pub fn launch_with_host_mcp_policy(
    located: &LocatedClient,
    cwd: PathBuf,
    servers: &[HostMcpServer],
    native_policy: bool,
) -> Result<HostMcpLaunch> {
    let mut spec = launch(located, cwd)?;
    if !servers.is_empty() || native_policy {
        spec.args.retain(|arg| arg != "--safe-mode");
    }
    if native_policy {
        spec.args.retain(|arg| {
            !matches!(
                arg.to_str(),
                Some(
                    "--restricted"
                        | "--tools"
                        | ""
                        | "--permission-mode"
                        | "dontAsk"
                        | "--permission-prompts"
                        | "none"
                )
            )
        });
        // Route print/SDK permission requests through the stream-json control
        // protocol so the host can mediate the native client's policy.
        spec.args.push("--permission-prompt-tool".into());
        spec.args.push("stdio".into());
    }
    if servers.is_empty() {
        return Ok(HostMcpLaunch {
            spec,
            config_dir: None,
        });
    }
    if !validate_host_mcp_servers(servers) {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Invalid host MCP server",
        ));
    }
    let unavailable = || {
        DomainError::new(
            ErrorCode::ProviderUnavailable,
            "Private MCP configuration unavailable",
        )
    };
    // tempfile creates the directory user-only (0700 on Unix; the per-user
    // temp directory ACL on Windows).
    let dir = tempfile::Builder::new()
        .prefix("originlm-claude-mcp-")
        .tempdir()
        .map_err(|_| unavailable())?;
    let path = dir.path().join("mcp.json");
    let bytes = serde_json::to_vec(&host_mcp_config(servers)).map_err(|_| unavailable())?;
    write_private(&path, &bytes).map_err(|_| unavailable())?;
    let allowed = crate::host_mcp::HostTools::new(servers)
        .qualified_names()
        .join(",");
    spec.args.push("--mcp-config".into());
    spec.args.push(path.into_os_string());
    spec.args.push(format!("--allowedTools={allowed}").into());
    Ok(HostMcpLaunch {
        spec,
        config_dir: Some(dir),
    })
}
/// A new random (v4) UUID for `--session-id`.
pub fn new_session_id() -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| {
        DomainError::new(
            ErrorCode::ProviderUnavailable,
            "Secure session id generation failed",
        )
    })?;
    b[6] = (b[6] & 0x0f) | 0x40;
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
/// Lowercase canonical UUID text, the only form the kit passes to Claude Code.
pub fn valid_session_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}
/// Persisted conversation: drop `--no-session-persistence` and either name the
/// new session (`--session-id`) or reattach one (`--resume`, which keeps the id).
pub fn persist(spec: &mut LaunchSpec, session_id: &str, resume: bool) -> Result<()> {
    if !valid_session_id(session_id) {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Invalid native session id",
        ));
    }
    spec.args.retain(|arg| arg != "--no-session-persistence");
    spec.args
        .push(if resume { "--resume" } else { "--session-id" }.into());
    spec.args.push(session_id.into());
    Ok(())
}
/// Whether Claude Code stored this conversation under the profile
/// (`<profile>/projects/<project>/<id>.jsonl`). Checked before a resume so a
/// missing conversation is refused instead of silently starting a new one.
pub fn persisted_session_exists(profile_home: &Path, session_id: &str) -> bool {
    if !valid_session_id(session_id) {
        return false;
    }
    let Ok(projects) = std::fs::read_dir(profile_home.join("projects")) else {
        return false;
    };
    projects.take(10_000).flatten().any(|entry| {
        entry
            .path()
            .join(format!("{session_id}.jsonl"))
            .symlink_metadata()
            .is_ok_and(|meta| meta.is_file())
    })
}
pub async fn probe(
    located: LocatedClient,
    connection: String,
    route: String,
) -> Result<ConnectionMetadata> {
    let dir = tempfile::tempdir().map_err(|_| {
        DomainError::new(
            ErrorCode::ProviderUnavailable,
            "Isolated workspace unavailable",
        )
    })?;
    super::client::probe_spec(launch(&located, dir.path().into())?, &connection, &route).await
}
