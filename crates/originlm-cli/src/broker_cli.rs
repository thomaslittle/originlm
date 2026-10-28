use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_service::{ExecutionGrant, NativeService};
use originlm_transports::local_ipc::{Broker, BrokerOptions};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
fn err() -> DomainError {
    DomainError::new(
        ErrorCode::ProviderUnavailable,
        "Broker control channel closed",
    )
}
fn invalid(message: &str) -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, message)
}

#[derive(Debug, Clone)]
pub enum Lifetime {
    /// The trusted parent owns the broker. Closing its control input stops the
    /// service, which is the historical embedded/managed-helper behavior.
    Parent,
    /// The broker owns its own lifetime. Applications may come and go without
    /// terminating it, and a second launch converges on the existing listener.
    Independent {
        /// Legacy owner credential. Optional when `grants_file` supplies
        /// distinct per-application credentials.
        token_file: Option<PathBuf>,
        admin_token_file: Option<PathBuf>,
        /// Owner-only manifest of independently scoped application grants.
        grants_file: Option<PathBuf>,
        idle_timeout: Option<Duration>,
    },
}

/// Independent-mode grant manifest. It contains scopes, never secrets; the
/// broker mints one credential per application and writes it to that
/// application's owner-only token file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GrantsFile {
    schema_version: u32,
    apps: Vec<AppGrantSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppGrantSpec {
    id: String,
    token_file: PathBuf,
    #[serde(default)]
    allow_passive_discovery: bool,
    /// Missing means every connection; an explicit empty list means none.
    #[serde(default)]
    connection_ids: Option<BTreeSet<String>>,
    #[serde(default)]
    reveal_identity: bool,
    #[serde(default)]
    probe_connection_ids: BTreeSet<String>,
    #[serde(default)]
    probe_provider_ids: BTreeSet<String>,
    #[serde(default)]
    execution: Option<ExecutionGrant>,
}

fn valid_app_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id != "ipc-host"
        && id != "kit-admin"
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn valid_scope_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}

fn absolute_regular_file(path: &Path) -> Result<()> {
    let text = path.to_string_lossy();
    if !path.is_absolute() || text.starts_with(r"\\") || text.starts_with("//") {
        return Err(invalid("Grant file paths must be absolute local paths"));
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| invalid("Grant file is unavailable"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("Grant file must be a regular file"));
    }
    Ok(())
}

