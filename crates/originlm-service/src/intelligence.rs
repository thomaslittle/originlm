//! Durable Artificial Analysis Intelligence Index cache.
//!
//! The free API allows only ~100 requests per 24 hours. One complete refresh is
//! two pages, so this lane runs at most four times a day. UI reads never call
//! Artificial Analysis; they read the stored catalog.
use originlm_contracts::{
    INTELLIGENCE_ATTRIBUTION, INTELLIGENCE_SOURCE, IntelligenceCatalog, IntelligenceModel,
    Timestamp,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;

const API_BASE: &str = "https://artificialanalysis.ai/api/v2";
const FREE_MODELS_PATH: &str = "/language/models/free";
const API_KEY_ENV: &str = "ARTIFICIAL_ANALYSIS_API_KEY";
const CACHE_FILE: &str = "intelligence-cache.json";
const CACHE_VERSION: u32 = 1;
const MAX_CACHE_BYTES: usize = 1024 * 1024;
const MAX_MODELS: usize = 1_200;
const MAX_PAGES: u32 = 10;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Two pages per sync, four syncs a day: ~8 requests of a 100-request budget.
pub(crate) const SYNC_PERIOD: Duration = Duration::from_secs(6 * 60 * 60);
const FAILURE_RETRY: Duration = Duration::from_secs(15 * 60);
const STALE_AFTER: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCache {
    version: u32,
    #[serde(default)]
    source: String,
    #[serde(default)]
    attribution: String,
    #[serde(default)]
    fetched_at: Option<String>,
    #[serde(default)]
    index_version: Option<u32>,
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    models: Vec<IntelligenceModel>,
    #[serde(default)]
    default_sort_for_apps: bool,
}

impl Default for StoredCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            source: INTELLIGENCE_SOURCE.into(),
            attribution: INTELLIGENCE_ATTRIBUTION.into(),
            fetched_at: None,
            index_version: None,
            tier: None,
            models: Vec::new(),
            default_sort_for_apps: false,
        }
    }
}

struct State {
    stored: StoredCache,
    next_attempt: Option<Instant>,
    last_error: Option<String>,
}

pub(crate) struct IntelligenceCache {
    state: Mutex<State>,
    sync: AsyncMutex<()>,
    api_key: Option<String>,
    path: Option<PathBuf>,
}

impl Default for IntelligenceCache {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                stored: StoredCache::default(),
                next_attempt: None,
                last_error: None,
            }),
            sync: AsyncMutex::new(()),
            api_key: None,
            path: None,
        }
    }
}

