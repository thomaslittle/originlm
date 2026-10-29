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
//! # Explicit bootstrap and shared identity
//!
//! Set [`StartOptions::required_mode`] to choose one backend and disable every
//! fallback. An explicit [`Mode::Shared`] bootstrap requires an address, an
//! owner-only application token descriptor, and [`StartOptions::expected_app_id`].
//! Every imported descriptor is validated at the same read/authentication
//! boundary: protocol `1.0`, `scope: "application"`, `independent: true`, a
//! non-empty `appId`, the exact broker address and the expected application
//! identity. A missing, malformed, foreign or revoked explicit grant fails
//! closed; it never selects a helper or in-process fallback. Automatic
//! resolution may still fall back when no explicit descriptor or identity was
//! supplied.
//!
//! [`Mode::InProcess`] as the required mode bypasses shared-token discovery,
//! `PATH`, adjacent-helper and `LAI_HELPER_PATH` resolution. [`Mode::Child`]
//! uses only [`StartOptions::helper`]. Helper bytes are not registry-
//! authenticated: an executable found automatically is a local convenience, not
//! proof of provenance. Applications that must not trust an arbitrary helper
//! should require in-process mode.
//!
//! # Cancellation and deadlines
//!
//! [`LocalAi::call`] uses [`StartOptions::request_timeout`], and
//! [`LocalAi::close`] uses [`StartOptions::close_timeout`]. Clone a
//! [`CancelToken`] into the host's Stop/Disable path and call
//! [`LocalAi::call_cancellable`]. Cancelling an in-process request drops the
//! pending future and leaves the client usable. Cancelling a shared or child
//! request returns [`ErrorCode::Cancelled`] and closes that transport because a
//! partial frame cannot be reused; restart to reconnect. The owned helper's
//! drop reaps its immediate child process on a best-effort bound; descendant
//! cleanup is not claimed beyond that evidence.
//!
//! In-process discovery can use [`StartOptions::home`],
//! [`StartOptions::inventory`], [`StartOptions::native_clients`] and
//! [`StartOptions::app_label`] without mutating process-global `HOME`, `PATH` or
//! provider configuration. Those fields are honored only by the in-process
//! runtime; supplying them while selecting automatic, shared or child mode is
//! rejected instead of silently scanning a broader machine home. The facade
//! re-exports [`ScanResult`], [`LocatedClient`], [`ExecutableIdentity`],
//! [`NativeProfile`] and [`NativeProvider`] so a consumer can build synthetic
//! inventory and scoped native configuration with only this crate.
//!
//! [`LocalAi::close`] cooperatively cancels in-process background lanes and the
//! active inventory refresh, then waits at most
//! [`StartOptions::close_timeout`] for blocking work. A sync inventory callback
//! that ignores its cancel flag cannot be aborted by Rust; the runtime detaches
//! it after the deadline rather than letting it extend the host's close.
//!
//! ```no_run
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut ai = originlm::LocalAi::start()?;
//!     let snapshot = ai.snapshot()?;
//!     println!("{} providers via {:?}", snapshot.providers.len(), ai.mode());
//!     Ok(())
//! }
//! ```
//!
//! # Execution and consent
//!
//! Execution is a separate grant, never implied by starting the runtime or by
//! passive discovery. In-process execution drivers are registered only when
//! [`StartOptions::execution`] is `true`; registration alone still denies every
//! session. The host must then call [`LocalAi::authorize_execution`] with the
//! exact connections, providers, workspaces, and write/paid/native permissions
//! the user approved. There is no implicit or machine-wide grant, and no method
//! here starts a service, reads provider credentials, or widens a policy.
//!
//! In [`Mode::Shared`] and [`Mode::Child`] the broker owns the grant. This crate
//! can use that grant but cannot create or change it; the user approves the
//! application through the broker's own consent flow. Calling
//! [`LocalAi::authorize_execution`] in those modes fails instead of bypassing
//! the broker.
//!
//! Session methods mirror the public runtime methods:
//! [`LocalAi::create_session`], [`LocalAi::get_session`],
//! [`LocalAi::suspend_session`], [`LocalAi::resume_session`],
//! [`LocalAi::update_session_options`], [`LocalAi::close_session`],
//! [`LocalAi::start_turn`], [`LocalAi::cancel_turn`],
//! [`LocalAi::respond_to_approval`], [`LocalAi::read_events`], and
//! [`LocalAi::subscribe_events`]. Every call carries the caller's existing
//! grant; a missing grant surfaces as
//! [`ErrorCode::LocalAccessDenied`] before any agent is started.
use originlm_contracts::Result;
pub use originlm_contracts::{
    ApprovalParams, DataMode, DomainError, ErrorCode, Event, EventPage, HostMcpServer, Selection,
    Session, SessionCreateParams, SessionOptionsParams, SessionParams, SessionResumeParams,
    SessionSuspendResult, Snapshot, TurnInputRef, TurnParams, TurnStartParams, TurnStarted,
};
use originlm_control::{BrokerClient, DEFAULT_CALL_TIMEOUT};
pub use originlm_kit::{
    ClientPolicy, ExecutableIdentity, ExecutionGrant, InventorySource, LocatedClient,
    NativeClientRegistry, NativeProfile, NativeProvider, ScanResult,
};
use originlm_kit::{EmbeddedClient, Kit};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    env,
    ffi::OsString,
    fmt,
    io::{BufRead, BufReader, Read as _},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Default deadline for one `LocalAi` request.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = DEFAULT_CALL_TIMEOUT;
