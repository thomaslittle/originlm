//! Bounded catalog pages over an already permission-filtered snapshot.
use crate::params;
use originlm_contracts::{
    DomainError, ErrorCode, ModelPage, ModelPageParams, Result, Snapshot, stable_id,
};
use serde_json::Value;
const PAGE_BYTES: usize = 240 * 1024;
fn stale() -> DomainError {
    DomainError::new(
        ErrorCode::SelectionStale,
        "Catalog cursor no longer matches this view",
    )
}
fn cursor(owner: &str, revision: &str, route: Option<&str>, index: usize) -> String {
    let binding = stable_id(
        "page",
        &[owner, revision, route.unwrap_or(""), &index.to_string()],
    );
    format!("{index}.{binding}")
}
pub(crate) fn page(snapshot: Snapshot, owner: &str, p: &Value) -> Result<Value> {
    let request: ModelPageParams = params::decode(p.clone())?;
    let limit = request.limit.unwrap_or(100);
    if !(1..=500).contains(&limit) {
        return Err(params::invalid());
    }
    let route = params::text(p, "routeId")?;
    let begin = if let Some(value) = request.cursor.as_deref() {
        if value.len() > 256 {
            return Err(params::invalid());
        }
        let (number, _) = value.split_once('.').ok_or_else(params::invalid)?;
        let index = number.parse::<usize>().map_err(|_| params::invalid())?;
        if value != cursor(owner, &snapshot.revision, route, index) {
            return Err(stale());
        }
        index
    } else {
        0
    };
    let filtered: Vec<_> = snapshot
        .models
        .into_iter()
        .filter(|m| route.is_none_or(|r| r == m.route_id))
        .collect();
    if begin > filtered.len() || (begin == filtered.len() && request.cursor.is_some()) {
        return Err(stale());
    }
    let mut models = Vec::new();
    let mut bytes = 0;
    for model in filtered.iter().skip(begin).take(limit as usize) {
        let size = serde_json::to_vec(model)
            .map_err(|_| params::invalid())?
            .len();
        if size > PAGE_BYTES {
            return Err(DomainError::new(
                ErrorCode::FrameTooLarge,
                "A model descriptor exceeds the page limit",
            ));
        }
        if bytes + size > PAGE_BYTES {
            break;
        }
        bytes += size;
        models.push(model.clone());
    }
    let next = begin + models.len();
    let next_cursor =
        (next < filtered.len()).then(|| cursor(owner, &snapshot.revision, route, next));
    serde_json::to_value(ModelPage {
        revision: snapshot.revision,
        models,
        next_cursor,
    })
    .map_err(|_| params::invalid())
}
