//! Broker I/O. Both workers use the public authenticated JSON-RPC surface and
//! never touch the broker store or provider credentials directly.

use crate::{
    bootstrap::Credential,
    history::{History, Sample},
};
use originlm_contracts::{CostReport, IntelligenceCatalog, ModelPrice, Snapshot, estimate_cost};
use originlm_control::BrokerClient;
use serde_json::{Value, json};
use std::{
    sync::mpsc::{Receiver, Sender, TryRecvError},
    thread,
    time::{Duration, Instant},
};

pub struct Bundle {
    pub snapshot: Snapshot,
    pub runtime: Value,
    pub apps: Vec<Value>,
    pub apps_error: Option<String>,
    pub adapters: Vec<Value>,
    pub local_models: Vec<originlm_contracts::LocalModelInventory>,
    pub detection: Option<originlm_contracts::DetectionReport>,
    pub history: Vec<Sample>,
    pub costs: Option<CostReport>,
    pub cost_error: Option<String>,
    /// Cached RankLLMs Intelligence Index scores. Never fetched from this
    /// worker on a timer; the runtime owns the durable cache and its sync lane.
    pub intelligence: IntelligenceCatalog,
    pub fetched_at: i64,
    pub error: Option<String>,
}

pub enum Command {
    Refresh,
    Probe(String),
    RevokeApp(String),
    SetPrices(Vec<ModelPrice>),
    SetIntelligenceDefault(bool),
    RefreshIntelligence,
}

pub struct BrokerHandle {
    pub commands: Sender<Command>,
    pub updates: Receiver<Bundle>,
}

