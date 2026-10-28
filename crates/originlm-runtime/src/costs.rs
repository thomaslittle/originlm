use originlm_contracts::{CostUsage, DecimalString, EventPayload, Session, Timestamp};
use std::collections::VecDeque;

/// Bounded runtime-lifetime usage history. Never retains prompts or credentials.
#[derive(Default)]
pub(crate) struct CostLedger {
    records: VecDeque<(String, CostUsage)>,
}
impl CostLedger {
    pub fn observe(&mut self, owner: &str, session: &Session, turn: &str, payload: &EventPayload) {
        if !matches!(
            payload,
            EventPayload::Usage { .. } | EventPayload::Final { .. }
        ) {
            return;
        }
        let index = self
            .records
            .iter()
            .position(|(_, record)| record.session_id == session.id && record.turn_id == turn);
        let index = index.unwrap_or_else(|| {
            if self.records.len() == 256 {
                self.records.pop_front();
            }
            self.records.push_back((
                owner.into(),
                CostUsage {
                    session_id: session.id.clone(),
                    turn_id: turn.into(),
                    connection_id: session.connection_id.clone(),
                    route_id: session.selection.route_id.clone(),
                    model_id: session.selection.model_id.clone(),
                    observed_at: Timestamp::now(),
                    input_tokens: None,
                    output_tokens: None,
                    reported_usd: None,
                    complete: false,
                },
            ));
            self.records.len() - 1
        });
        let record = &mut self.records[index].1;
        record.observed_at = Timestamp::now();
        match payload {
            EventPayload::Usage { unit, used } => {
                let Ok(value) = DecimalString::try_from(used.clone()) else {
                    return;
                };
                match unit.as_str() {
                    "input_tokens" => record.input_tokens = Some(value),
                    "output_tokens" => record.output_tokens = Some(value),
                    "cost_usd" => record.reported_usd = Some(value),
                    _ => {}
                }
            }
            EventPayload::Final { .. } => record.complete = true,
            _ => {}
        }
    }
    pub fn read(&self, owner: Option<&str>) -> Vec<CostUsage> {
        self.records
            .iter()
            .filter(|(id, _)| owner.is_none_or(|owner| owner == id))
            .map(|(_, record)| record.clone())
            .collect()
    }
}
