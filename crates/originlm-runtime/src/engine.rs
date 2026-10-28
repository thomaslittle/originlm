use crate::{Agent, Guard, ImageInput, TurnContext, err, journal::Journal};
use originlm_contracts::*;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, oneshot, watch};
static NEXT: AtomicU64 = AtomicU64::new(1);
pub(crate) fn id(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}
pub(crate) struct Pending {
    pub session: String,
    pub sender: oneshot::Sender<bool>,
}
pub(crate) struct State {
    pub view: Session,
    pub requests: BTreeSet<String>,
    pub turn: Option<String>,
    pub cancel: Option<watch::Sender<bool>>,
    pub usable: bool,
}
pub(crate) struct Cell {
    pub owner: String,
    pub state: Mutex<State>,
    pub agent: AsyncMutex<Option<Box<dyn Agent>>>,
    pub guard: Guard,
    pub active: AtomicBool,
    /// Closed/unusable does not imply that remote cleanup completed.
    cleanup_pending: AtomicBool,
    pub approvals: Mutex<BTreeMap<String, Pending>>,
    pub journal: Arc<Mutex<Journal>>,
    pub artifacts: Arc<Mutex<crate::artifact::ArtifactStore>>,
    pub output: Mutex<Vec<u8>>,
    pub image_input: bool,
    costs: Arc<Mutex<crate::costs::CostLedger>>,
}
impl Cell {
    pub fn publish_artifact(
        &self,
        turn: &str,
        name: &str,
        media: &str,
        bytes: Vec<u8>,
    ) -> Result<String> {
        (self.guard)()?;
        let state = self
            .state
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
        if !self.active.load(Ordering::Acquire) || state.view.state == SessionState::Closed {
            return Err(err(ErrorCode::LocalAccessDenied));
        }
        let session = state.view.id.clone();
        let size = bytes.len().to_string();
        drop(state);
        let id = self
            .artifacts
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .insert(&self.owner, &session, name, media, bytes)?;
        if let Err(error) = self.emit(
            Some(turn.into()),
            EventPayload::Artifact {
                artifact_id: id.clone(),
                media_type: media.into(),
                size,
            },
        ) {
            if let Ok(mut store) = self.artifacts.lock() {
                store.remove(&id);
            }
            return Err(error);
        }
        // A concurrent revoke/close must not leave an artifact reachable or retained.
        let state = self
            .state
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
        if !self.active.load(Ordering::Acquire) || state.view.state == SessionState::Closed {
            if let Ok(mut store) = self.artifacts.lock() {
                store.remove(&id);
            }
            return Err(err(ErrorCode::LocalAccessDenied));
        }
        Ok(id)
    }
    pub fn emit(&self, turn: Option<String>, payload: EventPayload) -> Result<()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(err(ErrorCode::LocalAccessDenied));
        }
        let state = self
            .state
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
        self.emit_locked(&state, turn, payload)
    }
    fn emit_locked(
        &self,
        state: &State,
        turn: Option<String>,
        payload: EventPayload,
    ) -> Result<()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(err(ErrorCode::LocalAccessDenied));
        }
        self.journal
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .append(Event {
                session_id: state.view.id.clone(),
                turn_id: turn.clone(),
                sequence: String::new(),
                timestamp: Timestamp::now(),
                adapter_id: "native".into(),
                schema_version: "1.0".into(),
                payload: payload.clone(),
            })?;
        if let Some(turn) = &turn {
            self.costs
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?
                .observe(&self.owner, &state.view, turn, &payload);
        }
        Ok(())
    }
}
#[derive(Default)]
pub struct Runtime {
    costs: Arc<Mutex<crate::costs::CostLedger>>,
    sessions: Mutex<BTreeMap<String, Arc<Cell>>>,
    journals: Mutex<BTreeMap<String, Arc<Mutex<Journal>>>>,
    artifacts: Arc<Mutex<crate::artifact::ArtifactStore>>,
}
impl Runtime {
    pub fn cost_usage(&self, owner: Option<&str>) -> Result<Vec<CostUsage>> {
        Ok(self
            .costs
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .read(owner))
    }
    pub fn artifact(
        &self,
        owner: &str,
        session: &str,
        artifact: &str,
        offset: u64,
        max: usize,
    ) -> Result<ArtifactChunk> {
        let cell = self.cell(owner, session)?;
        (cell.guard)()?;
        self.artifacts
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .read(owner, session, artifact, offset, max)
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(
        &self,
        owner: &str,
        connection: String,
        selection: Selection,
        agent: Box<dyn Agent>,
        guard: Guard,
    ) -> Result<Session> {
        if owner.is_empty() || owner.len() > 128 {
            return Err(err(ErrorCode::InvalidRequest));
        }
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
        sessions.retain(|_, cell| {
            cell.state
                .lock()
                .map(|s| {
                    s.view.state != SessionState::Closed
                        || cell.cleanup_pending.load(Ordering::Acquire)
                })
                .unwrap_or(true)
        });
        if sessions.len() >= 64 || sessions.values().filter(|s| s.owner == owner).count() >= 16 {
            return Err(err(ErrorCode::RateLimited));
        }
        let native_resumption = agent.native_resumption();
        let image_input = agent.image_input();
        let view = Session {
            id: id("session"),
            connection_id: connection,
            selection,
            state: SessionState::Ready,
            created_at: Timestamp::now(),
            native_resumption,
        };
        let journal = self
            .journals
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .entry(owner.into())
            .or_default()
            .clone();
        sessions.insert(
            view.id.clone(),
            Arc::new(Cell {
                owner: owner.into(),
                state: Mutex::new(State {
                    view: view.clone(),
                    requests: BTreeSet::new(),
                    turn: None,
                    cancel: None,
                    usable: true,
                }),
                agent: AsyncMutex::new(Some(agent)),
                guard,
                active: AtomicBool::new(true),
                cleanup_pending: AtomicBool::new(false),
                approvals: Mutex::new(BTreeMap::new()),
                journal,
                artifacts: self.artifacts.clone(),
                output: Mutex::new(Vec::new()),
                image_input,
                costs: self.costs.clone(),
            }),
        );
        Ok(view)
    }
    fn cell(&self, owner: &str, session: &str) -> Result<Arc<Cell>> {
        self.sessions
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .get(session)
            .filter(|s| s.owner == owner && s.active.load(Ordering::Acquire))
            .cloned()
            .ok_or_else(|| err(ErrorCode::NotFound))
    }
    pub fn get(&self, owner: &str, session: &str) -> Result<Session> {
        Ok(self
            .cell(owner, session)?
            .state
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .view
            .clone())
    }
    pub fn update_options(
        &self,
        owner: &str,
        session: &str,
        options: BTreeMap<String, Value>,
    ) -> Result<Session> {
        let cell = self.cell(owner, session)?;
        (cell.guard)()?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
        if !state.usable
            || !matches!(
                state.view.state,
                SessionState::Ready | SessionState::Completed
            )
        {
            return Err(err(ErrorCode::Conflict));
        }
        state.view.selection.options = options;
        Ok(state.view.clone())
    }
    pub async fn start(
        &self,
        owner: &str,
        session: &str,
        prompt: String,
        request: String,
    ) -> Result<Value> {
        self.start_with_inputs(owner, session, prompt, request, Vec::new())
            .await
    }
    /// Start a turn carrying validated images. Drivers that cannot forward images are
    /// rejected before any provider call instead of silently dropping the attachments.
    pub async fn start_with_inputs(
        &self,
        owner: &str,
        session: &str,
        prompt: String,
        request: String,
        images: Vec<ImageInput>,
    ) -> Result<Value> {
        if images.len() > MAX_INPUTS_PER_TURN {
            return Err(err(ErrorCode::InvalidRequest));
        }
        if prompt.is_empty()
            || prompt.len() > MAX_TURN_PROMPT_BYTES
            || prompt.contains('\0')
            || request.is_empty()
            || request.len() > 128
        {
            return Err(err(ErrorCode::InvalidRequest));
        }
        let cell = self.cell(owner, session)?;
        (cell.guard)()?;
        if !images.is_empty() && !cell.image_input {
            return Err(DomainError::new(
                ErrorCode::CapabilityUnsupported,
                "This session's adapter cannot carry image input",
            ));
        }
        let (turn, options, cancel) = {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            if !state.usable
                || !matches!(
                    state.view.state,
                    SessionState::Ready | SessionState::Completed
                )
                || state.requests.len() >= 256
                || !state.requests.insert(request)
            {
                return Err(err(ErrorCode::Conflict));
            }
            let turn = id("turn");
            let (tx, rx) = watch::channel(false);
            state.turn = Some(turn.clone());
            state.cancel = Some(tx);
            state.view.state = SessionState::Running;
            (turn, state.view.selection.options.clone(), rx)
        };
        cell.emit(
            Some(turn.clone()),
            EventPayload::State {
                state: SessionState::Running,
            },
        )?;
        cell.output
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .clear();
        let run_id = turn.clone();
        tokio::spawn(async move {
            run(cell, run_id, prompt, options, cancel, images).await;
        });
        Ok(json!({"turnId":turn}))
    }
    pub async fn cancel(&self, owner: &str, turn: &str) -> Result<Value> {
        let cells: Vec<_> = self
            .sessions
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .values()
            .filter(|s| s.owner == owner)
            .cloned()
            .collect();
        for cell in cells {
            let state = cell
                .state
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            if state.turn.as_deref() == Some(turn) {
                if let Some(cancel) = &state.cancel {
                    cancel.send_replace(true);
                }
                return Ok(json!({"cancelled":true}));
            }
        }
        Err(err(ErrorCode::NotFound))
    }
    pub async fn suspend(&self, owner: &str, session: &str) -> Result<String> {
        let cell = self.cell(owner, session)?;
        (cell.guard)()?;
        {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            if !state.usable
                || !state.view.native_resumption
                || !matches!(
                    state.view.state,
                    SessionState::Ready | SessionState::Completed
                )
                || state.turn.is_some()
            {
                return Err(err(ErrorCode::Conflict));
            }
            state.usable = false;
        }
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let mut agent = cell.agent.lock().await;
            let result = match agent.as_mut() {
                Some(driver) => driver.suspend().await,
                None => Err(err(ErrorCode::ProviderUnavailable)),
            };
            if result.is_ok() {
                agent.take();
            }
            result
        })
        .await
        .unwrap_or_else(|_| Err(err(ErrorCode::Timeout)));
        match result {
            Ok(native_session_id) => {
                {
                    let mut state = cell
                        .state
                        .lock()
                        .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
                    if !cell.active.load(Ordering::Acquire)
                        || state.view.state == SessionState::Closed
                    {
                        return Err(err(ErrorCode::LocalAccessDenied));
                    }
                    state.view.state = SessionState::Closed;
                }
                if let Ok(mut sessions) = self.sessions.lock() {
                    sessions.remove(session);
                }
                if let Ok(mut artifacts) = self.artifacts.lock() {
                    artifacts.clear_session(session);
                }
                if let Ok(mut approvals) = cell.approvals.lock() {
                    approvals.clear();
                }
                Ok(native_session_id)
            }
            Err(error) => {
                // The provider may already have suspended before failing. Keep this
                // session unusable; never continue an uncertain native conversation.
                if let Ok(mut state) = cell.state.lock()
                    && state.view.state != SessionState::Closed
                {
                    state.view.state = SessionState::Error;
                }
                Err(error)
            }
        }
    }
    pub async fn close(&self, owner: &str, session: &str) -> Result<Value> {
        let cell = self.cell(owner, session)?;
        cell.cleanup_pending.store(true, Ordering::Release);
        {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            state.view.state = SessionState::Closed;
            state.usable = false;
            if let Some(cancel) = &state.cancel {
                cancel.send_replace(true);
            }
        }
        self.artifacts
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .clear_session(session);
        cell.approvals
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .clear();
        let mut agent = tokio::time::timeout(Duration::from_secs(5), cell.agent.lock())
            .await
            .map_err(|_| err(ErrorCode::Timeout))?;
        close_driver(&mut agent).await?;
        cell.cleanup_pending.store(false, Ordering::Release);
        Ok(json!({"closed":true}))
    }
    pub fn approve(&self, owner: &str, approval: &str, allow: bool) -> Result<Value> {
        let cells: Vec<_> = self
            .sessions
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .values()
            .filter(|s| s.owner == owner && s.active.load(Ordering::Acquire))
            .cloned()
            .collect();
        for cell in cells {
            let pending = cell
                .approvals
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?
                .remove(approval);
            if let Some(p) = pending {
                (cell.guard)()?;
                if p.session
                    != cell
                        .state
                        .lock()
                        .map_err(|_| err(ErrorCode::ProviderUnavailable))?
                        .view
                        .id
                {
                    return Err(err(ErrorCode::NotFound));
                }
                p.sender.send(allow).map_err(|_| err(ErrorCode::NotFound))?;
                return Ok(json!({"allowed":allow}));
            }
        }
        Err(err(ErrorCode::NotFound))
    }
    pub fn events(&self, owner: &str, session: Option<&str>, after: Option<u64>) -> Result<Value> {
        if let Some(session) = session {
            self.cell(owner, session)?;
        }
        let journals = self
            .journals
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
        match journals.get(owner) {
            Some(j) => j
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?
                .read(session, after),
            None => Journal::default().read(session, after),
        }
    }
    pub fn revoke(&self, owner: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.retain(|_, cell| {
                if cell.owner != owner {
                    return true;
                }
                cell.active.store(false, Ordering::Release);
                if let Ok(state) = cell.state.lock()
                    && let Some(cancel) = &state.cancel
                {
                    cancel.send_replace(true);
                }
                let cell = cell.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let mut agent = cell.agent.lock().await;
                        if let Some(mut driver) = agent.take() {
                            let _ =
                                tokio::time::timeout(Duration::from_secs(2), driver.close()).await;
                        }
                    });
                }
                false
            });
        }
        if let Ok(mut journals) = self.journals.lock() {
            journals.remove(owner);
        }
        if let Ok(mut artifacts) = self.artifacts.lock() {
            artifacts.clear_owner(owner);
        }
    }
}
/// Retain the adapter on failed or dropped cleanup, so a later close can retry.
async fn close_driver(agent: &mut Option<Box<dyn Agent>>) -> Result<()> {
    if let Some(driver) = agent.as_mut() {
        tokio::time::timeout(Duration::from_secs(2), driver.close())
            .await
            .map_err(|_| err(ErrorCode::Timeout))??;
        *agent = None;
    }
    Ok(())
}

