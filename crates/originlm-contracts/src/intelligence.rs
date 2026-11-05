//! Cached Artificial Analysis Intelligence Index data.
//!
//! The cache is deliberately a separate read surface: it is not provider
//! evidence and it never changes a model's capability or billing state. Scores
//! stay optional so an unmeasured model is shown as unknown, never as zero.

use crate::Model;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

pub const INTELLIGENCE_SOURCE: &str = "artificial_analysis";
pub const INTELLIGENCE_ATTRIBUTION: &str = "Artificial Analysis (https://artificialanalysis.ai/)";
pub const INTELLIGENCE_SOURCE_URL: &str = "https://artificialanalysis.ai/";

/// One model in the free Artificial Analysis language-model list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntelligenceModel {
    /// Artificial Analysis' stable UUID for the model.
    pub id: String,
    /// Artificial Analysis' stable slug. This is the cache key.
    pub slug: String,
    pub name: String,
    pub creator: String,
    pub release_date: Option<String>,
    /// Null means the model has no published Intelligence Index yet.
    pub intelligence_index: Option<f64>,
    /// Artificial Analysis no longer publishes these two indices; they stay
    /// null unless the free response starts returning them again.
    pub coding_index: Option<f64>,
    pub agentic_index: Option<f64>,
    /// The six capability indices, keyed without the common
    /// `artificial_analysis_` prefix and trailing `_index`.
    pub capability_indexes: BTreeMap<String, f64>,
    pub input_price_usd_per_million: Option<f64>,
    pub output_price_usd_per_million: Option<f64>,
    pub median_output_tokens_per_second: Option<f64>,
    pub median_time_to_first_token_seconds: Option<f64>,
}

/// A snapshot of the durable cache. `models` may be empty when the API key is
/// missing or the first sync has not completed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntelligenceCatalog {
    pub source: String,
    pub attribution: String,
    pub fetched_at: Option<crate::Timestamp>,
    pub index_version: Option<u32>,
    pub tier: Option<String>,
    pub models: Vec<IntelligenceModel>,
    /// False when `ARTIFICIAL_ANALYSIS_API_KEY` was not present in the runtime
    /// environment. Callers must not treat this as an error.
    pub configured: bool,
    /// True when the cached fetch is older than the documented refresher.
    pub stale: bool,
    pub last_error: Option<String>,
    /// User opt-in: when true, `models.list`/`models.page` default to
    /// Intelligence Index order for clients that do not pass `sort`.
    pub default_sort_for_apps: bool,
}

impl Default for IntelligenceCatalog {
    fn default() -> Self {
        Self {
            source: INTELLIGENCE_SOURCE.into(),
            attribution: INTELLIGENCE_ATTRIBUTION.into(),
            fetched_at: None,
            index_version: None,
            tier: None,
            models: Vec::new(),
            configured: false,
            stale: true,
            last_error: None,
            default_sort_for_apps: false,
        }
    }
}

/// Prefix-free, lower-case lookup key for an AA slug, model name or a native
/// provider model id.
pub fn intelligence_key(value: &str) -> String {
    let mut key = String::new();
    let mut separator = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            key.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator {
            key.push('-');
            separator = true;
        }
    }
    while key.ends_with('-') {
        key.pop();
    }
    key
}

/// Provider catalogs often add a release date or a `latest` tag to the same
/// model. Strip those well-known suffixes only, so `claude-sonnet-4` is never
/// mistaken for `claude-sonnet-4-5`.
fn strip_catalog_suffixes(key: &str) -> String {
    let mut parts: Vec<&str> = key.split('-').collect();
    loop {
        let Some(last) = parts.last().copied() else {
            break;
        };
        let dated =
            (last.len() == 8 || last.len() == 4) && last.bytes().all(|byte| byte.is_ascii_digit());
        if dated && parts.len() > 1 {
            parts.pop();
            continue;
        }
        if matches!(last, "latest" | "preview" | "experimental") && parts.len() > 1 {
            parts.pop();
            continue;
        }
        break;
    }
    // ISO dates normalize to year-month-day; remove that triple when it is the
    // trailing component of a provider-native id.
    if parts.len() > 3 {
        let (year, month, day) = (
            parts[parts.len() - 3],
            parts[parts.len() - 2],
            parts[parts.len() - 1],
        );
        if year.len() == 4
            && year.bytes().all(|byte| byte.is_ascii_digit())
            && month.len() <= 2
            && month.bytes().all(|byte| byte.is_ascii_digit())
            && day.len() <= 2
            && day.bytes().all(|byte| byte.is_ascii_digit())
        {
            parts.truncate(parts.len() - 3);
        }
    }
    parts.join("-")
}

fn without_parenthetical(value: &str) -> &str {
    value.split_once('(').map(|(base, _)| base).unwrap_or(value)
}

fn add_keys(keys: &mut BTreeSet<String>, value: &str) {
    let normalized = intelligence_key(value);
    if normalized.is_empty() {
        return;
    }
    keys.insert(normalized.clone());
    keys.insert(strip_catalog_suffixes(&normalized));
    let base = without_parenthetical(value);
    if base != value {
        add_keys(keys, base);
    }
}

fn model_keys(native_id: &str, name: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    add_keys(&mut keys, native_id);
    if let Some(last) = native_id.rsplit(['/', ':']).next()
        && last != native_id
    {
        add_keys(&mut keys, last);
    }
    add_keys(&mut keys, name);
    keys
}

fn catalog_keys(model: &IntelligenceModel) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    add_keys(&mut keys, &model.slug);
    add_keys(&mut keys, &model.name);
    // Some provider-native ids are prefixed with the creator, e.g.
    // `openai/gpt-5`.
    let creator = intelligence_key(&model.creator);
    let name = intelligence_key(&model.name);
    if !creator.is_empty() && !name.is_empty() {
        keys.insert(format!("{creator}-{name}"));
    }
    keys
}

