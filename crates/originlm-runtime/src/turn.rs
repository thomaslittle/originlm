use crate::{
    engine::{Cell, Pending, id},
    err,
};
use originlm_contracts::*;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::sync::{oneshot, watch};
/// A validated image staged by the host service. Payload bytes are never logged.
#[derive(Clone)]
pub struct ImageInput {
    pub media_type: InputMediaType,
    pub bytes: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
}
impl std::fmt::Debug for ImageInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageInput")
            .field("media_type", &self.media_type)
            .field("byte_length", &self.bytes.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}
impl ImageInput {
    pub fn base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.media_type.as_str(), self.base64())
    }
}
pub struct TurnContext {
    pub(crate) cell: Arc<Cell>,
    pub(crate) turn: String,
    pub(crate) cancel: watch::Receiver<bool>,
    pub(crate) images: Vec<ImageInput>,
}
impl TurnContext {
    /// Images attached to this turn, in request order. Empty for text-only turns.
    pub fn images(&self) -> &[ImageInput] {
        &self.images
    }
    pub fn artifact(&self, name: &str, media: &str, bytes: Vec<u8>) -> Result<String> {
        if self.is_cancelled() {
            return Err(err(ErrorCode::Cancelled));
        }
        self.cell.publish_artifact(&self.turn, name, media, bytes)
    }
    pub fn emit(&self, payload: EventPayload) -> Result<()> {
        (self.cell.guard)()?;
        if *self.cancel.borrow() || !self.cell.active.load(Ordering::Acquire) {
            return Err(err(ErrorCode::Cancelled));
        }
        if matches!(
            payload,
            EventPayload::Final { .. }
                | EventPayload::Artifact { .. }
                | EventPayload::State { .. }
                | EventPayload::ApprovalRequest { .. }
                | EventPayload::ApprovalResolved { .. }
        ) {
            return Err(err(ErrorCode::InvalidRequest));
        }
        if let EventPayload::TextDelta { text } = &payload {
            let mut output = self
                .cell
                .output
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            if output.len() + text.len() > 4 * 1024 * 1024 {
                return Err(err(ErrorCode::FrameTooLarge));
            }
            output.extend_from_slice(text.as_bytes());
        }
        self.cell.emit(Some(self.turn.clone()), payload)
    }
    pub async fn cancelled(&mut self) {
        if !*self.cancel.borrow() {
            let _ = self.cancel.changed().await;
        }
    }
    pub fn is_cancelled(&self) -> bool {
        *self.cancel.borrow() || !self.cell.active.load(Ordering::Acquire)
    }
    pub async fn approve(&self, action: String, detail: String) -> Result<bool> {
        (self.cell.guard)()?;
        if self.is_cancelled() {
            return Err(err(ErrorCode::Cancelled));
        }
        if action.len() > 256 || detail.len() > 8192 {
            return Err(err(ErrorCode::FrameTooLarge));
        }
        let approval = id("approval");
        let (tx, rx) = oneshot::channel();
        {
            let mut state = self
                .cell
                .state
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            state.view.state = SessionState::AwaitingApproval;
            let mut approvals = self
                .cell
                .approvals
                .lock()
                .map_err(|_| err(ErrorCode::ProviderUnavailable))?;
            if approvals.len() >= 16 {
                return Err(err(ErrorCode::RateLimited));
            }
            approvals.insert(
                approval.clone(),
                Pending {
                    session: state.view.id.clone(),
                    sender: tx,
                },
            );
        }
        self.cell.emit(
            Some(self.turn.clone()),
            EventPayload::ApprovalRequest {
                approval_id: approval.clone(),
                action,
                detail,
            },
        )?;
        let mut cancel = self.cancel.clone();
        let allowed = tokio::select! {
         answer=rx=>answer.unwrap_or(false),
         _=async{if !*cancel.borrow(){let _=cancel.changed().await;}}=>false,
         _=tokio::time::sleep(Duration::from_secs(120))=>false
        };
        self.cell
            .approvals
            .lock()
            .map_err(|_| err(ErrorCode::ProviderUnavailable))?
            .remove(&approval);
        if let Ok(mut state) = self.cell.state.lock()
            && state.view.state == SessionState::AwaitingApproval
        {
            state.view.state = SessionState::Running;
        }
        let allowed = allowed && !self.is_cancelled() && (self.cell.guard)().is_ok();
        self.cell.emit(
            Some(self.turn.clone()),
            EventPayload::ApprovalResolved {
                approval_id: approval,
                allowed,
            },
        )?;
        Ok(allowed)
    }
}