/// Default deadline for closing a broker transport.
pub const DEFAULT_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

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
///
/// Set [`Self::required_mode`] to select one backend explicitly and disable the
/// automatic fallback chain. This is the supported choice for applications that
/// must not silently change transport (for example, an app whose user approved
/// an application-scoped shared grant, or an app that needs cancellable calls
/// in-process). Automatic resolution remains available when `required_mode` is
/// `None`.
pub struct StartOptions {
    /// Shared broker address. Defaults to `LAI_BROKER_ADDRESS`, then a
    /// per-user path (`$XDG_RUNTIME_DIR/originlm/user.sock` on Linux, a
    /// filesystem path under the home directory elsewhere, and the namespaced
    /// pipe name on Windows).
    pub address: Option<String>,
    /// Owner-only token descriptor for the shared broker. Defaults to
    /// `LAI_BROKER_TOKEN_FILE`, then `~/.originlm/broker-token.json`.
    ///
    /// Every imported descriptor must be an application grant (`scope:
    /// "application"`, `independent: true`, a non-empty `appId`, and protocol
    /// `1.0`). An explicitly supplied descriptor that is missing, malformed or
    /// revoked fails closed and never selects a helper or embedded fallback.
    pub token_file: Option<PathBuf>,
    /// Application identity expected in the shared grant (`appId`). When set,
    /// the descriptor must name exactly this application. Required by an
    /// explicit [`Mode::Shared`] bootstrap.
    pub expected_app_id: Option<String>,
    /// Explicit helper binary. Defaults to `LAI_HELPER_PATH`, then a binary next
    /// to the current executable, then `originlm` on `PATH`. An explicit
    /// [`Mode::Child`] bootstrap uses only this path.
    pub helper: Option<PathBuf>,
    /// Require exactly this backend; never fall back. `None` keeps the
    /// documented automatic order.
    pub required_mode: Option<Mode>,
    /// Client policy for the private helper and the in-process client.
    ///
    /// A private helper can only express passive discovery plus provider-level
    /// probes. A policy that narrows connections or asks for identity/admin
    /// surfaces is rejected before the helper is spawned.
    pub policy: ClientPolicy,
    /// Per-install state directory for the in-process runtime.
    pub state_dir: Option<PathBuf>,
    /// Host-owned home used for in-process discovery. Avoids reading or
    /// mutating the process-global `HOME`.
    pub home: Option<PathBuf>,
    /// Deterministic inventory for the in-process runtime, typically a
    /// synthetic fixture in tests or an app-owned scanner in production.
    pub inventory: Option<InventorySource>,
    /// Host-owned native client registry applied by the in-process runtime.
    pub native_clients: NativeClientRegistry,
    /// Application label recorded for the in-process client. Defaults to
    /// `originlm-dependency`.
    pub app_label: Option<String>,
    /// Allow the in-process fallback. Disable to require an isolated helper.
    pub allow_in_process: bool,
    /// Register in-process execution drivers. This is not permission: the host
    /// must still call [`LocalAi::authorize_execution`] with an explicit grant.
    /// Shared brokers and private helpers use their own broker-side grant.
    pub execution: bool,
    /// Deadline for one request. Applies to shared, child and in-process calls.
    pub request_timeout: Duration,
    /// Deadline for closing a shared or child transport.
    pub close_timeout: Duration,
}

impl Default for StartOptions {
    fn default() -> Self {
        Self {
            address: None,
            token_file: None,
            expected_app_id: None,
            helper: None,
            required_mode: None,
            policy: ClientPolicy::passive(),
            state_dir: None,
            home: None,
            inventory: None,
            native_clients: NativeClientRegistry::default(),
            app_label: None,
            allow_in_process: true,
            execution: false,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            close_timeout: DEFAULT_CLOSE_TIMEOUT,
        }
    }
}

impl fmt::Debug for StartOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StartOptions")
            .field("address", &self.address)
            .field("token_file", &self.token_file)
            .field("expected_app_id", &self.expected_app_id)
            .field("helper", &self.helper)
            .field("required_mode", &self.required_mode)
            .field("policy", &self.policy)
            .field("state_dir", &self.state_dir)
            .field("home", &self.home)
            .field("inventory", &self.inventory.as_ref().map(|_| "custom"))
            .field("native_clients", &self.native_clients)
            .field("app_label", &self.app_label)
            .field("allow_in_process", &self.allow_in_process)
            .field("execution", &self.execution)
            .field("request_timeout", &self.request_timeout)
            .field("close_timeout", &self.close_timeout)
            .finish()
    }
}

impl Clone for StartOptions {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            token_file: self.token_file.clone(),
            expected_app_id: self.expected_app_id.clone(),
            helper: self.helper.clone(),
            required_mode: self.required_mode,
            policy: self.policy.clone(),
            state_dir: self.state_dir.clone(),
            home: self.home.clone(),
            inventory: self.inventory.clone(),
            native_clients: self.native_clients.clone(),
            app_label: self.app_label.clone(),
            allow_in_process: self.allow_in_process,
            execution: self.execution,
            request_timeout: self.request_timeout,
            close_timeout: self.close_timeout,
        }
    }
}

/// A started runtime with its active [`Mode`].
pub struct LocalAi {
    mode: Mode,
    fallback_reason: Option<&'static str>,
    execution_enabled: bool,
    request_timeout: Duration,
    close_timeout: Duration,
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
    Closed,
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
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

/// A validated application-scoped shared broker grant.
struct SharedGrant {
    address: String,
    token: String,
}

/// An owned cancellation signal for one request. Clone it into the host's
/// Stop/Disable path; cancelling is safe from any thread.
#[derive(Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    async fn wait(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

fn unavailable(message: &str) -> DomainError {
    DomainError::new(ErrorCode::RuntimeMissing, message)
}

fn denied(message: &str) -> DomainError {
    DomainError::new(ErrorCode::LocalAccessDenied, message)
}

fn invalid(message: impl Into<String>) -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, message)
}

fn broker_error(error: String) -> DomainError {
    if error.starts_with("PROTOCOL_INCOMPATIBLE:") {
        DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The shared broker uses an incompatible protocol",
        )
    } else {
        denied(
            "The shared broker rejected this application's grant or returned an invalid response",
        )
    }
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

fn valid_app_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn parse_descriptor(line: &str) -> Result<Descriptor> {
    let descriptor: Descriptor = serde_json::from_str(line.trim()).map_err(|_| {
        DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The helper returned an incompatible descriptor",
        )
    })?;
    let scope_is_parent = descriptor
        .scope
        .as_deref()
        .is_none_or(|scope| scope == "single-use");
    if descriptor.protocol_version != "1.0"
        || !valid_address(&descriptor.address)
        || !valid_token(&descriptor.token)
        || descriptor.independent
        || !scope_is_parent
        || descriptor.app_id.is_some()
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