impl IntelligenceCache {
    /// Load the last cache and read the key from the runtime environment. A
    /// missing key is not an error; it only disables network refreshes.
    pub(crate) fn load(directory: PathBuf) -> Self {
        let path = directory.join(CACHE_FILE);
        let mut state = State {
            stored: StoredCache::default(),
            next_attempt: None,
            last_error: None,
        };
        match read_cache(&path) {
            Ok(Some(stored)) => {
                state.next_attempt = fresh_until(&stored.fetched_at);
                state.stored = stored;
            }
            Ok(None) => {}
            Err(error) => {
                state.last_error = Some(error);
            }
        }
        Self {
            state: Mutex::new(state),
            sync: AsyncMutex::new(()),
            api_key: std::env::var(API_KEY_ENV)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty() && value.len() <= 512),
            path: Some(path),
        }
    }

    pub(crate) fn default_sort_for_apps(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.stored.default_sort_for_apps)
            .unwrap_or(false)
    }

    pub(crate) fn catalog(&self) -> IntelligenceCatalog {
        let configured = self.api_key.is_some();
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                return IntelligenceCatalog {
                    configured,
                    ..IntelligenceCatalog::default()
                };
            }
        };
        let fetched_at = state
            .stored
            .fetched_at
            .clone()
            .filter(|_| state.stored.version == CACHE_VERSION);
        let stale = fetched_at
            .as_deref()
            .and_then(age)
            .is_none_or(|age| age > STALE_AFTER);
        IntelligenceCatalog {
            source: INTELLIGENCE_SOURCE.into(),
            attribution: INTELLIGENCE_ATTRIBUTION.into(),
            fetched_at: fetched_at.and_then(|value| Timestamp::try_from(value).ok()),
            index_version: state.stored.index_version,
            tier: state.stored.tier.clone(),
            models: state.stored.models.clone(),
            configured,
            stale,
            last_error: state.last_error.clone(),
            default_sort_for_apps: state.stored.default_sort_for_apps,
        }
    }

    pub(crate) fn set_default_sort_for_apps(&self, enabled: bool) -> IntelligenceCatalog {
        let payload = {
            let mut state = match self.state.lock() {
                Ok(state) => state,
                Err(_) => return self.catalog(),
            };
            state.stored.default_sort_for_apps = enabled;
            serde_json::to_vec_pretty(&state.stored).ok()
        };
        if let Some(bytes) = payload
            && self.persist(&bytes).is_err()
            && let Ok(mut state) = self.state.lock()
        {
            state.last_error =
                Some("The Intelligence Index preference could not be saved locally.".into());
        }
        self.catalog()
    }

    pub(crate) fn sync_due(&self) -> bool {
        if self.api_key.is_none() {
            return false;
        }
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state
            .next_attempt
            .is_none_or(|next_attempt| next_attempt <= Instant::now())
    }

    /// Refresh the whole free list. Never called from a read path; the
    /// background lane and an explicit administrator refresh are the only
    /// callers. Failures leave the previous cache untouched.
    pub(crate) async fn refresh(&self, force: bool) -> IntelligenceCatalog {
        let Some(api_key) = self.api_key.clone() else {
            return self.catalog();
        };
        if !force && !self.sync_due() {
            return self.catalog();
        }
        let _guard = self.sync.lock().await;
        if !force && !self.sync_due() {
            return self.catalog();
        }
        if let Ok(mut state) = self.state.lock() {
            // A conservative deadline until this attempt finishes, so a second
            // tick cannot start another network request.
            state.next_attempt = Some(Instant::now() + FAILURE_RETRY);
        }
        match fetch_all(&api_key).await {
            Ok(fetched) => {
                let stored = StoredCache {
                    version: CACHE_VERSION,
                    source: INTELLIGENCE_SOURCE.into(),
                    attribution: INTELLIGENCE_ATTRIBUTION.into(),
                    fetched_at: Some(
                        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    ),
                    index_version: fetched.index_version,
                    tier: fetched.tier,
                    models: fetched.models,
                    default_sort_for_apps: self.default_sort_for_apps(),
                };
                let serialized = serde_json::to_vec_pretty(&stored).ok();
                let persist_error = serialized
                    .as_deref()
                    .map(|bytes| self.persist(bytes))
                    .transpose()
                    .err();
                if let Ok(mut state) = self.state.lock() {
                    state.next_attempt = Some(Instant::now() + SYNC_PERIOD);
                    state.last_error = persist_error.map(|_| {
                        "Scores were refreshed but could not be saved; they remain in memory."
                            .to_owned()
                    });
                    state.stored = stored;
                }
            }
            Err(error) => {
                if let Ok(mut state) = self.state.lock() {
                    state.next_attempt =
                        Some(Instant::now() + error.retry_after.unwrap_or(FAILURE_RETRY));
                    state.last_error = Some(error.message);
                }
            }
        }
        self.catalog()
    }

    fn persist(&self, bytes: &[u8]) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(parent)?;
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce)
            .map_err(|_| std::io::Error::other("OS randomness unavailable"))?;
        let temporary = parent.join(format!(".intelligence-{:x}.tmp", u64::from_le_bytes(nonce)));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }
}

fn read_cache(path: &std::path::Path) -> Result<Option<StoredCache>, String> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("The Intelligence Index cache could not be read.".into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_CACHE_BYTES as u64 {
        return Err("The Intelligence Index cache is not a bounded regular file.".into());
    }
    let bytes = originlm_platform::read_private_file(path, MAX_CACHE_BYTES)
        .map_err(|_| "The Intelligence Index cache could not be read.".to_owned())?;
    let stored: StoredCache = serde_json::from_slice(&bytes)
        .map_err(|_| "The Intelligence Index cache is malformed and was ignored.".to_owned())?;
    if stored.version != CACHE_VERSION || stored.models.len() > MAX_MODELS {
        return Err("The Intelligence Index cache has an unsupported format.".into());
    }
    if !stored.models.iter().all(valid_model) {
        return Err("The Intelligence Index cache contains an invalid model record.".into());
    }
    Ok(Some(stored))
}

