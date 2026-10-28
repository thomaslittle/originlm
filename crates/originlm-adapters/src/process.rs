//! Bounded native protocol worker. The trusted host supplies the executable; no shell is used.
use originlm_contracts::{ConnectionMetadata, DomainError, ErrorCode, Result};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::PathBuf,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
    task::JoinHandle,
};

const MAX_FRAME: u64 = 1_048_576;
#[derive(Clone)]
pub struct LaunchSpec {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: BTreeMap<OsString, OsString>,
}
pub(crate) struct Peer {
    child: Box<dyn process_wrap::tokio::ChildWrapper>,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    stderr: JoinHandle<()>,
    pub(crate) sequence: u64,
    account_established: bool,
    account_stamp: Option<String>,
    /// An `account/updated` arrived since the identity was last confirmed.
    pub(crate) account_update_seen: bool,
    managed: bool,
}
fn error(code: ErrorCode) -> DomainError {
    DomainError::new(code, "Native client metadata request failed")
}
fn validate(spec: &LaunchSpec) -> Result<()> {
    let extension = spec
        .executable
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !spec.executable.is_absolute()
        || !spec.cwd.is_absolute()
        || ["cmd", "bat", "ps1"].contains(&extension.as_str())
        || spec.args.len() > 128
    {
        return Err(error(ErrorCode::InvalidRequest));
    }
    if spec
        .args
        .iter()
        .any(|v| v.len() > 32768 || v.as_encoded_bytes().contains(&0))
    {
        return Err(error(ErrorCode::InvalidRequest));
    }
    for (key, value) in &spec.env {
        let name = key
            .to_str()
            .ok_or_else(|| error(ErrorCode::InvalidRequest))?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || value.as_encoded_bytes().contains(&0)
            || [
                "NODE_OPTIONS",
                "LD_PRELOAD",
                "DYLD_INSERT_LIBRARIES",
                "PYTHONPATH",
                "PYTHONHOME",
                "PYTHONSTARTUP",
            ]
            .contains(&name.to_ascii_uppercase().as_str())
        {
            return Err(error(ErrorCode::InvalidRequest));
        }
    }
    Ok(())
}
/// A validated, env-scrubbed, tree-killing command. `stdio` pipes all three
/// streams for protocol peers; otherwise every stream is closed.
fn wrapped_command(spec: LaunchSpec, stdio: bool) -> Result<process_wrap::tokio::CommandWrap> {
    validate(&spec)?;
    #[cfg(unix)]
    let identity = originlm_discovery::resolve_executable(&spec.executable)?;
    #[cfg(unix)]
    let executable = &identity.resolved;
    #[cfg(not(unix))]
    let executable = &spec.executable;
    let mut command = Command::new(executable);
    command.args(spec.args).current_dir(spec.cwd).env_clear();
    if stdio {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    } else {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    for key in [
        "PATH",
        "Path",
        "SystemRoot",
        "SYSTEMROOT",
        "WINDIR",
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "TMP",
        "TEMP",
        "TMPDIR",
        "LANG",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.envs(spec.env);
    let mut command = process_wrap::tokio::CommandWrap::from(command);
    command.wrap(process_wrap::tokio::KillOnDrop);
    #[cfg(windows)]
    {
        command
            .wrap(process_wrap::tokio::CreationFlags(
                windows::Win32::System::Threading::CREATE_NO_WINDOW,
            ))
            .wrap(process_wrap::tokio::JobObject);
    }
    #[cfg(unix)]
    {
        command.wrap(process_wrap::tokio::ProcessGroup::leader());
    }
    #[cfg(unix)]
    identity.validate()?;
    Ok(command)
}
/// Stable identity of a Codex `account/read` account: the auth kind and the
/// account email, as T3 Code keys its Codex account. Volatile fields (plan
/// type, routing, managed-credential flags, anything a token refresh can
/// rewrite) are excluded so a long-running helper does not report a changed
/// selection while the user is still on the same account. `None` for a
/// signed-out (null) account.
pub(crate) fn account_identity_stamp(account: &Value) -> Option<String> {
    let kind = account["type"].as_str()?;
    let email = account["email"].as_str().unwrap_or("");
    Some(originlm_contracts::stable_id(
        "account",
        &[kind, &email.to_ascii_lowercase()],
    ))
}
/// `account/updated` carries only `authMode`/`planType`. A token refresh or
/// plan-label change keeps the session; a null auth mode is a sign-out. Any
/// real account switch is caught by the identity re-read before the next turn.
pub(crate) fn account_update_signs_out(params: &Value) -> bool {
    params.get("authMode").is_some_and(Value::is_null)
}
/// A background native server whose whole process tree dies with this value.
pub(crate) struct ManagedChild(Box<dyn process_wrap::tokio::ChildWrapper>);
impl ManagedChild {
    pub(crate) fn spawn(spec: LaunchSpec) -> Result<Self> {
        wrapped_command(spec, false)?
            .spawn()
            .map(Self)
            .map_err(|_| error(ErrorCode::RuntimeMissing))
    }
    pub(crate) fn exited(&mut self) -> bool {
        !matches!(self.0.try_wait(), Ok(None))
    }
    pub(crate) async fn shutdown(&mut self) {
        let _ = self.0.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(2), self.0.wait()).await;
    }
}
impl Peer {
    pub(crate) async fn spawn(spec: LaunchSpec) -> Result<Self> {
        let mut command = wrapped_command(spec, true)?;
        let mut child = command
            .spawn()
            .map_err(|_| error(ErrorCode::RuntimeMissing))?;
        let input = child
            .stdin()
            .take()
            .ok_or_else(|| error(ErrorCode::ProviderUnavailable))?;
        let output = BufReader::new(
            child
                .stdout()
                .take()
                .ok_or_else(|| error(ErrorCode::ProviderUnavailable))?,
        );
        let mut stderr = child
            .stderr()
            .take()
            .ok_or_else(|| error(ErrorCode::ProviderUnavailable))?;
        let drain = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
        });
        Ok(Self {
            child,
            input,
            output,
            stderr: drain,
            sequence: 0,
            account_established: false,
            account_stamp: None,
            account_update_seen: false,
            managed: false,
        })
    }
    pub(crate) async fn verify_account(&mut self) -> Result<()> {
        // The managed provider's identity comes from a verified SIWC token,
        // checked by its credential owner before each turn. Codex has no
        // native account in this configuration.
        if self.managed {
            return Ok(());
        }
        let current = self
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        if current["account"].is_null()
            || self.account_stamp.is_none()
            || self.account_stamp != account_identity_stamp(&current["account"])
        {
            return Err(error(ErrorCode::SelectionStale));
        }
        self.account_update_seen = false;
        Ok(())
    }
    pub(crate) async fn send(&mut self, value: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&value).map_err(|_| error(ErrorCode::InvalidRequest))?;
        if bytes.len() as u64 >= MAX_FRAME {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        bytes.push(b'\n');
        self.input
            .write_all(&bytes)
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        self.input
            .flush()
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))
    }
    /// Like [`Self::send`] with a caller-chosen frame cap, for protocols whose
    /// requests legitimately carry inline images.
    pub(crate) async fn send_bounded(&mut self, value: Value, max_frame: usize) -> Result<()> {
        let mut bytes = serde_json::to_vec(&value).map_err(|_| error(ErrorCode::InvalidRequest))?;
        if bytes.len() >= max_frame {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        bytes.push(b'\n');
        self.input
            .write_all(&bytes)
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        self.input
            .flush()
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))
    }
    /// A cancel-safe [`Self::read`]: partial bytes stay in `pending`, owned by
    /// the caller, so the read can be raced in `select!` and resumed later.
    pub(crate) async fn read_buffered(&mut self, pending: &mut Vec<u8>) -> Result<Value> {
        let room = (MAX_FRAME + 1).saturating_sub(pending.len() as u64);
        let count = (&mut self.output)
            .take(room)
            .read_until(b'\n', pending)
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        if pending.last() != Some(&b'\n') {
            if count == 0 {
                return Err(error(ErrorCode::ProviderUnavailable));
            }
            return Err(error(if pending.len() as u64 > MAX_FRAME {
                ErrorCode::FrameTooLarge
            } else {
                ErrorCode::ProtocolIncompatible
            }));
        }
        let frame = std::mem::take(pending);
        serde_json::from_slice(&frame).map_err(|_| error(ErrorCode::ProtocolIncompatible))
    }
    pub(crate) async fn read(&mut self) -> Result<Value> {
        let mut bytes = Vec::new();
        let count = (&mut self.output)
            .take(MAX_FRAME + 1)
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        if count == 0 {
            return Err(error(ErrorCode::ProviderUnavailable));
        }
        if count as u64 > MAX_FRAME {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        if bytes.last() != Some(&b'\n') {
            return Err(error(ErrorCode::ProtocolIncompatible));
        }
        serde_json::from_slice(&bytes).map_err(|_| error(ErrorCode::ProtocolIncompatible))
    }
    pub(crate) async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.sequence += 1;
        let id = self.sequence;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        for _ in 0..512 {
            let message = self.read().await?;
            if !message.is_object() {
                return Err(error(ErrorCode::ProtocolIncompatible));
            }
            if message.get("method").is_some() {
                if message.get("id").is_some() {
                    return Err(error(ErrorCode::IntegrationRestricted));
                }
                if message["method"] == "account/updated" && self.account_established {
                    if !self.managed && account_update_signs_out(&message["params"]) {
                        return Err(error(ErrorCode::SelectionStale));
                    }
                    self.account_update_seen = true;
                }
                continue;
            }
            if message["id"] != id {
                return Err(error(ErrorCode::ProtocolIncompatible));
            }
            match (message.get("result"), message.get("error")) {
                (Some(result), None) => return Ok(result.clone()),
                (None, Some(response_error)) => {
                    let detail = response_error["message"]
                        .as_str()
                        .filter(|message| !message.trim().is_empty())
                        .map(originlm_contracts::redact_diagnostic_detail)
                        .filter(|message| !message.is_empty())
                        .unwrap_or_else(|| "Codex app-server rejected the request".into());
                    return Err(DomainError::new(
                        ErrorCode::ProviderUnavailable,
                        format!("Codex app-server: {detail}"),
                    ));
                }
                _ => return Err(error(ErrorCode::ProtocolIncompatible)),
            }
        }
        Err(error(ErrorCode::FrameTooLarge))
    }
    pub(crate) async fn shutdown(&mut self) {
        let _ = self.input.shutdown().await;
        if tokio::time::timeout(Duration::from_millis(250), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
            let _ = self.child.wait().await;
        }
        self.stderr.abort();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stderr.abort();
        let _ = self.child.start_kill();
    }
}

