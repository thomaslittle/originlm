//! Dependency-first entry point for the local AI runtime.
//!
//! `LocalAi::start()` resolves, in order:
//!
//! 1. a **shared broker** the user already granted this application,
//! 2. a **private helper** child process (`serve --ipc`, parent-owned lifetime),
//! 3. the kit **in this process** when no helper binary exists at all.
//!
//! The standalone app, service and control center are never required, and this
//! crate never installs or downloads anything. Construction is inert until
//! [`LocalAi::start`] is called; nothing is scanned or probed at import time.
//!
//! The private helper is preferred over the in-process runtime because it keeps
//! broker work (scheduling, cache, provider probes) out of the application
//! process: a crash or a blocked adapter cannot take the host down. In-process
//! is the fallback that keeps "no separate install" true, and it is reported as
//! such through [`Mode::InProcess`].
//!
//! ```no_run
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut ai = originlm::LocalAi::start()?;
//!     let snapshot = ai.snapshot()?;
//!     println!("{} providers via {:?}", snapshot.providers.len(), ai.mode());
//!     Ok(())
//! }
//! ```
use originlm_contracts::Result;
pub use originlm_contracts::{DomainError, ErrorCode, Snapshot};
use originlm_control::BrokerClient;
use originlm_kit::{ClientPolicy, EmbeddedClient, Kit};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    env,
    ffi::OsString,
    io::{BufRead, BufReader, Read as _},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// How the runtime is running for this application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A broker that was already running, reachable through this application's
    /// own grant.
    Shared,
    /// A helper child owned by this process. It exits when this value drops.
    Child,
    /// The kit inside this process, used only when no helper binary exists.
    InProcess,
}

/// Options for [`LocalAi::start_with`]. The defaults are deliberately inert and
/// discovery-only: passive inventory, no probes, no execution.
#[derive(Debug, Clone)]
pub struct StartOptions {
    /// Shared broker address. Defaults to `LAI_BROKER_ADDRESS`, then a
    /// per-user path (`$XDG_RUNTIME_DIR/originlm/user.sock` on Linux, a
    /// filesystem path under the home directory elsewhere, and the namespaced
    /// pipe name on Windows).
    pub address: Option<String>,
    /// Owner-only token descriptor for the shared broker. Defaults to
    /// `LAI_BROKER_TOKEN_FILE`, then `~/.originlm/broker-token.json`.
    pub token_file: Option<PathBuf>,
    /// Explicit helper binary. Defaults to `LAI_HELPER_PATH`, then a binary next
    /// to the current executable, then `originlm` on `PATH`.
    pub helper: Option<PathBuf>,
    /// Client policy for the private helper and the in-process client.
    pub policy: ClientPolicy,
    /// Per-install state directory for the in-process runtime.
    pub state_dir: Option<PathBuf>,
    /// Allow the in-process fallback. Disable to require an isolated helper.
    pub allow_in_process: bool,
}

impl Default for StartOptions {
    fn default() -> Self {
        Self {
            address: None,
            token_file: None,
            helper: None,
            policy: ClientPolicy::passive(),
            state_dir: None,
            allow_in_process: true,
        }
    }
}

/// A started runtime with its active [`Mode`].
pub struct LocalAi {
    mode: Mode,
    fallback_reason: Option<&'static str>,
    backend: Backend,
}

enum Backend {
    Shared(BrokerClient),
    Child {
        client: BrokerClient,
        _child: OwnedHelper,
    },
    InProcess {
        client: EmbeddedClient,
        runtime: tokio::runtime::Runtime,
    },
}

/// A small, stable status surface for badges and menus.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub mode: Mode,
    pub fallback_reason: Option<&'static str>,
    pub revision: String,
    /// Provider display names, never account identities.
    pub providers: Vec<String>,
    pub models: usize,
    /// Present only when this application holds the administrative surface.
    pub connected_apps: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    protocol_version: String,
    address: String,
    token: String,
    #[serde(default)]
    independent: bool,
}

fn unavailable(message: &str) -> DomainError {
    DomainError::new(ErrorCode::RuntimeMissing, message)
}

fn denied(message: &str) -> DomainError {
    DomainError::new(ErrorCode::LocalAccessDenied, message)
}

fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The well-known address of a per-user shared broker.
pub fn default_address() -> String {
    if let Some(value) = env::var_os("LAI_BROKER_ADDRESS")
        && let Some(value) = value.to_str().filter(|value| valid_address(value))
    {
        return value.to_string();
    }
    if cfg!(windows) {
        return "originlm-user".into();
    }
    if let Some(runtime) = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
        && runtime.is_absolute()
    {
        return runtime.join("originlm/user.sock").to_string_lossy().into();
    }
    home_dir()
        .map(|home| home.join(".originlm/user.sock"))
        .unwrap_or_else(|| PathBuf::from("/tmp/originlm-user.sock"))
        .to_string_lossy()
        .into()
}