fn valid_model(model: &IntelligenceModel) -> bool {
    !model.id.is_empty()
        && model.id.len() <= 128
        && !model.slug.is_empty()
        && model.slug.len() <= 256
        && !model.name.is_empty()
        && model.name.len() <= 512
        && model.creator.len() <= 256
        && model
            .release_date
            .as_ref()
            .is_none_or(|date| date.len() <= 32)
        && [
            model.intelligence_index,
            model.coding_index,
            model.agentic_index,
            model.input_price_usd_per_million,
            model.output_price_usd_per_million,
            model.median_output_tokens_per_second,
            model.median_time_to_first_token_seconds,
        ]
        .into_iter()
        .all(|value| value.is_none_or(f64::is_finite))
        && model
            .capability_indexes
            .iter()
            .all(|(key, value)| key.len() <= 96 && value.is_finite())
}

fn age(value: &str) -> Option<Duration> {
    let then = chrono::DateTime::parse_from_rfc3339(value).ok()?;
    (chrono::Utc::now() - then.with_timezone(&chrono::Utc))
        .to_std()
        .ok()
}

fn fresh_until(fetched_at: &Option<String>) -> Option<Instant> {
    let age = age(fetched_at.as_deref()?)?;
    (age < SYNC_PERIOD).then(|| Instant::now() + (SYNC_PERIOD - age))
}

struct Fetched {
    index_version: Option<u32>,
    tier: Option<String>,
    models: Vec<IntelligenceModel>,
}

#[derive(Debug)]
struct FetchError {
    message: String,
    retry_after: Option<Duration>,
}

impl FetchError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after: None,
        }
    }
}

#[derive(Deserialize, Default)]
struct AaPage {
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    intelligence_index_version: Option<f64>,
    #[serde(default)]
    pagination: AaPagination,
    #[serde(default)]
    data: Vec<AaModel>,
}

#[derive(Deserialize, Default)]
struct AaPagination {
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    total_pages: Option<u32>,
}

#[derive(Deserialize, Default)]
struct AaModel {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    model_creator: Option<AaCreator>,
    #[serde(default)]
    evaluations: Option<AaEvaluations>,
    #[serde(default)]
    pricing: Option<AaPricing>,
    #[serde(default)]
    performance: Option<AaPerformance>,
}

#[derive(Deserialize, Default)]
struct AaCreator {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize, Default)]
struct AaEvaluations {
    #[serde(default)]
    artificial_analysis_intelligence_index: Option<f64>,
    #[serde(default)]
    artificial_analysis_coding_index: Option<f64>,
    #[serde(default)]
    artificial_analysis_agentic_index: Option<f64>,
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize, Default)]
struct AaPricing {
    #[serde(default)]
    price_1m_input_tokens: Option<f64>,
    #[serde(default)]
    price_1m_output_tokens: Option<f64>,
}

#[derive(Deserialize, Default)]
struct AaPerformance {
    #[serde(default)]
    median_output_tokens_per_second: Option<f64>,
    #[serde(default)]
    median_time_to_first_token_seconds: Option<f64>,
}

async fn fetch_all(api_key: &str) -> Result<Fetched, FetchError> {
    fetch_all_from(api_key, API_BASE).await
}

async fn fetch_all_from(api_key: &str, base: &str) -> Result<Fetched, FetchError> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("OriginLM/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| FetchError::new("The Artificial Analysis client could not start."))?;
    let mut by_slug: BTreeMap<String, IntelligenceModel> = BTreeMap::new();
    let mut index_version = None;
    let mut tier = None;
    let mut page = 1u32;
    loop {
        let response = client
            .get(format!("{base}{FREE_MODELS_PATH}"))
            .query(&[("page", page.to_string())])
            .header("x-api-key", api_key)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|_| {
                FetchError::new("Artificial Analysis is unreachable; cached scores remain visible.")
            })?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(|seconds| Duration::from_secs(seconds.clamp(60, 86_400)))
                .unwrap_or(Duration::from_secs(6 * 60 * 60));
            return Err(FetchError {
                message: "Artificial Analysis rate limit reached; cached scores remain visible."
                    .into(),
                retry_after: Some(retry_after),
            });
        }
        if !status.is_success() {
            let message = match status.as_u16() {
                401 => "Artificial Analysis rejected the API key (HTTP 401).",
                403 => "This Artificial Analysis key cannot read the free model list (HTTP 403).",
                _ => "Artificial Analysis returned an error; cached scores remain visible.",
            };
            return Err(FetchError::new(message));
        }
        let body: AaPage = response
            .json()
            .await
            .map_err(|_| FetchError::new("Artificial Analysis returned an unexpected response."))?;
        tier = body.tier.or(tier);
        index_version = index_version_number(body.intelligence_index_version).or(index_version);
        for item in body.data.into_iter().take(MAX_MODELS) {
            if let Some(model) = convert(item) {
                by_slug.insert(model.slug.clone(), model);
            }
        }
        if by_slug.len() > MAX_MODELS {
            return Err(FetchError::new(
                "Artificial Analysis returned more models than this cache accepts.",
            ));
        }
        let total_pages = body.pagination.total_pages.unwrap_or(page);
        if !body.pagination.has_more || page >= total_pages {
            break;
        }
        page += 1;
        if page > MAX_PAGES {
            return Err(FetchError::new(
                "Artificial Analysis pagination exceeded the cache limit.",
            ));
        }
    }
    Ok(Fetched {
        index_version,
        tier,
        models: by_slug.into_values().collect(),
    })
}