fn read_shared_grant(
    path: &Path,
    expected_address: &str,
    expected_app_id: Option<&str>,
) -> Result<SharedGrant> {
    let bytes = originlm_platform::read_private_file(path, 16 * 1024)
        .map_err(|_| denied("The shared broker grant is missing, unreadable or not owner-only"))?;
    let descriptor: Descriptor = serde_json::from_slice(&bytes)
        .map_err(|_| denied("The shared broker grant is malformed"))?;
    if descriptor.protocol_version != "1.0" {
        return Err(DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "The shared broker grant uses an incompatible protocol",
        ));
    }
    if descriptor.address != expected_address {
        return Err(denied(
            "The shared broker grant belongs to a different broker",
        ));
    }
    if !valid_address(&descriptor.address) || !valid_token(&descriptor.token) {
        return Err(denied("The shared broker grant is malformed"));
    }
    if !descriptor.independent || descriptor.scope.as_deref() != Some("application") {
        return Err(denied(
            "The shared descriptor is not an application grant; ask the user to grant this application access",
        ));
    }
    let app_id = descriptor
        .app_id
        .filter(|app_id| valid_app_id(app_id))
        .ok_or_else(|| denied("The shared broker grant has no valid application identity"))?;
    if expected_app_id.is_some_and(|expected| expected != app_id) {
        return Err(denied(
            "The shared broker grant belongs to a different application",
        ));
    }
    Ok(SharedGrant {
        address: descriptor.address,
        token: descriptor.token,
    })
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

/// The helper CLI can express exactly passive discovery plus provider-level
/// metadata probes. Anything narrower would otherwise be silently broadened, so
/// it must be rejected before a process is spawned.
fn validate_child_policy(policy: &ClientPolicy) -> Result<()> {
    let connection_shape = match &policy.connection_ids {
        None => policy.allow_passive_discovery,
        Some(ids) => !policy.allow_passive_discovery && ids.is_empty(),
    };
    if !connection_shape
        || !policy.probe_connection_ids.is_empty()
        || policy.reveal_identity
        || policy.admin
    {
        return Err(DomainError::new(
            ErrorCode::InvalidRequest,
            "The private helper cannot preserve this restricted client policy; use an explicit in-process mode for narrowed connections or identity/admin scopes",
        ));
    }
    Ok(())
}