pub fn spawn(credential: Credential, refresh: Duration, example: bool) -> BrokerHandle {
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let (update_tx, update_rx) = std::sync::mpsc::channel();
    thread::Builder::new()
        .name("originlm-broker".into())
        .spawn(move || {
            if example {
                let mut fixture: Value =
                    serde_json::from_str(include_str!("../assets/example-snapshot.json"))
                        .expect("bundled synthetic snapshot");
                let anchor = chrono::DateTime::parse_from_rfc3339(
                    fixture["observedAt"].as_str().expect("fixture timestamp"),
                )
                .expect("valid fixture timestamp");
                // Shift the labeled example once at launch, preserving all relative
                // ages/reset intervals. Real provider evidence is never rebased.
                rebase_example_times(
                    &mut fixture,
                    crate::clock::utc_now().signed_duration_since(anchor),
                );
                let snapshot: Snapshot =
                    serde_json::from_value(fixture).expect("rebased example snapshot");
                let mut costs: CostReport =
                    serde_json::from_str(include_str!("../assets/example-costs.json"))
                        .expect("bundled example costs");
                // Example mode never connects, probes, loads history or executes.
                // LAI_DESKTOP_EXAMPLE_STATE previews the shell's other states
                // with the same synthetic data, so they can be reviewed and
                // captured without a broker.
                match std::env::var("LAI_DESKTOP_EXAMPLE_STATE").ok().as_deref() {
                    Some("loading") => {
                        // Never answer: the window stays in its loading state.
                        let _ = command_rx.recv();
                        return;
                    }
                    Some("offline") => {
                        let _ = update_tx.send(offline_bundle(
                            "Example: the runtime did not answer on its local address.",
                        ));
                        let _ = command_rx.recv();
                        return;
                    }
                    Some("empty") => {
                        let _ = update_tx.send(Bundle {
                            snapshot: Snapshot::empty(originlm_contracts::DataMode::Synthetic),
                            runtime: json!({"mode":"synthetic"}),
                            apps: Vec::new(),
                            apps_error: None,
                            adapters: Vec::new(),
                            local_models: Vec::new(),
                            detection: None,
                            history: Vec::new(),
                            costs: None,
                            cost_error: None,
                            intelligence: IntelligenceCatalog::default(),
                            fetched_at: crate::clock::timestamp(),
                            error: None,
                        });
                        let _ = command_rx.recv();
                        return;
                    }
                    Some("stale") => {
                        let mut first = true;
                        loop {
                            let bundle = if first {
                                Bundle {
                                    snapshot: snapshot.clone(),
                                    runtime: json!({"mode":"synthetic"}),
                                    apps: Vec::new(),
                                    apps_error: Some(
                                        "Example mode has no connected applications.".into(),
                                    ),
                                    adapters: Vec::new(),
                                    local_models: Vec::new(),
                                    detection: None,
                                    history: Vec::new(),
                                    costs: Some(costs.clone()),
                                    cost_error: None,
                                    intelligence: IntelligenceCatalog::default(),
                                    // Pretend the last good read was 7 minutes ago.
                                    fetched_at: crate::clock::timestamp() - 420,
                                    error: None,
                                }
                            } else {
                                offline_bundle(
                                    "Example: the runtime stopped answering after the last read.",
                                )
                            };
                            first = false;
                            if update_tx.send(bundle).is_err() {
                                return;
                            }
                            if let Err(std::sync::mpsc::RecvTimeoutError::Disconnected) =
                                command_rx.recv_timeout(Duration::from_secs(2))
                            {
                                return;
                            }
                        }
                    }
                    _ => {}
                }
                loop {
                    if update_tx
                        .send(Bundle {
                            snapshot: snapshot.clone(),
                            runtime: json!({"mode":"synthetic"}),
                            apps: Vec::new(),
                            apps_error: Some("Example mode has no connected applications.".into()),
                            adapters: Vec::new(),
                            local_models: Vec::new(),
                            detection: None,
                            history: Vec::new(),
                            costs: Some(costs.clone()),
                            cost_error: None,
                            intelligence: IntelligenceCatalog::default(),
                            fetched_at: crate::clock::timestamp(),
                            error: None,
                        })
                        .is_err()
                    {
                        break;
                    }
                    match command_rx.recv() {
                        Ok(Command::SetPrices(prices)) => {
                            costs.prices = prices;
                            for record in &mut costs.records {
                                record.price = costs
                                    .prices
                                    .iter()
                                    .find(|price| {
                                        price.provider_id == record.provider_id
                                            && price.native_model_id == record.native_model_id
                                    })
                                    .cloned();
                                record.amount_usd = record.price.as_ref().and_then(|price| {
                                    estimate_cost(
                                        record.usage.input_tokens.as_ref()?,
                                        record.usage.output_tokens.as_ref()?,
                                        price,
                                    )
                                });
                                record.basis = if record.amount_usd.is_some() {
                                    "estimated"
                                } else {
                                    "unknown"
                                }
                                .into();
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            } else {
                worker(credential, refresh, command_rx, update_tx);
            }
        })
        .expect("broker worker");
    BrokerHandle {
        commands: command_tx,
        updates: update_rx,
    }
}

/// The bundle the worker sends when the runtime cannot be read.
fn offline_bundle(error: &str) -> Bundle {
    Bundle {
        snapshot: Snapshot::empty(originlm_contracts::DataMode::Live),
        runtime: Value::Null,
        apps: Vec::new(),
        apps_error: Some("Broker became unavailable; reconnecting.".into()),
        adapters: Vec::new(),
        local_models: Vec::new(),
        detection: None,
        costs: None,
        cost_error: Some("Runtime is offline".into()),
        intelligence: IntelligenceCatalog::default(),
        history: Vec::new(),
        fetched_at: crate::clock::timestamp(),
        error: Some(error.to_string()),
    }
}

fn rebase_example_times(value: &mut Value, offset: chrono::Duration) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(key.as_str(), "observedAt" | "resetsAt") {
                    if let Some(timestamp) = value
                        .as_str()
                        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
                    {
                        *value = Value::String((timestamp + offset).to_rfc3339());
                    }
                } else {
                    rebase_example_times(value, offset);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| rebase_example_times(item, offset)),
        _ => {}
    }
}

fn connect(credential: &Credential) -> Result<BrokerClient, String> {
    if std::env::var_os("LAI_DEBUG").is_some() {
        eprintln!("[debug] connecting with {}", credential.label());
    }
    let result = credential.connect();
    if std::env::var_os("LAI_DEBUG").is_some() {
        eprintln!(
            "[debug] connect result: {}",
            if result.is_ok() { "ok" } else { "err" }
        );
    }
    result
}

fn fetch(client: &mut BrokerClient, history: &mut History) -> Result<Bundle, String> {
    let snapshot = crate::data::canonical_snapshot(client.snapshot()?);
    let runtime = client
        .call("runtime.health", json!({}))
        .unwrap_or(Value::Null);
    let (apps, apps_error) = match client.apps() {
        Ok(apps) => (apps, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let adapters = client
        .call("adapters.list", json!({}))
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let local_models = client
        .call("models.local", json!({}))
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let detection = client
        .call("detection.status", json!({}))
        .ok()
        .and_then(|value| serde_json::from_value(value).ok());
    let (costs, cost_error) = match client.call("costs.read", json!({})).and_then(|value| {
        serde_json::from_value(value)
            .map_err(|_| "Cost evidence was not in the supported format".into())
    }) {
        Ok(report) => (Some(report), None),
        Err(_) => (
            None,
            Some("Cost evidence unavailable. Connect to a runtime with costs.read support.".into()),
        ),
    };
    let intelligence = client
        .call("intelligence.catalog", json!({}))
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let now = crate::clock::timestamp();
    history.observe(&snapshot, now);
    Ok(Bundle {
        snapshot,
        runtime,
        apps,
        apps_error,
        adapters,
        local_models,
        detection,
        costs,
        cost_error,
        intelligence,
        history: history.samples.clone(),
        fetched_at: now,
        error: None,
    })
}

fn worker(
    credential: Credential,
    refresh: Duration,
    commands: Receiver<Command>,
    updates: Sender<Bundle>,
) {
    let mut history = History::load();
    let mut client: Option<BrokerClient> = None;
    let mut last_error: Option<String> = None;
    let mut scanned = false;
    let mut metadata_workers: std::collections::BTreeMap<String, thread::JoinHandle<()>> =
        std::collections::BTreeMap::new();

    loop {
        metadata_workers.retain(|_, worker| !worker.is_finished());
        if client.is_none() {
            match connect(&credential) {
                Ok(mut connected) => {
                    // A private runtime starts empty. One passive scan gives the
                    // first window real local inventory instead of an empty page.
                    if !scanned && credential.is_local() {
                        let _ = connected.call("discovery.refresh", json!({"level": "passive"}));
                        scanned = true;
                    }
                    match fetch(&mut connected, &mut history) {
                        Ok(bundle) => {
                            let _ = updates.send(bundle);
                        }
                        Err(error) => {
                            last_error = Some(error);
                        }
                    }
                    client = Some(connected);
                }
                Err(error) => {
                    if std::env::var_os("LAI_DEBUG").is_some() {
                        eprintln!("[debug] connect failed: {error}");
                    }
                    if last_error.as_deref() != Some(error.as_str()) {
                        let _ = updates.send(Bundle {
                            snapshot: Snapshot::empty(originlm_contracts::DataMode::Live),
                            runtime: Value::Null,
                            apps: Vec::new(),
                            apps_error: Some(
                                "No administrative credential; app management is unavailable."
                                    .into(),
                            ),
                            adapters: Vec::new(),
                            local_models: Vec::new(),
                            detection: None,
                            costs: None,
                            cost_error: Some("Runtime is offline".into()),
                            intelligence: IntelligenceCatalog::default(),
                            history: history.samples.clone(),
                            fetched_at: crate::clock::timestamp(),
                            error: Some(error.clone()),
                        });
                        last_error = Some(error);
                    }
                    match commands.recv_timeout(Duration::from_secs(3)) {
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        _ => continue,
                    }
                }
            }
        }

        match commands.recv_timeout(refresh) {
            Ok(Command::Refresh) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(active) = client.as_mut() {
                    match fetch(active, &mut history) {
                        Ok(bundle) => {
                            if std::env::var_os("LAI_DEBUG").is_some() {
                                eprintln!(
                                    "[debug] bundle sent: {} connections",
                                    bundle.snapshot.connections.len()
                                );
                            }
                            let _ = updates.send(bundle);
                        }
                        Err(error) => {
                            let _ = updates.send(Bundle {
                                snapshot: Snapshot::empty(originlm_contracts::DataMode::Live),
                                runtime: Value::Null,
                                apps: Vec::new(),
                                apps_error: Some("Broker became unavailable; reconnecting.".into()),
                                adapters: Vec::new(),
                                local_models: Vec::new(),
                                detection: None,
                                costs: None,
                                cost_error: Some("Runtime is offline".into()),
                                intelligence: IntelligenceCatalog::default(),
                                history: history.samples.clone(),
                                fetched_at: crate::clock::timestamp(),
                                error: Some(error),
                            });
                            client = None;
                        }
                    }
                }
            }
            Ok(Command::Probe(connection_id)) => {
                // A cold native ACP startup can legitimately take longer than
                // a control request. Keep polling snapshots on this worker;
                // run the explicit re-read on its own authenticated connection.
                if !metadata_workers.contains_key(&connection_id) && metadata_workers.len() < 64 {
                    let credential = credential.clone();
                    let id = connection_id.clone();
                    let timeout = originlm_kit::metadata_timeout() * 2 + Duration::from_secs(30);
                    if let Ok(worker) = thread::Builder::new()
                        .name("originlm-metadata-refresh".into())
                        .spawn(move || {
                            if let Ok(mut active) = connect(&credential) {
                                let _ = active.call_typed_with(
                                    "connections.probe",
                                    json!({"connectionId":id,"force":true}),
                                    timeout,
                                    None,
                                );
                                active.close();
                            }
                        })
                    {
                        metadata_workers.insert(connection_id, worker);
                    }
                }
            }
            Ok(Command::SetPrices(prices)) => {
                if let Some(active) = client.as_mut() {
                    let error = active
                        .call("costs.setPrices", json!({"prices":prices}))
                        .err();
                    if let Ok(mut bundle) = fetch(active, &mut history) {
                        if error.is_some() {
                            bundle.cost_error = Some("Prices were not saved. Check administrator access and price values.".into());
                        }
                        let _ = updates.send(bundle);
                    }
                }
            }
            Ok(Command::SetIntelligenceDefault(enabled)) => {
                if let Some(active) = client.as_mut() {
                    let _ = active.call(
                        "intelligence.configure",
                        json!({"defaultSortForApps": enabled}),
                    );
                    if let Ok(bundle) = fetch(active, &mut history) {
                        let _ = updates.send(bundle);
                    }
                }
            }
            Ok(Command::RefreshIntelligence) => {
                if let Some(active) = client.as_mut() {
                    let _ = active.call("intelligence.refresh", json!({}));
                    if let Ok(bundle) = fetch(active, &mut history) {
                        let _ = updates.send(bundle);
                    }
                }
            }
            Ok(Command::RevokeApp(target)) => {
                if let Some(active) = client.as_mut() {
                    let _ = active.call("apps.revoke", json!({"targetClientId": target}));
                    if let Ok(bundle) = fetch(active, &mut history) {
                        let _ = updates.send(bundle);
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Some(client) = client {
        client.close();
    }
}

// ---------------------------------------------------------------------------
// Playground execution worker
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ModelChoice {
    pub route_id: String,
    pub model_id: String,
    pub revision: String,
    pub label: String,
    pub connection: String,
    pub provider: String,
    pub provider_id: String,
    pub billing: String,
}

pub enum ExecCommand {
    Run {
        prompt: String,
        choice: ModelChoice,
        allow_paid: bool,
    },
    Cancel,
    Close(String),
}

#[derive(Clone, Debug)]
pub enum ExecEvent {
    Images {
        images: Vec<originlm_kit::images::GeneratedImage>,
    },
    Started {
        session_id: String,
        turn_id: String,
        label: String,
    },
    Delta {
        session_id: String,
        text: String,
    },
    Usage {
        session_id: String,
        description: String,
    },
    Finished {
        session_id: String,
        status: String,
    },
    Failed {
        error: String,
    },
}

pub struct ExecHandle {
    pub commands: Sender<ExecCommand>,
    pub events: Receiver<ExecEvent>,
}

pub fn spawn_executor(credential: Credential, example: bool) -> ExecHandle {
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    thread::Builder::new()
        .name("originlm-playground".into())
        .spawn(move || {
            if example {
                while command_rx.recv().is_ok() {
                    let _ = event_tx.send(ExecEvent::Failed {
                        error: "Execution is disabled in example mode. Connect a granted runtime to run a turn.".into(),
                    });
                }
            } else { executor(credential, command_rx, event_tx); }
        })
        .expect("execution worker");
    ExecHandle {
        commands: command_tx,
        events: event_rx,
    }
}

fn executor(credential: Credential, commands: Receiver<ExecCommand>, events: Sender<ExecEvent>) {
    let image_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build();
    let mut client: Option<BrokerClient> = None;
    #[allow(unused_assignments)]
    let mut current_session: Option<String> = None;
    #[allow(unused_assignments)]
    let mut current_turn: Option<String> = None;

    while let Ok(command) = commands.recv() {
        match command {
            ExecCommand::Run {
                prompt,
                choice,
                allow_paid,
            } => {
                if current_session.is_some() {
                    let _ = events.send(ExecEvent::Failed {
                        error: "A turn is already running.".into(),
                    });
                    continue;
                }
                if client.is_none() {
                    match connect(&credential) {
                        Ok(connected) => client = Some(connected),
                        Err(error) => {
                            let _ = events.send(ExecEvent::Failed { error });
                            continue;
                        }
                    }
                }
                let Some(active) = client.as_mut() else {
                    continue;
                };
                let selection = json!({
                    "routeId": choice.route_id,
                    "modelId": choice.model_id,
                    "revision": choice.revision,
                    "options": {},
                });
                // Native agents already expose the host MCP transport. Direct
                // text-only HTTP integrations currently refuse host MCP tools.
                let supports_host_tools = !matches!(
                    choice.provider_id.as_str(),
                    "openai_api"
                        | "gemini_api"
                        | "anthropic_api"
                        | "compatible_api"
                        | "xai_api"
                        | "ollama"
                        | "lmstudio"
                );
                let image_server = if allow_paid && supports_host_tools {
                    match image_runtime
                        .as_ref()
                        .map_err(|_| "Image runtime could not start".to_owned())
                        .and_then(|runtime| {
                            start_image_server(runtime).map_err(|error| error.to_string())
                        }) {
                        Ok(server) => Some(server),
                        Err(error) => {
                            let _ = events.send(ExecEvent::Failed { error });
                            continue;
                        }
                    }
                } else {
                    None
                };
                let host_servers: Vec<_> = image_server
                    .as_ref()
                    .map(|s| s.descriptor.clone())
                    .into_iter()
                    .collect();
                let created = active.call(
                    "sessions.create",
                    json!({
                        "selection": selection,
                        "workspaceId": "default",
                        "permissionMode": "read_only",
                        "reviewRequests": false,
                        "allowPaid": allow_paid,
                        "hostMcpServers": host_servers,
                        "instructions": "For requested imagery, call generate_image if available. Include scene purpose and collection art direction. Present the image inline and offer variations. Image generation uses the installed Codex subscription by default, independently of your selected conversation model; an explicitly configured API route overrides that. If unavailable, explain that image generation needs a configured image route, paid-operation approval and a conversation integration supporting host tools; never pretend to have generated an image.",
                    }),
                );
                let session_id = match created {
                    Ok(value) => value
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    Err(error) => {
                        let _ = events.send(ExecEvent::Failed {
                            error: format!("Session could not start: {error}"),
                        });
                        continue;
                    }
                };
                let started = active.call(
                    "turns.start",
                    json!({
                        "sessionId": session_id,
                        "prompt": prompt,
                        "requestId": format!("desktop-{}", crate::clock::timestamp_millis()),
                    }),
                );
                let turn_id = match started {
                    Ok(value) => value
                        .get("turnId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    Err(error) => {
                        let _ = active.call("sessions.close", json!({"sessionId": session_id}));
                        let _ = events.send(ExecEvent::Failed {
                            error: format!("Turn could not start: {error}"),
                        });
                        continue;
                    }
                };
                current_session = Some(session_id.clone());
                current_turn = Some(turn_id.clone());
                let _ = (&current_session, &current_turn);
                let _ = events.send(ExecEvent::Started {
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    label: choice.label.clone(),
                });

                let mut cursor: Option<String> = None;
                let deadline = Instant::now()
                    + Duration::from_secs(if image_server.is_some() { 660 } else { 180 });
                let mut idle = 0u32;
                let mut finished = false;
                while Instant::now() < deadline && !finished {
                    thread::sleep(Duration::from_millis(500));
                    if let Ok(command) = commands.try_recv() {
                        match command {
                            ExecCommand::Cancel => {
                                let _ = active.call(
                                    "turns.cancel",
                                    json!({"sessionId":session_id,"turnId":turn_id}),
                                );
                                let _ = events.send(ExecEvent::Finished {
                                    session_id: session_id.clone(),
                                    status: "cancelled".into(),
                                });
                                finished = true;
                                break;
                            }
                            ExecCommand::Close(id) => {
                                let _ = active.call("sessions.close", json!({"sessionId":id}));
                            }
                            ExecCommand::Run { .. } => { /* Composer disables submission during a turn. */
                            }
                        }
                    }
                    let mut params = json!({"sessionId": session_id});
                    if let Some(after) = &cursor {
                        params["after"] = json!(after);
                    }
                    let page = match active.call("events.read", params) {
                        Ok(page) => page,
                        Err(error) => {
                            let _ = events.send(ExecEvent::Failed {
                                error: format!("Event stream stopped: {error}"),
                            });
                            break;
                        }
                    };
                    drain_images(image_server.as_ref(), &events);
                    let empty = page
                        .get("events")
                        .and_then(Value::as_array)
                        .is_none_or(Vec::is_empty);
                    if empty {
                        idle += 1;
                        if idle >= if image_server.is_some() { 1200 } else { 40 } {
                            break;
                        }
                        continue;
                    }
                    idle = 0;
                    if let Some(next) = page.get("cursor").and_then(Value::as_str) {
                        cursor = Some(next.to_string());
                    }
                    for event in page
                        .get("events")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                    {
                        let payload = event.get("payload").cloned().unwrap_or(Value::Null);
                        match payload.get("type").and_then(Value::as_str) {
                            Some("text_delta") => {
                                if let Some(text) = payload.get("text").and_then(Value::as_str) {
                                    let _ = events.send(ExecEvent::Delta {
                                        session_id: session_id.clone(),
                                        text: text.to_string(),
                                    });
                                }
                            }
                            Some("usage") => {
                                let unit = payload
                                    .get("unit")
                                    .and_then(Value::as_str)
                                    .unwrap_or("tokens");
                                let used =
                                    payload.get("used").and_then(Value::as_str).unwrap_or("?");
                                let _ = events.send(ExecEvent::Usage {
                                    session_id: session_id.clone(),
                                    description: format!("{used} {unit}"),
                                });
                            }
                            Some("final") => {
                                let status = payload
                                    .get("status")
                                    .and_then(Value::as_str)
                                    .unwrap_or("completed");
                                let _ = events.send(ExecEvent::Finished {
                                    session_id: session_id.clone(),
                                    status: status.to_string(),
                                });
                                finished = true;
                            }
                            Some("error") => {
                                let message = payload
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .unwrap_or("The provider reported an error.");
                                let _ = events.send(ExecEvent::Failed {
                                    error: message.to_string(),
                                });
                                finished = true;
                            }
                            _ => {}
                        }
                    }
                }
                drain_images(image_server.as_ref(), &events);
                if !finished {
                    let _ = events.send(ExecEvent::Finished {
                        session_id: session_id.clone(),
                        status: "idle".into(),
                    });
                }
                let _ = active.call("sessions.close", json!({"sessionId": session_id}));
                current_session = None;
                current_turn = None;
            }
            ExecCommand::Cancel => {
                if let (Some(active), Some(session), Some(turn)) = (
                    client.as_mut(),
                    current_session.clone(),
                    current_turn.clone(),
                ) {
                    let _ = active.call(
                        "turns.cancel",
                        json!({"sessionId": session, "turnId": turn}),
                    );
                }
            }
            ExecCommand::Close(session_id) => {
                if let Some(active) = client.as_mut() {
                    let _ = active.call("sessions.close", json!({"sessionId": session_id}));
                }
            }
        }
    }
    if let Some(client) = client {
        client.close();
    }
    let _ = TryRecvError::Empty;
}

fn drain_images(
    server: Option<&originlm_kit::images::mcp::ImageServer>,
    events: &Sender<ExecEvent>,
) {
    if let Some(server) = server {
        while let Ok(images) = server.results.try_recv() {
            let _ = events.send(ExecEvent::Images { images });
        }
    }
}
fn start_image_server(
    runtime: &tokio::runtime::Runtime,
) -> originlm_kit::images::Result<originlm_kit::images::mcp::ImageServer> {
    use originlm_kit::images::{Canvas, ErrorKind, ImageError, ImageTool};
    let provider = originlm_kit::default_image_provider()?;
    let assets = originlm_platform::default_state_dir(None)
        .ok_or_else(|| ImageError::new(ErrorKind::AssetFailure))?
        .join("playground-images");
    let tool = ImageTool::new(provider, assets, Canvas::default())?;
    runtime.block_on(originlm_kit::images::mcp::ImageServer::start(tool))
}