fn number(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && value.abs() <= 1_000_000_000.0)
}

fn index_version_number(value: Option<f64>) -> Option<u32> {
    value
        .filter(|value| value.is_finite() && (0.0..=1_000.0).contains(value))
        .map(|value| value.round() as u32)
}

fn convert(item: AaModel) -> Option<IntelligenceModel> {
    let slug = item.slug.trim();
    let name = item.name.trim();
    if slug.is_empty() || name.is_empty() || slug.len() > 256 || name.len() > 512 {
        return None;
    }
    let id = if item.id.trim().is_empty() {
        slug.to_owned()
    } else {
        item.id.trim().to_owned()
    };
    if id.len() > 128 {
        return None;
    }
    let creator = item.model_creator.unwrap_or_default().name;
    let creator = creator.trim();
    if creator.len() > 256 {
        return None;
    }
    let evaluations = item.evaluations.unwrap_or_default();
    let pricing = item.pricing.unwrap_or_default();
    let performance = item.performance.unwrap_or_default();
    let mut capability_indexes = BTreeMap::new();
    for (key, value) in evaluations.other {
        let Some(key) = key
            .strip_prefix("artificial_analysis_")
            .and_then(|key| key.strip_suffix("_index"))
        else {
            continue;
        };
        if matches!(key, "intelligence" | "coding" | "agentic") || key.len() > 96 {
            continue;
        }
        if let Some(value) = value.as_f64().and_then(|value| number(Some(value))) {
            capability_indexes.insert(key.to_owned(), value);
        }
    }
    Some(IntelligenceModel {
        id,
        slug: slug.to_owned(),
        name: name.to_owned(),
        creator: if creator.is_empty() {
            "Unknown".into()
        } else {
            creator.to_owned()
        },
        release_date: item
            .release_date
            .filter(|date| date.len() <= 32 && !date.trim().is_empty()),
        intelligence_index: number(evaluations.artificial_analysis_intelligence_index),
        coding_index: number(evaluations.artificial_analysis_coding_index),
        agentic_index: number(evaluations.artificial_analysis_agentic_index),
        capability_indexes,
        input_price_usd_per_million: number(pricing.price_1m_input_tokens),
        output_price_usd_per_million: number(pricing.price_1m_output_tokens),
        median_output_tokens_per_second: number(performance.median_output_tokens_per_second),
        median_time_to_first_token_seconds: number(performance.median_time_to_first_token_seconds),
    })
}

