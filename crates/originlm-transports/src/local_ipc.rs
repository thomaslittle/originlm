//! Shared local broker. OS endpoint permissions and one-use host grants are separate boundaries.
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{
    ListenerOptions,
    tokio::{Stream, prelude::*},
};
use originlm_contracts::*;
use originlm_core::ClientPolicy;
use originlm_service::{ExecutionGrant, NativeService};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{Semaphore, oneshot},
    task::JoinSet,
};
/// Public stream type for SDK/control consumers that implement the local
/// connector themselves. It carries the same authenticated framing.
pub type IpcStream = Stream;
#[derive(Clone)]
struct Grant {
    policy: ClientPolicy,
    execution: Option<ExecutionGrant>,
    created: Instant,
    /// An independent broker may reuse an app credential so the same
    /// application can open a new connection after a restart. Parent
    /// bootstraps remain one-use.
    reusable: bool,
    /// Stable application identity for an independently scoped grant. The
    /// administrator can revoke the credential and all of its live sockets.
    app_id: Option<String>,
}
#[derive(Clone, Copy, Default)]
pub struct BrokerOptions {
    /// Stop after this long with no connected clients. None means run until an
    /// explicit shutdown or parent control-channel close.
    pub idle_timeout: Option<Duration>,
}
struct Shared {
    service: Arc<NativeService>,
    pending: Mutex<BTreeMap<[u8; 32], Grant>>,
    active: AtomicUsize,
    idle_since: Mutex<Option<Instant>>,
}
impl Shared {
    /// Remove every reusable credential for one application. The caller also
    /// sweeps live connections; this method is what prevents reconnection.
    fn revoke_app(&self, app_id: &str) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        let before = pending.len();
        pending.retain(|_, grant| grant.app_id.as_deref() != Some(app_id));
        pending.len() != before
    }
}
pub struct Broker {
    shared: Arc<Shared>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
fn fail(code: ErrorCode) -> DomainError {
    DomainError::new(code, "Local broker operation failed")
}
fn hash(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}
fn key() -> Result<String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|_| fail(ErrorCode::ProviderUnavailable))?;
    Ok(b.iter().map(|b| format!("{b:02x}")).collect())
}
fn name(value: &str) -> Result<interprocess::local_socket::Name<'_>> {
    if value.is_empty()
        || value.len() > 200
        || value.chars().any(char::is_control)
        || value.contains("://")
        || value.starts_with("//")
        || value.starts_with(r"\\")
    {
        return Err(fail(ErrorCode::InvalidRequest));
    }
    #[cfg(windows)]
    {
        if !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(fail(ErrorCode::InvalidRequest));
        }
        value
            .to_ns_name::<GenericNamespaced>()
            .map_err(|_| fail(ErrorCode::InvalidRequest))
    }
    #[cfg(unix)]
    {
        if !std::path::Path::new(value).is_absolute() {
            return Err(fail(ErrorCode::InvalidRequest));
        }
        value
            .to_fs_name::<GenericFilePath>()
            .map_err(|_| fail(ErrorCode::InvalidRequest))
    }
}
/// The listener builder cannot set a socket mode on every Unix (macOS reports it unsupported), so
/// those platforms restrict the endpoint to its owner right after binding and fail closed if they
/// cannot. A connection also needs a one-use host grant, so the instant before this call exposes
/// no usable access.
#[cfg(unix)]
fn restrict_endpoint(address: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(address, std::fs::Permissions::from_mode(0o600)).map_err(|_| {
        let _ = std::fs::remove_file(address);
        fail(ErrorCode::LocalAccessDenied)
    })
}
impl Broker {
    pub async fn start(service: Arc<NativeService>, address: &str) -> Result<Self> {
        Self::start_with(service, address, BrokerOptions::default()).await
    }
    pub async fn start_with(
        service: Arc<NativeService>,
        address: &str,
        broker_options: BrokerOptions,
    ) -> Result<Self> {
        if broker_options
            .idle_timeout
            .is_some_and(|timeout| timeout.is_zero() || timeout > Duration::from_secs(86_400))
        {
            return Err(fail(ErrorCode::InvalidRequest));
        }
        let options = ListenerOptions::new().name(name(address)?);
        #[cfg(windows)]
        let options = {
            use interprocess::os::windows::{
                local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
            };
            let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;OW)")
                .map_err(|_| fail(ErrorCode::LocalAccessDenied))?;
            options.security_descriptor(
                SecurityDescriptor::deserialize(&sddl)
                    .map_err(|_| fail(ErrorCode::LocalAccessDenied))?,
            )
        };
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let options = {
            use interprocess::os::unix::local_socket::ListenerOptionsExt;
            options.mode(0o600)
        };
        let listener = match options.create_tokio() {
            Ok(listener) => listener,
            Err(_) => {
                #[cfg(unix)]
                {
                    // Distinguish a live owner from a stale socket left by a
                    // crash. Never unlink an endpoint that still accepts a
                    // connection; just report a conflict.
                    if Stream::connect(name(address)?).await.is_ok() {
                        return Err(fail(ErrorCode::Conflict));
                    }
                    use std::os::unix::fs::FileTypeExt;
                    let path = std::path::Path::new(address);
                    if path
                        .symlink_metadata()
                        .map(|metadata| metadata.file_type().is_socket())
                        .unwrap_or(false)
                    {
                        std::fs::remove_file(path).map_err(|_| fail(ErrorCode::Conflict))?;
                    }
                    ListenerOptions::new()
                        .name(name(address)?)
                        .create_tokio()
                        .map_err(|_| fail(ErrorCode::Conflict))?
                }
                #[cfg(windows)]
                {
                    return Err(fail(ErrorCode::Conflict));
                }
            }
        };
        #[cfg(unix)]
        restrict_endpoint(address)?;
        let shared = Arc::new(Shared {
            service,
            pending: Mutex::new(BTreeMap::new()),
            active: AtomicUsize::new(0),
            idle_since: Mutex::new(Some(Instant::now())),
        });
        let revoker_state = shared.clone();
        shared
            .service
            .set_app_revoker(Arc::new(move |app_id| revoker_state.revoke_app(app_id)));
        let state = shared.clone();
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let permits = Arc::new(Semaphore::new(32));
            let mut jobs = JoinSet::new();
            let mut idle = tokio::time::interval(Duration::from_secs(1));
            idle.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                 _=&mut stopped=>break,
                 _=idle.tick(),if broker_options.idle_timeout.is_some()=>{
                     if state.active.load(Ordering::SeqCst)==0
                        && state.idle_since.lock().ok().and_then(|since| *since).is_some_and(|since| since.elapsed()>=broker_options.idle_timeout.unwrap_or_default()) {
                         break;
                     }
                 },
                 _=jobs.join_next(),if !jobs.is_empty()=>{},
                 incoming=listener.accept()=>match incoming{
                  Ok(stream)=>{if let Ok(permit)=permits.clone().try_acquire_owned(){let state=state.clone();jobs.spawn(async move{let _p=permit;let _=serve_connection(stream,state).await;});}},
                  Err(_)=>break,
                 }
                }
            }
            jobs.abort_all();
            while jobs.join_next().await.is_some() {}
        });
        Ok(Self {
            shared,
            stop: Some(stop),
            task: Some(task),
        })
    }
    /// Pass this one-use credential over the embedding application's trusted channel, never logs or argv.
    pub fn authorize(
        &self,
        policy: ClientPolicy,
        execution: Option<ExecutionGrant>,
    ) -> Result<String> {
        self.insert_grant(None, policy, execution, false)
    }
    /// Owner-scoped credential for an independent broker's token file. It can
    /// open more than one connection; the OS file permissions are the boundary.
    pub fn authorize_shared(
        &self,
        policy: ClientPolicy,
        execution: Option<ExecutionGrant>,
    ) -> Result<String> {
        self.insert_grant(None, policy, execution, true)
    }
    /// Independently scoped credential for one application. The stable app id
    /// is the revocation and account-isolation identity.
    pub fn authorize_shared_for(
        &self,
        app_id: &str,
        policy: ClientPolicy,
        execution: Option<ExecutionGrant>,
    ) -> Result<String> {
        if app_id.is_empty()
            || app_id.len() > 128
            || app_id == "ipc-host"
            || app_id.chars().any(char::is_control)
            || !app_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(fail(ErrorCode::InvalidRequest));
        }
        self.insert_grant(Some(app_id.to_owned()), policy, execution, true)
    }
    fn insert_grant(
        &self,
        app_id: Option<String>,
        policy: ClientPolicy,
        execution: Option<ExecutionGrant>,
        reusable: bool,
    ) -> Result<String> {
        let token = key()?;
        let mut map = self
            .shared
            .pending
            .lock()
            .map_err(|_| fail(ErrorCode::ProviderUnavailable))?;
        map.retain(|_, grant| grant.reusable || grant.created.elapsed() < Duration::from_secs(300));
        if map.len() >= 64 {
            return Err(fail(ErrorCode::RateLimited));
        }
        map.insert(
            hash(&token),
            Grant {
                policy,
                execution,
                created: Instant::now(),
                reusable,
                app_id,
            },
        );
        Ok(token)
    }
    pub async fn close(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
    pub fn active_connections(&self) -> usize {
        self.shared.active.load(Ordering::SeqCst)
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
struct Owner {
    service: Arc<NativeService>,
    client: String,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.service.unregister_client(&self.client);
    }
}
struct ActiveConnection {
    shared: Arc<Shared>,
}
impl ActiveConnection {
    fn new(shared: Arc<Shared>) -> Self {
        shared.active.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut idle) = shared.idle_since.lock() {
            *idle = None;
        }
        Self { shared }
    }
}
impl Drop for ActiveConnection {
    fn drop(&mut self) {
        if self.shared.active.fetch_sub(1, Ordering::SeqCst) == 1
            && let Ok(mut idle) = self.shared.idle_since.lock()
        {
            *idle = Some(Instant::now());
        }
    }
}
async fn serve_connection(stream: Stream, state: Arc<Shared>) -> Result<()> {
    let mut stream = BufReader::new(stream);
    let mut bytes = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        (&mut stream).take(4097).read_until(b'\n', &mut bytes),
    )
    .await
    .map_err(|_| fail(ErrorCode::Timeout))?
    .map_err(|_| fail(ErrorCode::ProviderUnavailable))?;
    let request = crate::wire::parse(&bytes).ok().flatten();
    let id = request
        .as_ref()
        .map(|r| r.id.clone())
        .unwrap_or(Value::Null);
    let token = request
        .as_ref()
        .filter(|r| {
            r.method == "broker.authenticate" && r.params.as_object().is_some_and(|p| p.len() == 1)
        })
        .and_then(|r| r.params["token"].as_str())
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()));
    let grant = token.and_then(|token| {
        let mut pending = state.pending.lock().ok()?;
        let grant = pending.get(&hash(token)).cloned()?;
        if !grant.reusable {
            pending.remove(&hash(token));
        }
        Some(grant)
    });
    let Some(grant) =
        grant.filter(|grant| grant.reusable || grant.created.elapsed() < Duration::from_secs(300))
    else {
        let value = crate::wire::reply(id, Err(fail(ErrorCode::LocalAccessDenied)));
        let _ = stream
            .get_mut()
            .write_all(format!("{value}\n").as_bytes())
            .await;
        return Err(fail(ErrorCode::LocalAccessDenied));
    };
    if bytes.len() > 4096 || !bytes.ends_with(b"\n") {
        return Err(fail(ErrorCode::InvalidRequest));
    }
    let client = format!("ipc-{}", &key()?[..32]);
    state.service.register_client(&client, grant.policy)?;
    state
        .service
        .set_app_identity(&client, grant.app_id.as_deref().unwrap_or("ipc-host"))?;
    let _owner = Owner {
        service: state.service.clone(),
        client: client.clone(),
    };
    let _active = ActiveConnection::new(state.clone());
    if let Some(grant) = grant.execution {
        state.service.register_execution(&client, grant)?;
    }
    stream
        .get_mut()
        .write_all(
            format!(
                "{}\n",
                json!({"jsonrpc":"2.0","id":id,"result":{"authenticated":true}})
            )
            .as_bytes(),
        )
        .await
        .map_err(|_| fail(ErrorCode::ProviderUnavailable))?;
    let (reader, writer) = tokio::io::split(stream);
    crate::serve_io(
        reader,
        writer,
        state.service.clone(),
        client,
        Default::default(),
    )
    .await
}
/// Explicit connector used by SDK bridge processes. No daemon is implicitly started.
pub async fn connect(address: &str) -> Result<Stream> {
    Stream::connect(name(address)?)
        .await
        .map_err(|_| fail(ErrorCode::RuntimeMissing))
}
