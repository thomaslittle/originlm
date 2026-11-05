//! Bounded catalog pages over an already permission-filtered snapshot.
use crate::params;
use originlm_contracts::{
    DomainError, ErrorCode, IntelligenceCatalog, IntelligenceScores, Model, ModelPage,
    ModelPageParams, Result, Snapshot, sort_models_by_intelligence, stable_id,
};
use serde_json::Value;
const PAGE_BYTES: usize = 240 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortMode {
    Catalog,
    Intelligence,
}

impl SortMode {
    pub(crate) fn parse(value: Option<&str>, default_for_apps: bool) -> Result<Self> {
        match value {
            Some("intelligence") => Ok(Self::Intelligence),
            Some("catalog") => Ok(Self::Catalog),
            Some("") | None if default_for_apps => Ok(Self::Intelligence),
            Some("") | None => Ok(Self::Catalog),
            Some(_) => Err(params::invalid()),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Intelligence => "intelligence",
        }
    }

    pub(crate) fn apply(self, models: &mut [Model], intelligence: &IntelligenceCatalog) {
        if self == Self::Intelligence {
            let scores = IntelligenceScores::from_catalog(intelligence);
            sort_models_by_intelligence(models, &scores);
        }
    }
}

fn stale() -> DomainError {
    DomainError::new(
        ErrorCode::SelectionStale,
        "Catalog cursor no longer matches this view",
    )
}
fn cursor(
    owner: &str,
    revision: &str,
    route: Option<&str>,
    sort: SortMode,
    index: usize,
) -> String {
    let binding = stable_id(
        "page",
        &[
            owner,
            revision,
            route.unwrap_or(""),
            sort.label(),
            &index.to_string(),
        ],
    );
    format!("{index}.{binding}")
}
pub(crate) fn page(
    snapshot: Snapshot,
    owner: &str,
    intelligence: &IntelligenceCatalog,
    p: &Value,
) -> Result<Value> {
    let request: ModelPageParams = params::decode(p.clone())?;
    let limit = request.limit.unwrap_or(100);
    if !(1..=500).contains(&limit) {
        return Err(params::invalid());
    }
    let route = params::text(p, "routeId")?;
    let sort = SortMode::parse(request.sort.as_deref(), intelligence.default_sort_for_apps)?;
    let begin = if let Some(value) = request.cursor.as_deref() {
        if value.len() > 256 {
            return Err(params::invalid());
        }
        let (number, _) = value.split_once('.').ok_or_else(params::invalid)?;
        let index = number.parse::<usize>().map_err(|_| params::invalid())?;
        if value != cursor(owner, &snapshot.revision, route, sort, index) {
            return Err(stale());
        }
        index
    } else {
        0
    };
    let mut filtered: Vec<_> = snapshot
        .models
        .into_iter()
        .filter(|m| route.is_none_or(|r| r == m.route_id))
        .collect();
    sort.apply(&mut filtered, intelligence);
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
        (next < filtered.len()).then(|| cursor(owner, &snapshot.revision, route, sort, next));
    serde_json::to_value(ModelPage {
        revision: snapshot.revision,
        models,
        next_cursor,
    })
    .map_err(|_| params::invalid())
}
