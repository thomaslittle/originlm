use crate::err;
use originlm_contracts::*;
use serde_json::{Value, json};
use std::collections::VecDeque;
#[derive(Default)]
pub(crate) struct Journal {
    sequence: u64,
    bytes: usize,
    records: VecDeque<(Event, usize)>,
}
impl Journal {
    pub fn append(&mut self, mut event: Event) -> Result<()> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| err(ErrorCode::EventGap))?;
        event.sequence = self.sequence.to_string();
        let size = serde_json::to_vec(&event)
            .map_err(|_| err(ErrorCode::InvalidRequest))?
            .len();
        if size > 65_536 {
            return Err(err(ErrorCode::FrameTooLarge));
        }
        while self.records.len() >= 512 || self.bytes + size > 1_048_576 {
            if let Some((_, size)) = self.records.pop_front() {
                self.bytes -= size
            } else {
                break;
            }
        }
        self.bytes += size;
        self.records.push_back((event, size));
        Ok(())
    }
    pub fn read(&self, session: Option<&str>, after: Option<u64>) -> Result<Value> {
        if let Some(after) = after
            && (after > self.sequence
                || self.records.front().is_some_and(|(e, _)| {
                    after.saturating_add(1) < e.sequence.parse::<u64>().unwrap_or(0)
                }))
        {
            return Err(err(ErrorCode::EventGap));
        }
        let mut events = Vec::new();
        let mut bytes = 0;
        let mut cursor = after.unwrap_or_else(|| {
            self.records
                .front()
                .and_then(|(e, _)| e.sequence.parse::<u64>().ok())
                .unwrap_or(1)
                .saturating_sub(1)
        });
        for (event, size) in &self.records {
            let sequence = event
                .sequence
                .parse::<u64>()
                .map_err(|_| err(ErrorCode::EventGap))?;
            if sequence <= cursor {
                continue;
            }
            if session.is_none_or(|s| s == event.session_id) {
                if !events.is_empty() && (events.len() >= 128 || bytes + size > 262_144) {
                    break;
                }
                bytes += size;
                events.push(event);
            }
            cursor = sequence;
        }
        if self.records.is_empty() {
            cursor = self.sequence;
        }
        Ok(
            json!({"events":events,"cursor":cursor.to_string(),"tailCursor":self.sequence.to_string(),"hasMore":cursor<self.sequence}),
        )
    }
}