#[cfg(test)]
fn parse_page(bytes: &[u8]) -> Result<Vec<IntelligenceModel>, String> {
    let page: AaPage = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    Ok(page.data.into_iter().filter_map(convert).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(models: &str) -> AaPage {
        serde_json::from_str(models).unwrap()
    }

    #[test]
    fn missing_scores_stay_null() {
        let parsed = page(
            r#"{"tier":"free","intelligence_index_version":5,"pagination":{"has_more":false},"data":[
                {"id":"one","name":"Unknown Model","slug":"unknown-model","model_creator":{"name":"Example"}}
            ]}"#,
        );
        let model = convert(parsed.data.into_iter().next().unwrap()).unwrap();
        assert_eq!(model.intelligence_index, None);
        assert_eq!(model.coding_index, None);
        assert_eq!(model.agentic_index, None);
        assert!(model.capability_indexes.is_empty());
    }

    #[test]
    fn parses_free_fields_and_capability_indexes() {
        let bytes = br#"{"tier":"free","intelligence_index_version":5,"pagination":{"has_more":false},"data":[
            {"id":"abc","name":"GLM-5.3 (Max)","slug":"glm-5-3","release_date":"2026-08-18",
             "model_creator":{"name":"Z AI"},
             "evaluations":{"artificial_analysis_intelligence_index":31.7,
               "artificial_analysis_coding_index":null,
               "artificial_analysis_agentic_index":null,
               "artificial_analysis_legal_index":35.8,
               "artificial_analysis_economics_index":null},
             "pricing":{"price_1m_input_tokens":1.4,"price_1m_output_tokens":4.4},
             "performance":{"median_output_tokens_per_second":80.07,
               "median_time_to_first_token_seconds":2.22}}
        ]}"#;
        let models = parse_page(bytes).unwrap();
        assert_eq!(models.len(), 1);
        let model = &models[0];
        assert_eq!(model.slug, "glm-5-3");
        assert_eq!(model.intelligence_index, Some(31.7));
        assert_eq!(model.capability_indexes.get("legal"), Some(&35.8));
        assert!(!model.capability_indexes.contains_key("economics"));
        assert_eq!(model.input_price_usd_per_million, Some(1.4));
    }

    #[test]
    fn cache_round_trips_and_rejects_malformed_records() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(CACHE_FILE);
        let stored = StoredCache {
            fetched_at: Some("2026-11-05T10:00:00.000Z".into()),
            index_version: Some(5),
            tier: Some("free".into()),
            models: vec![IntelligenceModel {
                id: "abc".into(),
                slug: "glm-5-3".into(),
                name: "GLM-5.3".into(),
                creator: "Z AI".into(),
                release_date: None,
                intelligence_index: Some(31.7),
                coding_index: None,
                agentic_index: None,
                capability_indexes: BTreeMap::new(),
                input_price_usd_per_million: None,
                output_price_usd_per_million: None,
                median_output_tokens_per_second: None,
                median_time_to_first_token_seconds: None,
            }],
            default_sort_for_apps: true,
            ..StoredCache::default()
        };
        std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let loaded = read_cache(&path).unwrap().unwrap();
        assert_eq!(loaded.models.len(), 1);
        assert!(loaded.default_sort_for_apps);

        let mut malformed = stored;
        malformed.models[0].slug.clear();
        std::fs::write(&path, serde_json::to_vec(&malformed).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(read_cache(&path).is_err());
    }

    #[test]
    fn due_refresh_is_skipped_without_a_key() {
        let cache = IntelligenceCache {
            api_key: None,
            ..IntelligenceCache::default()
        };
        assert!(!cache.sync_due());
        assert!(!cache.catalog().configured);
    }

    #[test]
    fn cache_is_ready_for_the_first_sync_when_empty() {
        let cache = IntelligenceCache {
            api_key: Some("test-key".into()),
            ..IntelligenceCache::default()
        };
        assert!(cache.sync_due());
    }

    #[test]
    fn sync_reads_every_page_and_sends_the_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (requests, received) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0u8; 8192];
                let read = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                requests.send(request.clone()).unwrap();
                let body = if request.contains("page=1") {
                    r#"{"tier":"free","intelligence_index_version":5,"pagination":{"page":1,"page_size":200,"total_pages":2,"has_more":true},"data":[
                        {"id":"one","name":"High Model","slug":"high-model","model_creator":{"name":"Example"},
                         "evaluations":{"artificial_analysis_intelligence_index":80.0}}
                    ]}"#
                } else {
                    r#"{"tier":"free","intelligence_index_version":5,"pagination":{"page":2,"page_size":200,"total_pages":2,"has_more":false},"data":[
                        {"id":"two","name":"Unknown Model","slug":"unknown-model","model_creator":{"name":"Example"},
                         "evaluations":{"artificial_analysis_intelligence_index":null}}
                    ]}"#
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let fetched = runtime
            .block_on(fetch_all_from("secret-key", &format!("http://{address}")))
            .unwrap();
        server.join().unwrap();
        assert_eq!(fetched.index_version, Some(5));
        assert_eq!(fetched.models.len(), 2);
        assert_eq!(
            fetched
                .models
                .iter()
                .find(|model| model.slug == "high-model")
                .and_then(|model| model.intelligence_index),
            Some(80.0)
        );
        assert_eq!(
            fetched
                .models
                .iter()
                .find(|model| model.slug == "unknown-model")
                .and_then(|model| model.intelligence_index),
            None
        );
        for _ in 0..2 {
            let request = received.recv().unwrap().to_ascii_lowercase();
            assert!(request.contains("x-api-key: secret-key"));
            assert!(request.contains("/language/models/free"));
        }
    }
}