/// The token descriptor an application receives when the user grants it access
/// to the shared broker.
pub fn default_token_file() -> Option<PathBuf> {
    if let Some(value) = env::var_os("LAI_BROKER_TOKEN_FILE") {
        return Some(PathBuf::from(value));
    }
    home_dir().map(|home| home.join(".originlm/broker-token.json"))
}

fn valid_address(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && !value.chars().any(char::is_control)
        && !value.contains("://")
        && !value.starts_with("//")
        && !value.starts_with(r"\\")
}

fn valid_token(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_descriptor(line: &str) -> Result<Descriptor> {
    let descriptor: Descriptor = serde_json::from_str(line.trim()).map_err(|_| {
        DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The helper returned an incompatible descriptor",
        )
    })?;
    if descriptor.protocol_version != "1.0"
        || !valid_address(&descriptor.address)
        || !valid_token(&descriptor.token)
        || descriptor.independent
    {
        return Err(DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The helper returned an incompatible descriptor",
        ));
    }
    Ok(descriptor)
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "originlm.exe"
    } else {
        "originlm"
    }
}

/// A helper binary must be a real executable file, never a shell wrapper.
pub fn usable_helper(path: &Path) -> bool {
    if !path.is_absolute() || path.as_os_str().is_empty() {
        return false;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ["cmd", "bat", "ps1"].contains(&extension.as_str()) {
        return false;
    }
    path.metadata().is_ok_and(|meta| {
        if !meta.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 == 0 {
                return false;
            }
        }
        true
    })
}

fn path_helper() -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(executable_name()))
        .find(|candidate| usable_helper(candidate))
}

/// Resolves the private helper: explicit, then adjacent to this executable,
/// then `PATH`.
pub fn resolve_helper(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(explicit) = explicit {
        return usable_helper(&explicit).then_some(explicit);
    }
    if let Some(value) = env::var_os("LAI_HELPER_PATH") {
        let path = PathBuf::from(value);
        return usable_helper(&path).then_some(path);
    }
    if let Ok(current) = env::current_exe()
        && let Some(directory) = current.parent()
    {
        let adjacent = directory.join(executable_name());
        if usable_helper(&adjacent) {
            return Some(adjacent);
        }
    }
    path_helper()
}

fn shared_token(path: &Path, expected_address: &str) -> Option<String> {
    let bytes = originlm_platform::read_private_file(path, 16 * 1024).ok()?;
    let descriptor: Value = serde_json::from_slice(&bytes).ok()?;
    if descriptor
        .get("address")
        .and_then(Value::as_str)
        .is_some_and(|address| address != expected_address)
    {
        return None;
    }
    let token = descriptor.get("token")?.as_str()?;
    valid_token(token).then(|| token.to_string())
}

fn address_listening(address: &str, token: &str) -> bool {
    if !cfg!(windows) {
        // A filesystem socket that is not there cannot be listening, and probing
        // must not create anything.
        if !address.contains("://")
            && Path::new(address).is_absolute()
            && !Path::new(address).exists()
        {
            return false;
        }
    }
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            originlm_transports::local_ipc::connect(address),
        )
        .await
        .is_ok_and(|result| result.is_ok() && valid_token(token))
    })
}

fn unique_address() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    if cfg!(windows) {
        format!("originlm-private-{}-{nanos}", std::process::id())
    } else {
        let name = format!("originlm-{}-{nanos}.sock", std::process::id());
        let candidate = env::temp_dir().join(&name);
        // Unix sockets have a much smaller limit than filesystem paths. Keep
        // the authenticated owner-only endpoint usable with a long TMPDIR.
        let path = if candidate.as_os_str().len() <= 100 {
            candidate
        } else {
            PathBuf::from("/tmp").join(name)
        };
        path.to_string_lossy().into()
    }
}

fn child_args(address: &str, policy: &ClientPolicy) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["serve".into(), "--ipc".into(), address.into()];
    if policy.allow_passive_discovery {
        args.push("--allow-passive-discovery".into());
    }
    for provider in &policy.probe_provider_ids {
        args.push("--allow-metadata".into());
        args.push(provider.into());
    }
    args
}

/// Own the process immediately, including while its startup handshake is pending.
struct OwnedHelper(Child);