/// Configuration that only the in-process runtime can honor. Other modes must
/// reject it instead of silently scanning a broader machine home.
fn embedded_only_option(options: &StartOptions) -> Option<&'static str> {
    if options.home.is_some() {
        Some("home")
    } else if options.inventory.is_some() {
        Some("inventory")
    } else if options.state_dir.is_some() {
        Some("state_dir")
    } else if options.app_label.is_some() {
        Some("app_label")
    } else if !options.native_clients.profiles.is_empty()
        || options.native_clients.browser_blocker.is_some()
    {
        Some("native_clients")
    } else {
        None
    }
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

    /// Start with explicit options.
    ///
    /// Automatic resolution (`required_mode: None`) treats a missing shared
    /// broker or helper as absence and may fall back. An explicitly supplied
    /// token descriptor, an expected application identity, or a required mode
    /// fails closed instead.
    pub fn start_with(options: StartOptions) -> Result<Self> {
        if options.request_timeout.is_zero() {
            return Err(invalid("request_timeout must be greater than zero"));
        }
        if options.close_timeout.is_zero() {
            return Err(invalid("close_timeout must be greater than zero"));
        }
        if let Some(app_id) = &options.expected_app_id
            && !valid_app_id(app_id)
        {
            return Err(invalid(
                "expected_app_id must be a non-empty, bounded application identity",
            ));
        }
        if let Some(home) = &options.home
            && (!home.is_absolute() || home.to_string_lossy().chars().any(char::is_control))
        {
            return Err(invalid("home must be an absolute local directory"));
        }
        if let Some(label) = &options.app_label
            && (label.is_empty() || label.len() > 128 || label.chars().any(char::is_control))
        {
            return Err(invalid("app_label must be a non-empty, bounded label"));
        }
        if options.required_mode != Some(Mode::InProcess)
            && let Some(option) = embedded_only_option(&options)
        {
            return Err(invalid(format!(
                "StartOptions.{option} is honored only by the in-process runtime; set required_mode to Some(Mode::InProcess) or remove it so a shared broker or helper cannot silently ignore the supplied scope"
            )));
        }
        if options.execution && options.required_mode != Some(Mode::InProcess) {
            return Err(invalid(
                "Execution drivers are in-process only; set required_mode to Some(Mode::InProcess) so a shared broker or helper cannot silently drop the requested execution",
            ));
        }
        match options.required_mode {
            Some(Mode::Shared) => Self::start_shared(&options),
            Some(Mode::Child) => Self::start_child_explicit(&options),
            Some(Mode::InProcess) => Self::start_in_process(&options, None),
            None => Self::start_auto(options),
        }
    }

    fn start_auto(options: StartOptions) -> Result<Self> {
        if options
            .helper
            .as_deref()
            .is_some_and(|path| !usable_helper(path))
            || (options.helper.is_none()
                && env::var_os("LAI_HELPER_PATH")
                    .is_some_and(|path| !usable_helper(Path::new(&path))))
        {
            return Err(invalid(
                "The configured helper must be an absolute executable file; fix LAI_HELPER_PATH or StartOptions.helper",
            ));
        }
        let explicit_shared = options.token_file.is_some() || options.expected_app_id.is_some();
        let mut fallback_reason = None;
        let address = options.address.clone().unwrap_or_else(default_address);
        let token_file = options.token_file.clone().or_else(default_token_file);
        if token_file.is_none() && explicit_shared {
            return Err(denied("The explicit shared broker grant is missing"));
        }
        if let Some(path) = token_file {
            match read_shared_grant(&path, &address, options.expected_app_id.as_deref()) {
                Ok(grant) if address_listening(&grant.address, &grant.token) => {
                    match BrokerClient::connect_with_timeout(
                        &grant.address,
                        &grant.token,
                        options.request_timeout,
                    ) {
                        Ok(client) => return Ok(Self::shared(client, None, &options)),
                        Err(error) if error.starts_with("PROTOCOL_INCOMPATIBLE:") => {
                            if explicit_shared {
                                return Err(broker_error(error));
                            }
                            fallback_reason = Some("FALLBACK_PROTOCOL_MISMATCH");
                        }
                        Err(_) => {
                            return Err(denied(
                                "The shared broker rejected this application's grant or returned an invalid response",
                            ));
                        }
                    }
                }
                Ok(_) if explicit_shared => {
                    return Err(unavailable(
                        "The explicit shared broker grant is not listening",
                    ));
                }
                Ok(_) => {}
                Err(error) if explicit_shared => return Err(error),
                Err(_) => {}
            }
        }
        if let Some(helper) = resolve_helper(options.helper.clone()) {
            validate_child_policy(&options.policy)?;
            let (child, descriptor) = spawn_private_helper(&helper, &options.policy)?;
            let client = BrokerClient::connect_with_timeout(
                &descriptor.address,
                &descriptor.token,
                options.request_timeout,
            )
            .map_err(|_| denied("The private helper rejected its own grant"))?;
            return Ok(Self {
                mode: Mode::Child,
                fallback_reason,
                execution_enabled: false,
                request_timeout: options.request_timeout,
                close_timeout: options.close_timeout,
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
        Self::start_in_process(&options, fallback_reason)
    }

    fn start_shared(options: &StartOptions) -> Result<Self> {
        let address = options
            .address
            .clone()
            .ok_or_else(|| invalid("An explicit shared mode requires StartOptions.address"))?;
        let path = options.token_file.clone().ok_or_else(|| {
            invalid("An explicit shared mode requires an application token descriptor")
        })?;
        let app_id = options.expected_app_id.clone().ok_or_else(|| {
            invalid("An explicit shared mode requires StartOptions.expected_app_id")
        })?;
        let grant = read_shared_grant(&path, &address, Some(&app_id))?;
        if !address_listening(&grant.address, &grant.token) {
            return Err(unavailable(
                "The explicit shared broker grant is not listening",
            ));
        }
        let client = BrokerClient::connect_with_timeout(
            &grant.address,
            &grant.token,
            options.request_timeout,
        )
        .map_err(broker_error)?;
        Ok(Self::shared(client, None, options))
    }

    fn start_child_explicit(options: &StartOptions) -> Result<Self> {
        let helper = options
            .helper
            .clone()
            .ok_or_else(|| invalid("An explicit child mode requires StartOptions.helper"))?;
        if !usable_helper(&helper) {
            return Err(invalid(
                "The configured helper must be an absolute executable file",
            ));
        }
        validate_child_policy(&options.policy)?;
        let (child, descriptor) = spawn_private_helper(&helper, &options.policy)?;
        let client = BrokerClient::connect_with_timeout(
            &descriptor.address,
            &descriptor.token,
            options.request_timeout,
        )
        .map_err(|_| denied("The private helper rejected its own grant"))?;
        Ok(Self {
            mode: Mode::Child,
            fallback_reason: None,
            execution_enabled: false,
            request_timeout: options.request_timeout,
            close_timeout: options.close_timeout,
            backend: Backend::Child {
                client,
                _child: child,
            },
        })
    }

    fn start_in_process(
        options: &StartOptions,
        fallback_reason: Option<&'static str>,
    ) -> Result<Self> {
        let mut builder = Kit::builder().execution(options.execution);
        if let Some(directory) = options.state_dir.clone() {
            builder = builder.state_dir(directory);
        }
        if let Some(home) = options.home.clone() {
            builder = builder.home(home);
        }
        if let Some(inventory) = options.inventory.clone() {
            builder = builder.inventory(inventory);
        }
        builder = builder.native_clients(options.native_clients.clone());
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|_| unavailable("The in-process runtime is unavailable"))?;
        let label = options
            .app_label
            .as_deref()
            .unwrap_or("originlm-dependency");
        let client = {
            let _entered = runtime.enter();
            let kit = builder.build()?;
            kit.service().start_background_tasks();
            kit.connect(label, options.policy.clone())?
        };
        Ok(Self {
            mode: Mode::InProcess,
            fallback_reason,
            execution_enabled: options.execution,
            request_timeout: options.request_timeout,
            close_timeout: options.close_timeout,
            backend: Backend::InProcess { client, runtime },
        })
    }

    fn shared(
        client: BrokerClient,
        fallback_reason: Option<&'static str>,
        options: &StartOptions,
    ) -> Self {
        Self {
            mode: Mode::Shared,
            fallback_reason,
            execution_enabled: false,
            request_timeout: options.request_timeout,
            close_timeout: options.close_timeout,
            backend: Backend::Shared(client),
        }
    }

    /// Why a shared protocol selected a private/embedded fallback, when applicable.
    pub fn fallback_reason(&self) -> Option<&'static str> {
        self.fallback_reason
    }

    /// Which resolution step succeeded.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Whether in-process execution drivers were registered. This is not a
    /// grant; see [`LocalAi::authorize_execution`].
    pub fn execution_enabled(&self) -> bool {
        self.execution_enabled
    }

    /// One JSON-RPC call with the configured deadline.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let cancel = CancelToken::new();
        self.call_cancellable(method, params, &cancel)
    }

    /// One JSON-RPC call with an independently cancellable signal.
    ///
    /// Cancelling an in-process request drops its pending future at the next
    /// await point and leaves the client usable. Cancelling a shared or child
    /// request returns [`ErrorCode::Cancelled`] and closes that transport,
    /// because a partial frame cannot be reused; restart to reconnect.
    pub fn call_cancellable(
        &mut self,
        method: &str,
        params: Value,
        cancel: &CancelToken,
    ) -> Result<Value> {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        let timeout = self.request_timeout;
        match &mut self.backend {
            Backend::Shared(client) => {
                client.call_typed_with(method, params, timeout, Some(&cancel.cancelled))
            }
            Backend::Child { client, .. } => {
                client.call_typed_with(method, params, timeout, Some(&cancel.cancelled))
            }
            Backend::InProcess {
                client, runtime, ..
            } => runtime.block_on(request_in_process(client, method, params, timeout, cancel)),
            Backend::Closed => Err(closed()),
        }
    }

    /// The committed inventory snapshot.
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        self.typed_request("snapshot.get", json!({}))
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

    /// Record the user's explicit execution grant for this in-process runtime.
    ///
    /// The grant names exact connections or providers, canonical absolute
    /// workspaces, and separate paid/write/review/native-policy permissions.
    /// Calling this twice is a conflict; revoke through the runtime's normal
    /// `grants.revoke` call. Shared brokers and private helpers own their grants,
    /// so this returns [`ErrorCode::LocalAccessDenied`] in those modes rather
    /// than bypassing the user's broker approval.
    pub fn authorize_execution(&mut self, grant: ExecutionGrant) -> Result<()> {
        if self.mode != Mode::InProcess {
            return Err(DomainError::new(
                ErrorCode::LocalAccessDenied,
                "Execution grants for a shared broker or private helper are approved by the user and cannot be changed through this client",
            ));
        }
        if !self.execution_enabled {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "Execution drivers were not registered; set StartOptions.execution to true before starting",
            ));
        }
        match &mut self.backend {
            Backend::InProcess { client, .. } => client.authorize_execution(grant),
            Backend::Closed => Err(closed()),
            _ => Err(DomainError::new(
                ErrorCode::LocalAccessDenied,
                "Execution grants cannot be changed through this client",
            )),
        }
    }

    /// Open a session under the caller's existing execution grant.
    pub fn create_session(&mut self, params: SessionCreateParams) -> Result<Session> {
        self.typed_request("sessions.create", params)
    }

    /// Read the current state of a session owned by this application.
    pub fn get_session(&mut self, session_id: &str) -> Result<Session> {
        self.typed_request(
            "sessions.get",
            SessionParams {
                session_id: session_id.to_owned(),
            },
        )
    }

    /// Suspend a resumable session and return its opaque resume token.
    pub fn suspend_session(&mut self, session_id: &str) -> Result<SessionSuspendResult> {
        self.typed_request(
            "sessions.suspend",
            SessionParams {
                session_id: session_id.to_owned(),
            },
        )
    }

    /// Resume a session from a token issued by [`LocalAi::suspend_session`].
    pub fn resume_session(&mut self, params: SessionResumeParams) -> Result<Session> {
        self.typed_request("sessions.resume", params)
    }

    /// Change session options, subject to the same selection validation and grant.
    pub fn update_session_options(&mut self, params: SessionOptionsParams) -> Result<Session> {
        self.typed_request("sessions.updateOptions", params)
    }

    /// Close a session and release its agent. A missing or foreign session is
    /// reported as `NotFound`.
    pub fn close_session(&mut self, session_id: &str) -> Result<()> {
        self.acknowledge(
            "sessions.close",
            SessionParams {
                session_id: session_id.to_owned(),
            },
        )
    }

    /// Start a turn. `request_id` makes retries idempotent at the runtime.
    pub fn start_turn(&mut self, params: TurnStartParams) -> Result<TurnStarted> {
        self.typed_request("turns.start", params)
    }

    /// Cancel a running turn owned by this application.
    pub fn cancel_turn(&mut self, turn_id: &str) -> Result<()> {
        self.acknowledge(
            "turns.cancel",
            TurnParams {
                turn_id: turn_id.to_owned(),
            },
        )
    }

    /// Answer a pending approval. The runtime only accepts approvals it raised
    /// for this application's own session.
    pub fn respond_to_approval(&mut self, approval_id: &str, allow: bool) -> Result<()> {
        self.acknowledge(
            "approvals.respond",
            ApprovalParams {
                approval_id: approval_id.to_owned(),
                allow,
            },
        )
    }

    /// Read a bounded page of session events after an optional cursor.
    pub fn read_events(
        &mut self,
        session_id: Option<&str>,
        after: Option<&str>,
    ) -> Result<EventPage> {
        let mut params = serde_json::Map::new();
        if let Some(session_id) = session_id {
            params.insert("sessionId".into(), Value::String(session_id.to_owned()));
        }
        if let Some(after) = after {
            params.insert("after".into(), Value::String(after.to_owned()));
        }
        self.typed_request("events.read", Value::Object(params))
    }

    /// Start a cursor-based event subscription at the current journal tail.
    ///
    /// This does not open a socket, spawn a task, or start a service: callers
    /// drive [`EventSubscription::poll`] from their own loop. Events published
    /// after this call are returned by later polls.
    pub fn subscribe_events(&mut self, session_id: Option<&str>) -> Result<EventSubscription> {
        let params = match session_id {
            Some(session_id) => json!({"sessionId": session_id}),
            None => json!({}),
        };
        let value = self.call("events.read", params)?;
        let cursor = value
            .get("tailCursor")
            .or_else(|| value.get("cursor"))
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_owned();
        Ok(EventSubscription {
            session_id: session_id.map(str::to_owned),
            cursor,
            closed: false,
        })
    }

    /// Close the client transport. The private helper, when present, is reaped
    /// and later calls return `ProviderUnavailable`. Idempotent.
    ///
    /// In-process close cooperatively cancels background lanes and the active
    /// inventory refresh, then waits at most [`StartOptions::close_timeout`] for
    /// blocking work. A non-cooperative sync inventory callback that ignores its
    /// cancel flag cannot be aborted by Rust; it is detached after the deadline
    /// rather than blocking the host.
    pub fn close(&mut self) {
        let timeout = self.close_timeout;
        let backend = std::mem::replace(&mut self.backend, Backend::Closed);
        match backend {
            Backend::Shared(client) => client.close_with(timeout, None),
            Backend::Child { client, _child } => {
                client.close_with(timeout, None);
                drop(_child);
            }
            Backend::InProcess { client, runtime } => {
                client.stop_background();
                client.close();
                // A sync inventory callback runs on a blocking task that cannot
                // be aborted. Cooperative cancellation usually ends it before
                // this deadline; a non-cooperative callback is detached after
                // the bound instead of holding close indefinitely.
                runtime.shutdown_timeout(timeout);
            }
            Backend::Closed => {}
        }
    }

    fn typed_request<T: DeserializeOwned, P: Serialize>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<T> {
        let params = serde_json::to_value(params)
            .map_err(|_| DomainError::new(ErrorCode::InvalidRequest, "Request is invalid"))?;
        let value = self.call(method, params)?;
        serde_json::from_value(value).map_err(|_| {
            DomainError::new(
                ErrorCode::ProtocolIncompatible,
                "The runtime returned an incompatible response",
            )
        })
    }

    fn acknowledge<P: Serialize>(&mut self, method: &str, params: P) -> Result<()> {
        self.typed_request::<Value, _>(method, params).map(|_| ())
    }
}