fn load_grants(path: &Path) -> Result<GrantsFile> {
    absolute_regular_file(path)?;
    let bytes = std::fs::read(path).map_err(|_| invalid("Grant file could not be read"))?;
    if bytes.len() > 64 * 1024 {
        return Err(invalid("Grant file is too large"));
    }
    let grants: GrantsFile =
        serde_json::from_slice(&bytes).map_err(|_| invalid("Grant file is not valid JSON"))?;
    if grants.schema_version != 1 || grants.apps.is_empty() || grants.apps.len() > 32 {
        return Err(invalid(
            "Grant file schema or application count is unsupported",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut token_files = BTreeSet::new();
    for app in &grants.apps {
        if !valid_app_id(&app.id) || !ids.insert(app.id.clone()) {
            return Err(invalid("Grant application ids must be unique and local"));
        }
        if !app.token_file.is_absolute() || !token_files.insert(app.token_file.clone()) {
            return Err(invalid("Grant token files must be absolute and unique"));
        }
        if let Some(connections) = &app.connection_ids
            && (connections.len() > 64 || !connections.iter().all(|id| valid_scope_id(id)))
        {
            return Err(invalid("Grant connection ids are invalid"));
        }
        if app.probe_connection_ids.len() > 64
            || app.probe_provider_ids.len() > 64
            || !app
                .probe_connection_ids
                .iter()
                .chain(app.probe_provider_ids.iter())
                .all(|id| valid_scope_id(id))
        {
            return Err(invalid("Grant probe ids are invalid"));
        }
        if let Some(execution) = &app.execution {
            if execution.workspaces.is_empty()
                || execution.workspaces.len() > 16
                || !execution
                    .workspaces
                    .iter()
                    .all(|(id, path)| valid_scope_id(id) && path.is_absolute())
            {
                return Err(invalid("Grant execution workspaces are invalid"));
            }
            if execution.connection_ids.len() > 64
                || execution.provider_ids.len() > 64
                || !execution
                    .connection_ids
                    .iter()
                    .chain(execution.provider_ids.iter())
                    .all(|id| valid_scope_id(id))
            {
                return Err(invalid("Grant execution ids are invalid"));
            }
        }
    }
    Ok(grants)
}

fn app_policy(app: &AppGrantSpec) -> ClientPolicy {
    ClientPolicy {
        allow_passive_discovery: app.allow_passive_discovery,
        connection_ids: app.connection_ids.clone(),
        reveal_identity: app.reveal_identity,
        probe_connection_ids: app.probe_connection_ids.clone(),
        probe_provider_ids: app.probe_provider_ids.clone(),
        admin: false,
    }
}

#[derive(Debug, Clone)]
struct IndependentFiles {
    token_file: Option<PathBuf>,
    admin_token_file: Option<PathBuf>,
    grants_file: Option<PathBuf>,
    idle_timeout: Option<Duration>,
}

fn grant_paths(
    token_file: &Option<PathBuf>,
    admin_token_file: &Option<PathBuf>,
    grants_file: &Option<PathBuf>,
    grants: Option<&GrantsFile>,
) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(path) = token_file {
        paths.push(path.clone());
    }
    if let Some(path) = admin_token_file {
        paths.push(path.clone());
    }
    if let Some(grants) = grants {
        paths.extend(grants.apps.iter().map(|app| app.token_file.clone()));
    }
    let mut unique = BTreeSet::new();
    if paths
        .iter()
        .any(|path| !path.is_absolute() || !unique.insert(path.clone()))
    {
        return Err(invalid("Broker token files must be absolute and unique"));
    }
    if let Some(grants_file) = grants_file
        && unique.contains(grants_file)
    {
        return Err(invalid(
            "A broker token file cannot overwrite the grant file",
        ));
    }
    Ok(paths)
}

fn private_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            DomainError::new(
                ErrorCode::StorageUnavailable,
                "Broker token directory is unavailable",
            )
        })?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&temporary)
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(bytes)
        })
        .map_err(|_| {
            DomainError::new(
                ErrorCode::StorageUnavailable,
                "Broker token file could not be written",
            )
        })?;
    #[cfg(windows)]
    let _ = std::fs::remove_file(path);
    std::fs::rename(&temporary, path).map_err(|_| {
        DomainError::new(
            ErrorCode::StorageUnavailable,
            "Broker token file could not be published",
        )
    })?;
    #[cfg(windows)]
    {
        // The file contains an owner credential. Remove inherited ACEs and
        // grant only the current user. Fail closed if the ACL cannot be set.
        let user = std::env::var("USERNAME").map_err(|_| {
            DomainError::new(
                ErrorCode::LocalAccessDenied,
                "Broker token owner is unavailable",
            )
        })?;
        let status = std::process::Command::new("icacls")
            .args([
                path.as_os_str(),
                std::ffi::OsStr::new("/inheritance:r"),
                std::ffi::OsStr::new("/grant:r"),
                std::ffi::OsStr::new(&format!("{user}:F")),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|_| {
                DomainError::new(
                    ErrorCode::StorageUnavailable,
                    "Broker token ACL could not be set",
                )
            })?;
        if !status.success() {
            let _ = std::fs::remove_file(path);
            return Err(DomainError::new(
                ErrorCode::LocalAccessDenied,
                "Broker token ACL could not be restricted to the owner",
            ));
        }
    }
    Ok(())
}

fn descriptor(
    broker: &Broker,
    address: &str,
    policy: ClientPolicy,
    execution: Option<ExecutionGrant>,
    reusable: bool,
) -> Result<serde_json::Value> {
    let token = if reusable {
        broker.authorize_shared(policy, execution)?
    } else {
        broker.authorize(policy, execution)?
    };
    Ok(serde_json::json!({
        "protocolVersion": "1.0",
        "address": address,
        "token": token,
        "expiresInSeconds": if reusable { 0 } else { 300 },
        "independent": reusable,
        "scope": if reusable { "owner" } else { "single-use" },
        "pid": std::process::id(),
    }))
}

fn app_descriptor(
    broker: &Broker,
    address: &str,
    app_id: &str,
    policy: ClientPolicy,
    execution: Option<ExecutionGrant>,
) -> Result<serde_json::Value> {
    let token = broker.authorize_shared_for(app_id, policy, execution)?;
    Ok(serde_json::json!({
        "protocolVersion": "1.0",
        "address": address,
        "token": token,
        "expiresInSeconds": 0,
        "independent": true,
        "appId": app_id,
        "scope": "application",
        "pid": std::process::id(),
    }))
}

pub async fn connect(address: &str) -> Result<()> {
    let stream = originlm_transports::local_ipc::connect(address).await?;
    let (mut r, mut w) = tokio::io::split(stream);
    let (mut input, mut output) = (tokio::io::stdin(), tokio::io::stdout());
    tokio::select! {
     result=async {tokio::io::copy(&mut input,&mut w).await?;w.shutdown().await}=>{result.map_err(|_|err())?;},
     result=tokio::io::copy(&mut r,&mut output)=>{result.map_err(|_|err())?;}
    }
    Ok(())
}

pub async fn serve(
    service: Arc<NativeService>,
    address: &str,
    policy: ClientPolicy,
    execution: Option<ExecutionGrant>,
    lifetime: Lifetime,
) -> Result<()> {
    match lifetime {
        Lifetime::Parent => serve_parent(service, address, policy, execution).await,
        Lifetime::Independent {
            token_file,
            admin_token_file,
            grants_file,
            idle_timeout,
        } => {
            serve_independent(
                service,
                address,
                policy,
                execution,
                IndependentFiles {
                    token_file,
                    admin_token_file,
                    grants_file,
                    idle_timeout,
                },
            )
            .await
        }
    }
}