impl std::ops::Deref for OwnedHelper {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for OwnedHelper {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for OwnedHelper {
    fn drop(&mut self) {
        let _ = self.0.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_private_helper(helper: &Path, policy: &ClientPolicy) -> Result<(OwnedHelper, Descriptor)> {
    let address = unique_address();
    let mut child = OwnedHelper(
        Command::new(helper)
            .args(child_args(&address, policy))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| unavailable("The private helper could not start"))?,
    );
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| unavailable("The private helper has no control channel"))?;
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("originlm-helper-descriptor".into())
        .spawn(move || {
            let mut reader = BufReader::new(stdout).take(4097);
            let mut line = String::new();
            let read = reader.read_line(&mut line);
            let _ = sender.send((read, line));
        })
        .map_err(|_| unavailable("The private helper control channel is unavailable"))?;
    let (read, line) = receiver
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| unavailable("The private helper did not become ready"))?;
    let Ok(read) = read else {
        let _ = child.kill();
        return Err(unavailable("The private helper control channel failed"));
    };
    if read == 0 || read > 4096 {
        let _ = child.kill();
        return Err(unavailable("The private helper did not become ready"));
    }
    match parse_descriptor(&line) {
        Ok(descriptor) if descriptor.address == address => Ok((child, descriptor)),
        Ok(_) => Err(DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The private helper returned a different address",
        )),
        Err(error) => {
            let _ = child.kill();
            Err(error)
        }
    }
}

impl LocalAi {
    /// Start with the default resolution order.
    pub fn start() -> Result<Self> {
        Self::start_with(StartOptions::default())
    }

    /// Start with explicit options. A missing shared broker or helper is not an
    /// error; a shared broker that is listening but rejects the token is.
    pub fn start_with(options: StartOptions) -> Result<Self> {
        if options
            .helper
            .as_deref()
            .is_some_and(|path| !usable_helper(path))
            || (options.helper.is_none()
                && env::var_os("LAI_HELPER_PATH")
                    .is_some_and(|path| !usable_helper(Path::new(&path))))
        {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "The configured helper must be an absolute executable file; fix LAI_HELPER_PATH or StartOptions.helper",
            ));
        }
        let mut fallback_reason = None;
        let address = options.address.clone().unwrap_or_else(default_address);
        let token_file = options.token_file.clone().or_else(default_token_file);
        if let Some(path) = token_file.as_deref()
            && let Some(token) = shared_token(path, &address)
            && address_listening(&address, &token)
        {
            match BrokerClient::connect(&address, &token) {
                Ok(client) => {
                    return Ok(Self {
                        mode: Mode::Shared,
                        fallback_reason: None,
                        backend: Backend::Shared(client),
                    });
                }
                Err(error) if error.starts_with("PROTOCOL_INCOMPATIBLE:") => {
                    fallback_reason = Some("FALLBACK_PROTOCOL_MISMATCH");
                }
                Err(_) => {
                    return Err(denied(
                        "The shared broker rejected this application's grant or returned an invalid response",
                    ));
                }
            }
        }
        if let Some(helper) = resolve_helper(options.helper.clone()) {
            let (child, descriptor) = spawn_private_helper(&helper, &options.policy)?;
            let client = BrokerClient::connect(&descriptor.address, &descriptor.token)
                .map_err(|_| denied("The private helper rejected its own grant"))?;
            return Ok(Self {
                mode: Mode::Child,
                fallback_reason,
                backend: Backend::Child {
                    client,
                    _child: child,
                },
            });
        }
        if !options.allow_in_process {
            return Err(unavailable(
                "No shared broker or helper binary is available, and the in-process fallback is disabled",
            ));
        }
        let mut builder = Kit::builder().execution(false);
        if let Some(directory) = options.state_dir.clone() {
            builder = builder.state_dir(directory);
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|_| unavailable("The in-process runtime is unavailable"))?;
        let client = {
            let _entered = runtime.enter();
            let kit = builder.build()?;
            kit.service().start_background_tasks();
            kit.connect("originlm-dependency", options.policy.clone())?
        };
        Ok(Self {
            mode: Mode::InProcess,
            fallback_reason,
            backend: Backend::InProcess { client, runtime },
        })
    }

    /// Why a shared protocol selected a private/embedded fallback, when applicable.
    pub fn fallback_reason(&self) -> Option<&'static str> {
        self.fallback_reason
    }

    /// Which resolution step succeeded.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// One JSON-RPC call. The helper and the shared broker use the same surface.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        match &mut self.backend {
            Backend::Shared(client) => client.call_typed(method, params),
            Backend::Child { client, .. } => client.call_typed(method, params),
            Backend::InProcess {
                client, runtime, ..
            } => runtime.block_on(client.request::<Value>(method, params)),
        }
    }

    /// The committed inventory snapshot.
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        match &mut self.backend {
            Backend::Shared(client) => typed_snapshot(client),
            Backend::Child { client, .. } => typed_snapshot(client),
            Backend::InProcess {
                client, runtime, ..
            } => runtime.block_on(client.snapshot()),
        }
    }

    /// Passive discovery plus a fresh snapshot.
    pub fn scan(&mut self) -> Result<Snapshot> {
        self.call("discovery.refresh", json!({"level":"passive"}))?;
        self.snapshot()
    }

    /// A small status object for badges and menus. Connected apps are only
    /// present when this application holds the administrative surface.
    pub fn status(&mut self) -> Result<Status> {
        let snapshot = self.snapshot()?;
        let connected_apps = self
            .call("apps.list", json!({}))
            .ok()
            .and_then(|value| value.as_array().map(|apps| apps.len()));
        Ok(Status {
            mode: self.mode,
            fallback_reason: self.fallback_reason,
            revision: snapshot.revision.clone(),
            providers: snapshot
                .providers
                .iter()
                .map(|provider| provider.name.clone())
                .collect(),
            models: snapshot.models.len(),
            connected_apps,
        })
    }
}