/// Cursor-based event subscription returned by [`LocalAi::subscribe_events`].
///
/// No background work runs while this value exists. Call [`Self::poll`] from
/// the host's event loop and stop on an error or after calling [`Self::close`].
#[derive(Debug, Clone)]
pub struct EventSubscription {
    session_id: Option<String>,
    cursor: String,
    closed: bool,
}

impl EventSubscription {
    /// The session filter, or `None` for every session owned by this application.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// The cursor for the next read.
    pub fn cursor(&self) -> &str {
        &self.cursor
    }

    /// Read the next bounded batch. Returns an empty vector when nothing new
    /// has happened; the cursor advances even for empty pages.
    pub fn poll(&mut self, ai: &mut LocalAi) -> Result<Vec<Event>> {
        if self.closed {
            return Err(closed());
        }
        let page = ai.read_events(self.session_id.as_deref(), Some(&self.cursor))?;
        self.cursor = page.cursor;
        Ok(page.events)
    }

    /// Stop polling locally. This does not close the runtime or a session.
    pub fn close(&mut self) {
        self.closed = true;
    }
}

fn closed() -> DomainError {
    DomainError::new(ErrorCode::ProviderUnavailable, "Runtime is closed")
}

fn cancelled() -> DomainError {
    DomainError::new(ErrorCode::Cancelled, "Request was cancelled")
}