async fn serve_parent(
    service: Arc<NativeService>,
    address: &str,
    policy: ClientPolicy,
    execution: Option<ExecutionGrant>,
) -> Result<()> {
    let broker = Broker::start(service, address).await?;
    // This stdout is a trusted parent control channel, not a diagnostic log.
    // Each bootstrap is single-use. No provider credential is involved.
    let send = || -> Result<()> {
        println!(
            "{}",
            descriptor(&broker, address, policy.clone(), execution.clone(), false)?
        );
        Ok(())
    };
    send()?;
    let mut reader = BufReader::new(tokio::io::stdin());
    loop {
        let mut bytes = Vec::new();
        let n = (&mut reader)
            .take(1025)
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|_| err())?;
        if n == 0 {
            break;
        }
        if n > 1024 || !bytes.ends_with(b"\n") {
            break;
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        if value == serde_json::json!({"action":"authorize"}) {
            send()?;
        } else {
            break;
        }
    }
    broker.close().await;
    Ok(())
}

async fn serve_independent(
    service: Arc<NativeService>,
    address: &str,
    policy: ClientPolicy,
    execution: Option<ExecutionGrant>,
    files: IndependentFiles,
) -> Result<()> {
    let IndependentFiles {
        token_file,
        admin_token_file,
        grants_file,
        idle_timeout,
    } = files;
    if idle_timeout.is_some_and(|timeout| timeout < Duration::from_secs(5)) {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "Idle timeout must be at least five seconds",
        ));
    }
    let grants = grants_file.as_deref().map(load_grants).transpose()?;
    if token_file.is_none() && grants.is_none() {
        return Err(invalid(
            "An independent broker needs at least one token file or grant manifest",
        ));
    }
    let written_paths = grant_paths(
        &token_file,
        &admin_token_file,
        &grants_file,
        grants.as_ref(),
    )?;
    // Endpoint binding is the race-free single-instance boundary. A second
    // launch gets Conflict and must connect to the existing service instead of
    // replacing it.
    let broker = Broker::start_with(service, address, BrokerOptions { idle_timeout: None }).await?;
    let mut published: Vec<PathBuf> = Vec::new();
    let result = (|| -> Result<Option<serde_json::Value>> {
        let owner_value = if let Some(token_file) = &token_file {
            let value = descriptor(&broker, address, policy.clone(), execution.clone(), true)?;
            let bytes = serde_json::to_vec(&value).map_err(|_| err())?;
            private_write(token_file, &bytes)?;
            published.push(token_file.clone());
            Some(value)
        } else {
            None
        };
        if let Some(admin_file) = &admin_token_file {
            let mut admin_policy = policy.clone();
            admin_policy.admin = true;
            let value = app_descriptor(
                &broker,
                address,
                "kit-admin",
                admin_policy,
                execution.clone(),
            )?;
            let bytes = serde_json::to_vec(&value).map_err(|_| err())?;
            private_write(admin_file, &bytes)?;
            published.push(admin_file.clone());
        }
        if let Some(grants) = &grants {
            for app in &grants.apps {
                let value = app_descriptor(
                    &broker,
                    address,
                    &app.id,
                    app_policy(app),
                    app.execution.clone(),
                )?;
                let bytes = serde_json::to_vec(&value).map_err(|_| err())?;
                private_write(&app.token_file, &bytes)?;
                published.push(app.token_file.clone());
            }
        }
        Ok(owner_value)
    })();
    let owner_value = match result {
        Ok(value) => value,
        Err(error) => {
            for path in &published {
                let _ = std::fs::remove_file(path);
            }
            broker.close().await;
            return Err(error);
        }
    };
    let summary = owner_value.unwrap_or_else(|| {
        let apps: Vec<serde_json::Value> = grants
            .as_ref()
            .map(|grants| {
                grants
                    .apps
                    .iter()
                    .map(|app| {
                        serde_json::json!({
                            "id": app.id,
                            "tokenFile": app.token_file,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        serde_json::json!({
            "protocolVersion": "1.0",
            "address": address,
            "independent": true,
            "scope": "applications",
            "apps": apps,
            "pid": std::process::id(),
        })
    });
    println!("{summary}");
    let started = std::time::Instant::now();
    let mut idle_since = std::time::Instant::now();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tick.tick() => {
                if broker.active_connections() == 0 {
                    if idle_timeout.is_some_and(|timeout| idle_since.elapsed() >= timeout) {
                        break;
                    }
                } else {
                    idle_since = std::time::Instant::now();
                }
                // A clock jump backwards must not extend the lease forever.
                if idle_timeout.is_some_and(|timeout| started.elapsed() > timeout.saturating_mul(2) && broker.active_connections() == 0) {
                    break;
                }
            }
        }
    }
    broker.close().await;
    for path in &written_paths {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}