pub async fn codex_metadata(
    spec: LaunchSpec,
    connection: &str,
    route: &str,
    deadline: Duration,
) -> Result<ConnectionMetadata> {
    if deadline.is_zero() || deadline > Duration::from_secs(60) {
        return Err(error(ErrorCode::InvalidRequest));
    }
    let mut peer = Peer::spawn(spec).await?;
    let result = tokio::time::timeout(deadline, exchange(&mut peer, connection, route))
        .await
        .unwrap_or_else(|_| Err(error(ErrorCode::Timeout)));
    peer.shutdown().await;
    result
}
pub async fn codex_managed_metadata(
    spec: LaunchSpec,
    connection: &str,
    route: &str,
    identity: crate::codex::ManagedIdentity,
    deadline: Duration,
) -> Result<ConnectionMetadata> {
    if deadline.is_zero() || deadline > Duration::from_secs(60) {
        return Err(error(ErrorCode::InvalidRequest));
    }
    let mut peer = Peer::spawn(spec).await?;
    let result = tokio::time::timeout(
        deadline,
        exchange_with_identity(&mut peer, connection, route, Some(&identity)),
    )
    .await
    .unwrap_or_else(|_| Err(error(ErrorCode::Timeout)));
    peer.shutdown().await;
    result
}
pub(crate) async fn exchange(
    peer: &mut Peer,
    connection: &str,
    route: &str,
) -> Result<ConnectionMetadata> {
    exchange_with_identity(peer, connection, route, None).await
}