async fn request_in_process<T: DeserializeOwned>(
    client: &EmbeddedClient,
    method: &str,
    params: Value,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<T> {
    tokio::select! {
        biased;
        () = cancel.wait() => Err(cancelled()),
        result = tokio::time::timeout(timeout, client.request::<T>(method, params)) => {
            result.map_err(|_| DomainError::new(ErrorCode::Timeout, "Request exceeded its deadline"))?
        }
    }
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

    fn fixture() -> Snapshot {
        let mut snapshot = Snapshot::empty(originlm_contracts::DataMode::Synthetic);
        snapshot.providers.push(originlm_contracts::Provider {
            id: "ollama".into(),
            name: "Ollama".into(),
            kind: "local".into(),
        });
        snapshot
            .installations
            .push(originlm_contracts::Installation {
                id: "install-local".into(),
                provider_id: "ollama".into(),
                environment_id: "synthetic".into(),
                location_hint: "synthetic".into(),
                version: originlm_contracts::Observation::unknown("NOT_PROBED"),
            });
        snapshot.profiles.push(originlm_contracts::Profile {
            id: "profile-local".into(),
            installation_id: "install-local".into(),
            label: "local".into(),
        });
        snapshot.connections.push(originlm_contracts::Connection {
            id: "local".into(),
            provider_id: "ollama".into(),
            installation_id: "install-local".into(),
            profile_id: "profile-local".into(),
            display_name: "Local".into(),
            account_id: None,
            auth: originlm_contracts::Observation::unknown("LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH"),
            plan: originlm_contracts::Observation::unknown("NOT_REPORTED"),
            status: "ok".into(),
            diagnostics: vec![],
        });
        snapshot.routes.push(originlm_contracts::AccessRoute {
            id: "route-local".into(),
            connection_id: "local".into(),
            provider_id: "ollama".into(),
            display_name: "Local".into(),
            billing: originlm_contracts::Observation::reported(
                originlm_contracts::BillingMode::Local,
                originlm_contracts::EvidenceSource::new(
                    originlm_contracts::EvidenceKind::Synthetic,
                    "test",
                    "synthetic local route",
                ),
            ),
            capabilities: vec![],
            catalog_revision: "1".into(),
        });
        snapshot.models.push(originlm_contracts::Model {
            id: "model-local".into(),
            route_id: "route-local".into(),
            native_id: "local".into(),
            name: "Local".into(),
            revision: "1".into(),
            is_default: true,
            capabilities: vec![originlm_contracts::Capability {
                id: "generation.text".into(),
                support: originlm_contracts::Support::Supported,
                origin: originlm_contracts::CapabilityOrigin::Model,
                evidence: originlm_contracts::Observation::reported(
                    "synthetic".into(),
                    originlm_contracts::EvidenceSource::new(
                        originlm_contracts::EvidenceKind::Synthetic,
                        "test",
                        "synthetic model capability",
                    ),
                ),
                conditions: vec![],
            }],
            options: vec![],
            context_window: originlm_contracts::Observation::unknown("NOT_REPORTED"),
        });
        snapshot
    }

    fn session_params() -> SessionCreateParams {
        SessionCreateParams {
            selection: Selection {
                route_id: "route-local".into(),
                model_id: "model-local".into(),
                revision: "1".into(),
                options: Default::default(),
            },
            workspace_id: "scratch".into(),
            permission_mode: None,
            review_requests: false,
            allow_paid: false,
            host_mcp_servers: vec![],
            persistent: false,
        }
    }

    fn in_process(execution: bool) -> (LocalAi, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("scratch");
        std::fs::create_dir(&workspace).unwrap();
        let _workspace = workspace.canonicalize().unwrap();
        let snapshot = fixture();
        let locations = std::collections::BTreeMap::from([(
            "local".to_string(),
            originlm_discovery::LocatedClient {
                provider_id: "ollama".into(),
                profile_home: directory.path().join("profile"),
                executable: None,
                executable_identity: None,
            },
        )]);
        let inventory = std::sync::Arc::new(move |_cancel: &std::sync::atomic::AtomicBool| {
            Ok(originlm_discovery::ScanResult {
                snapshot: snapshot.clone(),
                locations: locations.clone(),
                metadata_probes: 0,
            })
        });
        let kit = Kit::builder()
            .inventory(inventory)
            .execution(execution)
            .build()
            .unwrap();
        let client = kit
            .connect("originlm-execution-test", ClientPolicy::passive())
            .unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        (
            LocalAi {
                mode: Mode::InProcess,
                fallback_reason: None,
                execution_enabled: execution,
                request_timeout: DEFAULT_REQUEST_TIMEOUT,
                close_timeout: DEFAULT_CLOSE_TIMEOUT,
                backend: Backend::InProcess { client, runtime },
            },
            directory,
        )
    }

    #[test]
    fn execution_requires_an_explicit_grant() {
        let (mut ai, directory) = in_process(true);
        let error = ai.create_session(session_params()).unwrap_err();
        assert_eq!(error.code, ErrorCode::LocalAccessDenied);
        // An explicit grant is the only thing that moves the boundary.
        ai.authorize_execution(ExecutionGrant {
            connection_ids: ["local".to_string()].into_iter().collect(),
            workspaces: [(
                "scratch".to_string(),
                directory.path().join("scratch").canonicalize().unwrap(),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        })
        .unwrap();
        ai.scan().unwrap();
        let session = ai.create_session(session_params()).unwrap();
        assert_eq!(session.connection_id, "local");
        assert_eq!(session.selection.route_id, "route-local");
        assert_eq!(ai.get_session(&session.id).unwrap().id, session.id);
        let mut events = ai.subscribe_events(Some(&session.id)).unwrap();
        assert!(events.poll(&mut ai).unwrap().is_empty());
        ai.close_session(&session.id).unwrap();
    }

    #[test]
    fn execution_requires_explicit_in_process_mode() {
        let error = match LocalAi::start_with(StartOptions {
            execution: true,
            ..StartOptions::default()
        }) {
            Ok(_) => panic!("execution must require an explicit in-process mode"),
            Err(error) => error,
        };
        assert_eq!(error.code, ErrorCode::InvalidRequest);
    }

    #[test]
    fn execution_grants_are_explicit_and_registration_must_be_opted_in() {
        let (mut disabled, directory) = in_process(false);
        let error = disabled
            .authorize_execution(ExecutionGrant {
                connection_ids: ["local".to_string()].into_iter().collect(),
                workspaces: [(
                    "scratch".to_string(),
                    directory.path().join("scratch").canonicalize().unwrap(),
                )]
                .into_iter()
                .collect(),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest);

        let mut shared = LocalAi {
            mode: Mode::Shared,
            fallback_reason: None,
            execution_enabled: false,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            close_timeout: DEFAULT_CLOSE_TIMEOUT,
            backend: Backend::Closed,
        };
        let error = shared
            .authorize_execution(ExecutionGrant::default())
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::LocalAccessDenied);
    }

    #[test]
    fn event_subscriptions_are_cursor_based_and_do_not_spawn_services() {
        let (mut ai, _directory) = in_process(true);
        let mut subscription = ai.subscribe_events(None).unwrap();
        assert!(subscription.poll(&mut ai).unwrap().is_empty());
        subscription.close();
        assert_eq!(
            subscription.poll(&mut ai).unwrap_err().code,
            ErrorCode::ProviderUnavailable
        );
        ai.close();
        ai.close();
        assert_eq!(
            ai.snapshot().unwrap_err().code,
            ErrorCode::ProviderUnavailable
        );
    }

    fn write_grant(
        path: &Path,
        app_id: Option<&str>,
        scope: &str,
        independent: bool,
        protocol: &str,
        address: &str,
    ) {
        let token = "b".repeat(64);
        let mut descriptor = json!({
            "token": token,
            "address": address,
            "protocolVersion": protocol,
            "scope": scope,
            "independent": independent,
        });
        if let Some(app_id) = app_id {
            descriptor["appId"] = Value::String(app_id.into());
        }
        std::fs::write(path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    fn inventory(snapshot: Snapshot, delay: Duration) -> InventorySource {
        Arc::new(move |_cancel| {
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            Ok(originlm_discovery::ScanResult {
                snapshot: snapshot.clone(),
                locations: Default::default(),
                metadata_probes: 0,
            })
        })
    }

    fn cooperative_inventory(snapshot: Snapshot) -> InventorySource {
        Arc::new(move |cancel| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(originlm_discovery::ScanResult {
                snapshot: snapshot.clone(),
                locations: Default::default(),
                metadata_probes: 0,
            })
        })
    }

    #[test]
    fn shared_grants_require_application_identity_and_owner_only_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("token.json");
        let address = "/tmp/example.sock";
        write_grant(&path, Some("haiwire"), "application", true, "1.0", address);
        assert!(read_shared_grant(&path, address, Some("haiwire")).is_ok());
        assert!(read_shared_grant(&path, address, Some("other")).is_err());
        assert!(read_shared_grant(&path, "/tmp/other.sock", Some("haiwire")).is_err());

        write_grant(&path, Some("haiwire"), "owner", true, "1.0", address);
        assert!(read_shared_grant(&path, address, None).is_err());

        write_grant(&path, None, "application", true, "1.0", address);
        assert!(read_shared_grant(&path, address, None).is_err());

        write_grant(&path, Some("haiwire"), "application", false, "1.0", address);
        assert!(read_shared_grant(&path, address, None).is_err());

        write_grant(&path, Some("haiwire"), "application", true, "2.0", address);
        assert!(read_shared_grant(&path, address, None).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            write_grant(&path, Some("haiwire"), "application", true, "1.0", address);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(read_shared_grant(&path, address, Some("haiwire")).is_err());
        }
    }

    #[test]
    fn explicit_shared_grant_failure_never_falls_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("token.json");
        let address = directory.path().join("broker.sock");
        write_grant(
            &path,
            Some("haiwire"),
            "owner",
            true,
            "1.0",
            &address.to_string_lossy(),
        );
        let outcome = LocalAi::start_with(StartOptions {
            address: Some(address.to_string_lossy().into_owned()),
            token_file: Some(path.clone()),
            allow_in_process: true,
            ..StartOptions::default()
        });
        assert!(
            outcome.is_err(),
            "owner-scoped explicit grants must fail closed"
        );

        write_grant(
            &path,
            Some("haiwire"),
            "application",
            true,
            "1.0",
            &address.to_string_lossy(),
        );
        let missing_identity = LocalAi::start_with(StartOptions {
            address: Some(address.to_string_lossy().into_owned()),
            token_file: Some(path),
            required_mode: Some(Mode::Shared),
            allow_in_process: true,
            ..StartOptions::default()
        });
        assert!(missing_identity.is_err());
    }

    #[test]
    fn required_in_process_bypasses_helpers_and_default_token_discovery() {
        let directory = tempfile::tempdir().unwrap();
        let ai = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::InProcess),
            helper: Some(directory.path().join("missing-helper")),
            token_file: Some(directory.path().join("missing-token.json")),
            home: Some(directory.path().to_path_buf()),
            inventory: Some(inventory(fixture(), Duration::ZERO)),
            ..StartOptions::default()
        })
        .unwrap();
        assert_eq!(ai.mode(), Mode::InProcess);
        let mut ai = ai;
        assert_eq!(ai.scan().unwrap().providers[0].id, "ollama");
        ai.close();

        let invalid_label = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::InProcess),
            app_label: Some(String::new()),
            ..StartOptions::default()
        });
        assert!(invalid_label.is_err());
    }

    #[test]
    fn child_policy_must_be_exact_or_rejected_before_spawn() {
        assert!(validate_child_policy(&ClientPolicy::passive()).is_ok());
        assert!(validate_child_policy(&ClientPolicy::denied()).is_ok());
        let mut probes = ClientPolicy::passive();
        probes.probe_provider_ids.insert("codex".into());
        assert!(validate_child_policy(&probes).is_ok());

        let mut narrowed = ClientPolicy::passive();
        narrowed.connection_ids = Some(["local".to_string()].into_iter().collect());
        assert!(validate_child_policy(&narrowed).is_err());
        let mut probe_connections = ClientPolicy::passive();
        probe_connections
            .probe_connection_ids
            .insert("local".into());
        assert!(validate_child_policy(&probe_connections).is_err());
        let mut identity = ClientPolicy::passive();
        identity.reveal_identity = true;
        assert!(validate_child_policy(&identity).is_err());
        let mut admin = ClientPolicy::passive();
        admin.admin = true;
        assert!(validate_child_policy(&admin).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let directory = tempfile::tempdir().unwrap();
            let helper = directory.path().join("helper");
            let marker = directory.path().join("spawned");
            std::fs::write(
                &helper,
                format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
            )
            .unwrap();
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
            let outcome = LocalAi::start_with(StartOptions {
                required_mode: Some(Mode::Child),
                helper: Some(helper),
                policy: narrowed,
                ..StartOptions::default()
            });
            assert!(outcome.is_err());
            assert!(
                !marker.exists(),
                "unsupported policy must be rejected before the helper starts"
            );
        }
    }

    #[test]
    fn embedded_calls_have_bounded_deadlines_and_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let mut ai = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::InProcess),
            home: Some(directory.path().to_path_buf()),
            inventory: Some(inventory(fixture(), Duration::from_millis(300))),
            request_timeout: Duration::from_millis(50),
            ..StartOptions::default()
        })
        .unwrap();
        let error = ai.scan().unwrap_err();
        assert_eq!(error.code, ErrorCode::Timeout);

        let directory = tempfile::tempdir().unwrap();
        let mut ai = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::InProcess),
            home: Some(directory.path().to_path_buf()),
            inventory: Some(inventory(fixture(), Duration::from_millis(300))),
            ..StartOptions::default()
        })
        .unwrap();
        let cancel = CancelToken::new();
        let signal = cancel.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            signal.cancel();
        });
        let started = Instant::now();
        let error = ai
            .call_cancellable("discovery.refresh", json!({"level":"passive"}), &cancel)
            .unwrap_err();
        stopper.join().unwrap();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(started.elapsed() < Duration::from_millis(250));
        ai.close();
    }

    #[test]
    fn embedded_only_configuration_is_rejected_before_helper_resolution() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let directory = tempfile::tempdir().unwrap();
            let helper = directory.path().join("helper");
            let marker = directory.path().join("spawned");
            std::fs::write(
                &helper,
                format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
            )
            .unwrap();
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
            let outcome = LocalAi::start_with(StartOptions {
                helper: Some(helper),
                home: Some(directory.path().join("account-home")),
                ..StartOptions::default()
            });
            assert!(outcome.is_err());
            assert!(
                !marker.exists(),
                "scoped embedded configuration must be rejected before a helper starts"
            );
        }

        let mut native = NativeClientRegistry::default();
        native.profiles.insert(
            "fixture".into(),
            NativeProfile {
                id: "fixture".into(),
                label: "Fixture".into(),
                provider: NativeProvider::GeminiCli,
                executable: PathBuf::from("/bin/false"),
                profile_home: PathBuf::from("/tmp/originlm-fixture-home"),
                auth_method: None,
                billing: None,
            },
        );
        let child = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::Child),
            helper: Some(PathBuf::from("/definitely/not/a/helper")),
            native_clients: native,
            ..StartOptions::default()
        });
        assert_eq!(
            child.err().map(|error| error.code),
            Some(ErrorCode::InvalidRequest)
        );

        let shared = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::Shared),
            app_label: Some("haiwire".into()),
            ..StartOptions::default()
        });
        assert_eq!(
            shared.err().map(|error| error.code),
            Some(ErrorCode::InvalidRequest)
        );
    }

    #[test]
    fn embedded_close_signals_cooperative_inventory() {
        let directory = tempfile::tempdir().unwrap();
        let mut ai = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::InProcess),
            home: Some(directory.path().to_path_buf()),
            inventory: Some(cooperative_inventory(fixture())),
            close_timeout: Duration::from_millis(200),
            ..StartOptions::default()
        })
        .unwrap();
        let cancel = CancelToken::new();
        let signal = cancel.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            signal.cancel();
        });
        let call_started = Instant::now();
        let error = ai
            .call_cancellable("discovery.refresh", json!({"level":"passive"}), &cancel)
            .unwrap_err();
        let call_elapsed = call_started.elapsed();
        stopper.join().unwrap();
        assert_eq!(error.code, ErrorCode::Cancelled);

        let close_started = Instant::now();
        ai.close();
        let close_elapsed = close_started.elapsed();
        assert!(call_elapsed < Duration::from_millis(500));
        assert!(
            close_elapsed < Duration::from_millis(500),
            "cooperative inventory should finish inside the close bound: {close_elapsed:?}"
        );
    }

    #[test]
    fn embedded_close_deadline_detaches_noncooperative_inventory() {
        let directory = tempfile::tempdir().unwrap();
        let mut ai = LocalAi::start_with(StartOptions {
            required_mode: Some(Mode::InProcess),
            home: Some(directory.path().to_path_buf()),
            inventory: Some(inventory(fixture(), Duration::from_millis(1500))),
            close_timeout: Duration::from_millis(100),
            ..StartOptions::default()
        })
        .unwrap();
        let cancel = CancelToken::new();
        let signal = cancel.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            signal.cancel();
        });
        let error = ai
            .call_cancellable("discovery.refresh", json!({"level":"passive"}), &cancel)
            .unwrap_err();
        stopper.join().unwrap();
        assert_eq!(error.code, ErrorCode::Cancelled);

        let close_started = Instant::now();
        ai.close();
        let close_elapsed = close_started.elapsed();
        assert!(
            close_elapsed < Duration::from_millis(700),
            "non-cooperative inventory must not extend close past its deadline: {close_elapsed:?}"
        );
    }
}