async fn run(
    cell: Arc<Cell>,
    turn: String,
    prompt: String,
    options: BTreeMap<String, Value>,
    mut cancel: watch::Receiver<bool>,
    images: Vec<ImageInput>,
) {
    let mut agent = cell.agent.lock().await;
    let result = if let Some(driver) = agent.as_mut() {
        let cx = TurnContext {
            cell: cell.clone(),
            turn: turn.clone(),
            cancel: cancel.clone(),
            images,
        };
        let operation = driver.turn(prompt, options, cx);
        tokio::pin!(operation);
        tokio::select! {
         result=&mut operation=>result,
         _=async{if !*cancel.borrow(){let _=cancel.changed().await;}}=>{
          let _=tokio::time::timeout(Duration::from_secs(2),&mut operation).await;Err(err(ErrorCode::Cancelled))
         },
         _=tokio::time::sleep(Duration::from_secs(600))=>Err(err(ErrorCode::Timeout))
        }
    } else {
        Err(err(ErrorCode::ProviderUnavailable))
    };
    let result = if *cancel.borrow() {
        Err(err(ErrorCode::Cancelled))
    } else {
        result
    };
    let result = result.and_then(|()| {
        let bytes = std::mem::take(
            &mut *cell
                .output
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?,
        );
        if !bytes.is_empty() {
            cell.publish_artifact(&turn, "response.txt", "text/plain; charset=utf-8", bytes)?;
        }
        Ok(())
    });
    if result.is_err() {
        cell.cleanup_pending
            .store(agent.is_some(), Ordering::Release);
        if close_driver(&mut agent).await.is_ok() {
            cell.cleanup_pending.store(false, Ordering::Release);
        }
    }
    if let Ok(mut approvals) = cell.approvals.lock() {
        approvals.clear();
    }
    let state = match &result {
        Ok(()) => SessionState::Completed,
        Err(e) if e.code == ErrorCode::Cancelled => SessionState::Cancelled,
        _ => SessionState::Error,
    };
    // Publish terminal events and state under the same lock. A subsequent turn or
    // suspend must never race the previous turn's final event publication.
    let Ok(mut record) = cell.state.lock() else {
        return;
    };
    if let Err(error) = &result {
        let _ = cell.emit_locked(
            &record,
            Some(turn.clone()),
            EventPayload::Error {
                code: serde_json::to_value(&error.code)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                message: {
                    let detail = originlm_contracts::redact_diagnostic_detail(&error.message);
                    if detail.is_empty() {
                        "Native turn failed without diagnostic text".into()
                    } else {
                        detail
                    }
                },
            },
        );
    }
    let _ = cell.emit_locked(
        &record,
        Some(turn.clone()),
        EventPayload::State {
            state: state.clone(),
        },
    );
    let _ = cell.emit_locked(
        &record,
        Some(turn),
        EventPayload::Final {
            status: if result.is_ok() {
                "completed"
            } else if *cancel.borrow() {
                "cancelled"
            } else {
                "error"
            }
            .into(),
        },
    );
    record.usable = result.is_ok() && record.view.state != SessionState::Closed;
    record.cancel = None;
    record.turn = None;
    if record.view.state != SessionState::Closed {
        record.view.state = state;
    }
}