pub(crate) async fn exchange_with_identity(
    peer: &mut Peer,
    connection: &str,
    route: &str,
    managed: Option<&crate::codex::ManagedIdentity>,
) -> Result<ConnectionMetadata> {
    peer.managed = managed.is_some();
    let app_name = managed.map_or("originlm", |identity| identity.app_name.as_str());
    let app_title = managed.map_or("OriginLM", |identity| identity.app_name.as_str());
    let initialize=peer.request("initialize",json!({"clientInfo":{"name":app_name,"title":app_title,"version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":false}})).await?;
    peer.send(json!({"method":"initialized","params":{}}))
        .await?;
    let account = peer
        .request("account/read", json!({"refreshToken":false}))
        .await?;
    peer.account_established = true;
    peer.account_stamp = account_identity_stamp(&account["account"]);
    if managed.is_none() && account["account"].is_null() {
        return crate::codex::parse_metadata(connection, route, &initialize, &account, &[], None);
    }
    let mut models = Vec::new();
    let mut cursors = BTreeSet::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut params = json!({"limit":100,"includeHidden":false});
        if let Some(value) = &cursor {
            params["cursor"] = json!(value);
        }
        let page = peer.request("model/list", params).await?;
        let values = page["data"]
            .as_array()
            .ok_or_else(|| error(ErrorCode::ProtocolIncompatible))?;
        if models.len() + values.len() > 2048 {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        models.extend(values.iter().cloned());
        cursor = match page.get("nextCursor") {
            None | Some(Value::Null) => None,
            Some(Value::String(next)) if !next.is_empty() && next.len() <= 4096 => {
                Some(next.clone())
            }
            _ => return Err(error(ErrorCode::ProtocolIncompatible)),
        };
        let Some(next) = &cursor else {
            break;
        };
        if !cursors.insert(next.clone()) || cursors.len() > 32 {
            return Err(error(ErrorCode::ProtocolIncompatible));
        }
    }
    // The saved service tier is the current selection across models that
    // already advertise it. A missing or rejected config read leaves the
    // catalog defaults in place. Reimplemented from T3 Code
    // `applyConfiguredCodexServiceTier` at
    // [3b844dab5c](https://github.com/pingdotgg/t3code/commit/3b844dab5c).
    let configured_tier = match peer
        .request("config/read", json!({"includeLayers": false}))
        .await
    {
        Ok(value) => value["config"]["service_tier"]
            .as_str()
            .map(str::to_owned)
            .filter(|tier| {
                !tier.is_empty()
                    && tier.len() <= 64
                    && tier
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            }),
        Err(error) if error.code == ErrorCode::ProviderUnavailable => None,
        Err(error) => return Err(error),
    };
    let limits = if managed.is_some() {
        None
    } else {
        match peer.request("account/rateLimits/read", json!({})).await {
            Ok(value) => Some(value),
            Err(e) if e.code == ErrorCode::ProviderUnavailable => None,
            Err(error) => return Err(error),
        }
    };
    // A token refresh during the probe is fine; a different account is not.
    if peer.account_update_seen {
        peer.verify_account().await?;
    }
    let mut report = if let Some(identity) = managed {
        crate::codex::parse_managed_metadata(connection, route, &initialize, identity, &models)?
    } else {
        crate::codex::parse_metadata(
            connection,
            route,
            &initialize,
            &account,
            &models,
            limits.as_ref(),
        )?
    };
    if let Some(tier) = configured_tier.as_deref() {
        crate::codex::apply_configured_service_tier(&mut report.models, tier);
    }
    Ok(report)
}

