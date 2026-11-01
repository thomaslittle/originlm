//! First-run credentials for the desktop control center.
//!
//! A control center must open on real state without the user hunting for a
//! token file. The app prefers a broker that is already running for this user.
//! When no descriptor is reachable it hosts a private broker in-process and
//! mints one reusable administrative credential for its two workers. That
//! credential exists only in memory and dies with the process; nothing is
//! written to disk. The private runtime runs passive discovery and read-only
//! metadata probes for discovered connections, and keeps them fresh on its
//! periodic background lanes for as long as the app is open.

use originlm_control::BrokerClient;
use originlm_kit::{ClientPolicy, ExecutionGrant, InventorySource, Kit};
use originlm_transports::local_ipc::{Broker, BrokerOptions};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PROBE_TIMEOUT: Duration = Duration::from_millis(750);
const START_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_DESCRIPTOR: usize = 16 * 1024;

/// How this window reaches its broker.
#[derive(Clone, Debug)]
pub enum Credential {
    /// An explicit `--token-file` or a descriptor already issued to this user.
    Descriptor(PathBuf),
    /// A private broker hosted by this process.
    Local { address: String, token: String },
    /// No broker is available and the local one could not start.
    Unavailable(String),
}

impl Credential {
    /// Resolve a credential before the window opens.
    ///
    /// An explicit descriptor fails closed: a missing or malformed file is
    /// reported instead of silently substituting a different runtime. Without
    /// one, existing owner/admin descriptors are probed in order, and a private
    /// in-process broker is the final fallback.
    pub fn acquire(explicit: Option<PathBuf>) -> Self {
        if let Some(path) = explicit {
            return Self::Descriptor(path);
        }
        for path in candidate_descriptors() {
            if !path.is_file() {
                continue;
            }
            let Ok((address, token)) = read_descriptor(&path) else {
                continue;
            };
            match BrokerClient::connect_with_timeout(&address, &token, PROBE_TIMEOUT) {
                Ok(client) => {
                    client.close();
                    return Self::Descriptor(path);
                }
                Err(_) => continue,
            }
        }
        match start_local() {
            Ok((address, token)) => Self::Local { address, token },
            Err(error) => Self::Unavailable(error),
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local { .. })
    }

    pub fn connect(&self) -> Result<BrokerClient, String> {
        match self {
            Self::Descriptor(path) => BrokerClient::from_token_file(path, None),
            Self::Local { address, token } => BrokerClient::connect(address, token),
            Self::Unavailable(error) => Err(error.clone()),
        }
    }

    /// Human-readable source for the connection banner and diagnostics.
    pub fn label(&self) -> String {
        match self {
            Self::Descriptor(path) => format!("token file {}", path.display()),
            Self::Local { .. } => "the built-in local runtime".into(),
            Self::Unavailable(_) => "the built-in local runtime".into(),
        }
    }

    /// Broker endpoint for Settings and diagnostics.
    pub fn address(&self) -> String {
        match self {
            Self::Descriptor(path) => read_descriptor(path)
                .map(|(address, _)| address)
                .unwrap_or_else(|_| "unavailable".into()),
            Self::Local { address, .. } => address.clone(),
            Self::Unavailable(_) => "unavailable".into(),
        }
    }

    /// Where the credential came from, for Settings.
    pub fn descriptor_label(&self) -> String {
        match self {
            Self::Descriptor(path) => path.display().to_string(),
            Self::Local { .. } => "In-memory credential issued by this app".into(),
            Self::Unavailable(_) => "No credential".into(),
        }
    }