/// Precomputed score lookup. Build it once per catalog read instead of
/// normalizing every AA record for every row.
#[derive(Debug, Clone, Default)]
pub struct IntelligenceScores {
    by_key: BTreeMap<String, Option<f64>>,
}

impl IntelligenceScores {
    pub fn from_catalog(catalog: &IntelligenceCatalog) -> Self {
        let mut by_key: BTreeMap<String, Option<f64>> = BTreeMap::new();
        for model in &catalog.models {
            let Some(score) = model.intelligence_index.filter(|value| value.is_finite()) else {
                // A matched model without a published score is intentionally
                // indistinguishable from an unmatched one: both stay unknown.
                continue;
            };
            for key in catalog_keys(model) {
                by_key
                    .entry(key)
                    .and_modify(|existing| {
                        // More than one AA model sharing a key makes a match
                        // unsafe. Refuse to guess.
                        if existing.is_none_or(|current| (current - score).abs() > f64::EPSILON) {
                            *existing = None;
                        }
                    })
                    .or_insert(Some(score));
            }
        }
        Self { by_key }
    }

    pub fn score(&self, native_id: &str, name: &str) -> Option<f64> {
        model_keys(native_id, name)
            .into_iter()
            .find_map(|key| self.by_key.get(&key).copied().flatten())
    }
}

/// Stable, highest-first order. Models without a published score sort last and
/// keep their existing relative order.
pub fn intelligence_order(scores: &IntelligenceScores, left: &Model, right: &Model) -> Ordering {
    match (
        scores.score(&left.native_id, &left.name),
        scores.score(&right.native_id, &right.name),
    ) {
        (Some(left), Some(right)) => right.partial_cmp(&left).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Stable, highest-first sort. Models without a published score sort last and
/// keep their existing relative order.
pub fn sort_models_by_intelligence(models: &mut [Model], scores: &IntelligenceScores) {
    models.sort_by(|left, right| intelligence_order(scores, left, right));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(slug: &str, name: &str, score: Option<f64>) -> IntelligenceModel {
        IntelligenceModel {
            id: slug.into(),
            slug: slug.into(),
            name: name.into(),
            creator: "Example".into(),
            release_date: None,
            intelligence_index: score,
            coding_index: None,
            agentic_index: None,
            capability_indexes: BTreeMap::new(),
            input_price_usd_per_million: None,
            output_price_usd_per_million: None,
            median_output_tokens_per_second: None,
            median_time_to_first_token_seconds: None,
        }
    }

    #[test]
    fn catalog_keys_match_native_ids_without_guessing_across_versions() {
        let catalog = IntelligenceCatalog {
            models: vec![
                model("claude-sonnet-4-5", "Claude Sonnet 4.5", Some(55.0)),
                model("claude-sonnet-4", "Claude Sonnet 4", Some(40.0)),
                model("gpt-5-1", "GPT-5.1", Some(70.0)),
            ],
            ..IntelligenceCatalog::default()
        };
        let scores = IntelligenceScores::from_catalog(&catalog);
        assert_eq!(
            scores.score("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5"),
            Some(55.0)
        );
        assert_eq!(
            scores.score("claude-sonnet-4", "Claude Sonnet 4"),
            Some(40.0)
        );
        assert_eq!(scores.score("openai/gpt-5.1", "GPT-5.1"), Some(70.0));
        assert_eq!(scores.score("claude-sonnet-4-6", "Claude Sonnet 4.6"), None);
    }

    #[test]
    fn missing_scores_are_unknown_and_sort_last_stably() {
        let catalog = IntelligenceCatalog {
            models: vec![
                model("unknown", "Unknown", None),
                model("low", "Low", Some(10.0)),
                model("high", "High", Some(90.0)),
            ],
            ..IntelligenceCatalog::default()
        };
        let scores = IntelligenceScores::from_catalog(&catalog);
        let mut models = vec![
            crate::Model {
                id: "a".into(),
                route_id: "r".into(),
                native_id: "unknown".into(),
                name: "Unknown".into(),
                revision: "1".into(),
                is_default: false,
                capabilities: vec![],
                options: vec![],
                context_window: crate::Observation::unknown("TEST"),
            },
            crate::Model {
                id: "b".into(),
                route_id: "r".into(),
                native_id: "low".into(),
                name: "Low".into(),
                revision: "1".into(),
                is_default: false,
                capabilities: vec![],
                options: vec![],
                context_window: crate::Observation::unknown("TEST"),
            },
            crate::Model {
                id: "c".into(),
                route_id: "r".into(),
                native_id: "high".into(),
                name: "High".into(),
                revision: "1".into(),
                is_default: false,
                capabilities: vec![],
                options: vec![],
                context_window: crate::Observation::unknown("TEST"),
            },
        ];
        sort_models_by_intelligence(&mut models, &scores);
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec!["c", "b", "a"]
        );
    }

    #[test]
    fn ambiguous_catalog_keys_do_not_invent_a_score() {
        let catalog = IntelligenceCatalog {
            models: vec![
                model("gpt-5", "GPT-5 (high)", Some(80.0)),
                model("gpt-5-1", "GPT-5", Some(70.0)),
            ],
            ..IntelligenceCatalog::default()
        };
        let scores = IntelligenceScores::from_catalog(&catalog);
        assert_eq!(scores.score("gpt-5", "GPT-5"), None);
        assert_eq!(scores.score("gpt-5-high", "GPT-5 (high)"), Some(80.0));
    }
}