/// Execute an explicitly chosen metadata command. No shell or unbounded output.
pub async fn capture(spec: LaunchSpec, deadline: Duration) -> Result<String> {
    if deadline.is_zero() || deadline > Duration::from_secs(60) {
        return Err(error(ErrorCode::InvalidRequest));
    }
    let mut peer = Peer::spawn(spec).await?;
    let result = tokio::time::timeout(deadline, async {
        peer.input
            .shutdown()
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        let mut bytes = Vec::new();
        (&mut peer.output)
            .take(MAX_FRAME + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        if bytes.len() as u64 > MAX_FRAME {
            return Err(error(ErrorCode::FrameTooLarge));
        }
        let status = peer
            .child
            .wait()
            .await
            .map_err(|_| error(ErrorCode::ProviderUnavailable))?;
        if !status.success() {
            return Err(error(ErrorCode::ProviderUnavailable));
        }
        String::from_utf8(bytes).map_err(|_| error(ErrorCode::ProtocolIncompatible))
    })
    .await
    .unwrap_or_else(|_| Err(error(ErrorCode::Timeout)));
    peer.shutdown().await;
    result
}

#[cfg(test)]
mod account_identity_tests {
    use super::{account_identity_stamp, account_update_signs_out};
    use serde_json::json;

    #[test]
    fn volatile_account_fields_do_not_change_the_identity() {
        let first = json!({"type":"chatgpt","email":"user@example.test","planType":"pro"});
        let later = json!({"type":"chatgpt","email":"User@Example.test","planType":"unknown","workspaceRouting":{"chatgptAccountId":"x"}});
        assert_eq!(
            account_identity_stamp(&first),
            account_identity_stamp(&later)
        );
        assert!(account_identity_stamp(&first).is_some());
    }

    #[test]
    fn a_different_account_or_auth_kind_is_a_new_identity() {
        let base = account_identity_stamp(&json!({"type":"chatgpt","email":"a@example.test"}));
        assert_ne!(
            base,
            account_identity_stamp(&json!({"type":"chatgpt","email":"b@example.test"}))
        );
        assert_ne!(base, account_identity_stamp(&json!({"type":"apiKey"})));
        assert_eq!(account_identity_stamp(&json!(null)), None);
    }

    #[test]
    fn only_a_sign_out_update_ends_the_session() {
        assert!(account_update_signs_out(&json!({"authMode":null})));
        assert!(!account_update_signs_out(
            &json!({"authMode":"chatgpt","planType":"pro"})
        ));
        assert!(!account_update_signs_out(&json!({"planType":"plus"})));
    }
}