    /// One sentence that tells the user what to try next.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            Self::Local { .. } | Self::Unavailable(_) => {
                "Restart the app, or start the shared broker and pass --token-file."
            }
            Self::Descriptor(_) => {
                "Check that the shared broker is running and the token file is owner-only, or pass another --token-file."
            }
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// Descriptors already issued on this machine, most privileged first. The
/// platform state directory is authoritative; the legacy home directory and
/// the documented control-center path remain readable for existing installs.
fn candidate_descriptors() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(directory) = originlm_platform::default_state_dir(None)
        && directory.is_absolute()
    {
        roots.push(directory);
    }
    if let Some(home) = home_dir() {
        roots.push(home.join(".originlm"));
        roots.push(home.join(".local/state/originlm"));
        roots.push(home.join(".local/state/originlm/fedora-shared"));
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    for root in roots {
        for name in [
            "broker-admin-token.json",
            "broker-token.json",
            "control-admin-token.json",
            "control-token.json",
        ] {
            let path = root.join(name);
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

fn read_descriptor(path: &Path) -> Result<(String, String), String> {
    let bytes = originlm_platform::read_private_file(path, MAX_DESCRIPTOR)
        .map_err(|_| "Token file must be a readable, bounded owner-only regular file".to_owned())?;
    let descriptor: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Token file is malformed".to_owned())?;
    let address = descriptor["address"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Token file has no address".to_owned())?;
    let token = descriptor["token"]
        .as_str()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| "Token file has no token".to_owned())?;
    Ok((address.to_owned(), token.to_owned()))
}

/// The Playground runs one explicit turn at a time in a dedicated, empty
/// workspace. It never receives the user's home or a project directory.
fn workspace_dir() -> Option<PathBuf> {
    let base = originlm_platform::default_state_dir(None)
        .unwrap_or_else(|| std::env::temp_dir().join("originlm"));
    let path = base.join("playground");
    std::fs::create_dir_all(&path).ok()?;
    path.canonicalize().ok()
}

fn local_address() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    if cfg!(windows) {
        return format!("originlm-desktop-{}-{nanos}", std::process::id());
    }
    let name = format!("originlm-desktop-{}-{nanos}.sock", std::process::id());
    let candidate = std::env::temp_dir().join(&name);
    // Unix socket paths are much shorter than ordinary filesystem paths.
    let path = if candidate.as_os_str().len() <= 100 {
        candidate
    } else {
        PathBuf::from("/tmp").join(name)
    };
    path.to_string_lossy().into_owned()
}

/// Where the private runtime reads inventory and keeps state.
struct LocalRuntime {
    /// `None` scans this machine; tests inject fixture discovery instead.
    inventory: Option<InventorySource>,
    state_dir: Option<PathBuf>,
    workspace: Option<PathBuf>,
}

impl LocalRuntime {
    fn host() -> Self {
        Self {
            inventory: None,
            state_dir: originlm_platform::default_state_dir(None).filter(|path| path.is_absolute()),
            workspace: workspace_dir(),
        }
    }
}

/// Host a private broker for this window. The runtime thread owns the broker
/// for the process lifetime; dropping the app ends the thread.
fn start_local() -> Result<(String, String), String> {
    start_local_with(LocalRuntime::host())
}

fn start_local_with(config: LocalRuntime) -> Result<(String, String), String> {
    let LocalRuntime {
        inventory,
        state_dir: state,
        workspace,
    } = config;
    let address = local_address();
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("originlm-desktop-runtime".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = sender.send(Err(
                        "The built-in local runtime could not start a thread.".to_owned(),
                    ));
                    return;
                }
            };
            runtime.block_on(async move {
                let mut builder = Kit::builder().execution(true);
                if let Some(state) = state {
                    builder = builder.state_dir(state);
                }
                if let Some(inventory) = inventory {
                    builder = builder.inventory(inventory);
                }
                let kit = match builder.build() {
                    Ok(kit) => kit,
                    Err(_) => {
                        let _ =
                            sender
                                .send(Err("The built-in local runtime could not be constructed."
                                    .to_owned()));
                        return;
                    }
                };
                let mut policy = ClientPolicy::passive();
                policy.admin = true;
                // The private runtime exists to show this machine's AI. Allow
                // read-only metadata probes for the providers discovery knows
                // about; execution, identity reveal and paid options stay off.
                policy.probe_provider_ids = originlm_discovery::definitions()
                    .iter()
                    .map(|definition| definition.id.to_owned())
                    .collect();
                let service = kit.service();
                // Keep the verified state fresh: the periodic background lanes
                // repeat passive inventory and the same read-only metadata grants
                // the window uses for its startup probes. Without them, auth
                // evidence goes stale after the provider freshness window and
                // the UI falls back to 0 verified.
                service.enable_inventory_refresh(true);
                service.enable_autonomous_refresh(policy.probe_provider_ids.clone());
                service.start_background_tasks();
                let broker = match Broker::start_with(service, &address, BrokerOptions::default())
                    .await
                {
                    Ok(broker) => broker,
                    Err(_) => {
                        let _ = sender.send(Err(
                            "The built-in local runtime could not open its endpoint.".to_owned(),
                        ));
                        return;
                    }
                };
                let execution = workspace.map(|workspace| ExecutionGrant {
                    provider_ids: policy.probe_provider_ids.clone(),
                    workspaces: BTreeMap::from([("default".to_owned(), workspace)]),
                    // The Playground still requires an explicit per-turn toggle
                    // before it asks for a paid or unknown-billing route.
                    allow_paid: true,
                    ..Default::default()
                });
                let token = match broker.authorize_shared(policy, execution) {
                    Ok(token) => token,
                    Err(_) => {
                        let _ = sender.send(Err(
                            "The built-in local runtime could not issue a credential.".to_owned(),
                        ));
                        return;
                    }
                };
                if sender.send(Ok((address.clone(), token))).is_err() {
                    return;
                }
                std::future::pending::<()>().await;
                drop(broker);
            });
        })
        .map_err(|_| "The built-in local runtime could not start a thread.".to_owned())?;
    receiver
        .recv_timeout(START_TIMEOUT)
        .map_err(|_| "The built-in local runtime did not start in time.".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;

    use originlm_contracts::{DataMode, Snapshot};
    use originlm_kit::ScanResult;
    use std::sync::Arc;

    /// A runtime isolated from this machine: fixture discovery that finds
    /// nothing, so neither the startup scan nor the background lanes can see
    /// or probe a real account, plus private state and workspace directories.
    fn isolated(name: &str) -> LocalRuntime {
        let root = std::env::temp_dir().join(format!(
            "originlm-desktop-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let workspace = root.join("playground");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let inventory: InventorySource = Arc::new(|_| {
            Ok(ScanResult {
                snapshot: Snapshot::empty(DataMode::Live),
                locations: BTreeMap::new(),
                metadata_probes: 0,
            })
        });
        LocalRuntime {
            inventory: Some(inventory),
            state_dir: Some(root.join("state")),
            workspace: Some(workspace.canonicalize().expect("canonical workspace")),
        }
    }

    #[test]
    fn local_runtime_issues_a_working_administrative_credential() {
        let (address, token) = start_local_with(isolated("bootstrap")).expect("local runtime");
        let mut client = BrokerClient::connect(&address, &token).expect("connect");
        client.snapshot().expect("snapshot");
        client.apps().expect("administrative apps list");
        let health = client
            .call("runtime.health", serde_json::json!({}))
            .expect("runtime health");
        assert_eq!(health["autonomousRefresh"], true);
        let grants = client
            .call("grants.list", serde_json::json!({}))
            .expect("grants");
        assert_eq!(grants["execution"], true);
        // Fixture discovery is what the runtime sees, not this machine.
        client
            .call("discovery.refresh", serde_json::json!({"level": "passive"}))
            .expect("passive refresh");
        let snapshot = client.snapshot().expect("snapshot after refresh");
        assert!(snapshot.connections.is_empty());
        client.close();
    }

    #[test]
    fn an_explicit_descriptor_is_not_silently_replaced() {
        let path = std::env::temp_dir().join("originlm-desktop-missing-token.json");
        let credential = Credential::acquire(Some(path.clone()));
        assert!(matches!(&credential, Credential::Descriptor(found) if found == &path));
        assert!(credential.connect().is_err());
    }
}