fn typed_snapshot(client: &mut BrokerClient) -> Result<Snapshot> {
    serde_json::from_value(client.call_typed("snapshot.get", json!({}))?).map_err(|_| {
        DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The broker returned an incompatible snapshot",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_requires_the_parent_protocol_and_a_real_token() {
        let token = "a".repeat(64);
        let good = format!(
            r#"{{"protocolVersion":"1.0","address":"/tmp/example.sock","token":"{token}","expiresInSeconds":300,"independent":false}}"#
        );
        assert_eq!(parse_descriptor(&good).unwrap().token, token);
        let reusable = format!(
            r#"{{"protocolVersion":"1.0","address":"/tmp/example.sock","token":"{token}","independent":true}}"#
        );
        assert!(parse_descriptor(&reusable).is_err());
        let short = r#"{"protocolVersion":"1.0","address":"/tmp/example.sock","token":"abc","independent":false}"#;
        assert!(parse_descriptor(short).is_err());
        assert!(parse_descriptor("{}").is_err());
    }

    #[test]
    fn helper_resolution_rejects_scripts_and_relative_paths() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("helper");
        std::fs::write(&file, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(!usable_helper(&file));
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(usable_helper(&file));
        assert!(!usable_helper(Path::new("helper")));
        let script = directory.path().join("helper.cmd");
        std::fs::write(&script, b"").unwrap();
        assert!(!usable_helper(&script));
        assert!(resolve_helper(Some(directory.path().join("missing"))).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn failed_private_descriptor_reaps_the_owned_process() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let helper = directory.path().join("helper");
        let pid_file = directory.path().join("pid");
        // Synthetic child: no provider, home directory, or account access.
        std::fs::write(
            &helper,
            format!(
                "#!/bin/sh\necho $$ > '{}'\necho invalid\nwhile read line; do :; done\n",
                pid_file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(spawn_private_helper(&helper, &ClientPolicy::denied()).is_err());
        let pid = std::fs::read_to_string(pid_file).unwrap();
        let status = Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "failed startup must reap its child");
    }

    #[test]
    fn invalid_explicit_helper_never_falls_back() {
        let directory = tempfile::tempdir().unwrap();
        let result = LocalAi::start_with(StartOptions {
            helper: Some(directory.path().join("missing")),
            ..StartOptions::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn child_arguments_carry_only_granted_scopes() {
        let mut policy = ClientPolicy::passive();
        policy.probe_provider_ids.insert("codex".into());
        let args: Vec<String> = child_args("/tmp/example.sock", &policy)
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "serve");
        assert_eq!(args[1], "--ipc");
        assert!(args.contains(&"--allow-passive-discovery".to_string()));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--allow-metadata", "codex"])
        );
        let closed = child_args("/tmp/example.sock", &ClientPolicy::denied());
        assert!(!closed.iter().any(|arg| arg == "--allow-passive-discovery"));
    }

    #[test]
    fn token_descriptors_must_be_owner_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("token.json");
        let token = "b".repeat(64);
        std::fs::write(
            &path,
            format!(r#"{{"token":"{token}","address":"/tmp/example.sock"}}"#),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(
                shared_token(&path, "/tmp/example.sock").as_deref(),
                Some(token.as_str())
            );
            assert!(shared_token(&path, "/tmp/other.sock").is_none());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(shared_token(&path, "/tmp/example.sock").is_none());
        }
        #[cfg(not(unix))]
        assert_eq!(
            shared_token(&path, "/tmp/example.sock").as_deref(),
            Some(token.as_str())
        );
    }
}
