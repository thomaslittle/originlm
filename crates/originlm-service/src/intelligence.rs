//! Durable Intelligence Index cache backed by RankLLMs.
//!
//! RankLLMs needs no key. One refresh is a single bounded leaderboard request,
//! so this lane runs at most four times a day. UI reads never call RankLLMs;
//! they read the stored catalog.
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

const API_BASE: &str = "https://api.rankllms.com/api/v1";
const LEADERBOARD_PATH: &str = "/leaderboard";
const LEADERBOARD_LIMIT: u32 = 500;
const CACHE_FILE: &str = "intelligence-cache.json";
const CACHE_VERSION: u32 = 2;
const MAX_CACHE_BYTES: usize = 1024 * 1024;
const MAX_MODELS: usize = 1_200;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// One request per sync, four syncs a day.
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
            path: None,
        }
    }
}

impl IntelligenceCache {
    /// Load the last cache. RankLLMs needs no key, so a valid cache plus this
    /// module is the only configuration the source requires.
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
            path: Some(path),
        }
    }

    pub(crate) fn default_sort_for_apps(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.stored.default_sort_for_apps)
            .unwrap_or(false)
    }

    /// The host enabled the durable cache by giving it a state directory.
    pub(crate) fn enabled(&self) -> bool {
        self.path.is_some()
    }

    pub(crate) fn catalog(&self) -> IntelligenceCatalog {
        let configured = self.enabled();
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
        if !self.enabled() {
            return false;
        }
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state
            .next_attempt
            .is_none_or(|next_attempt| next_attempt <= Instant::now())
    }

    /// Refresh the whole leaderboard. Never called from a read path; the
    /// background lane and an explicit administrator refresh are the only
    /// callers. Failures leave the previous cache untouched.
    pub(crate) async fn refresh(&self, force: bool) -> IntelligenceCatalog {
        if !self.enabled() {
            return self.catalog();
        }
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
        match fetch_all().await {
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
            .openrouter_id
            .as_ref()
            .is_none_or(|id| !id.is_empty() && id.len() <= 512)
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
struct RankPage {
    #[serde(default)]
    rankings: Vec<RankModel>,
}

#[derive(Deserialize, Default)]
struct RankModel {
    #[serde(default)]
    id: serde_json::Value,
    #[serde(default)]
    openrouter_id: Option<String>,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    intelligence_index: Option<f64>,
    #[serde(default)]
    coding_index: Option<f64>,
    #[serde(default)]
    agentic_index: Option<f64>,
    #[serde(default)]
    prompt_price_per_1m: Option<serde_json::Value>,
    #[serde(default)]
    completion_price_per_1m: Option<serde_json::Value>,
}

async fn fetch_all() -> Result<Fetched, FetchError> {
    fetch_all_from(API_BASE).await
}

async fn fetch_all_from(base: &str) -> Result<Fetched, FetchError> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("OriginLM/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| FetchError::new("The RankLLMs client could not start."))?;
    let response = client
        .get(format!("{base}{LEADERBOARD_PATH}"))
        .query(&[("limit", LEADERBOARD_LIMIT.to_string())])
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| FetchError::new("RankLLMs is unreachable; cached scores remain visible."))?;
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
            message: "RankLLMs rate limit reached; cached scores remain visible.".into(),
            retry_after: Some(retry_after),
        });
    }
    if !status.is_success() {
        return Err(FetchError::new(
            "RankLLMs returned an error; cached scores remain visible.",
        ));
    }
    let body: RankPage = response
        .json()
        .await
        .map_err(|_| FetchError::new("RankLLMs returned an unexpected response."))?;
    let mut by_slug: BTreeMap<String, IntelligenceModel> = BTreeMap::new();
    for item in body.rankings.into_iter().take(MAX_MODELS) {
        if let Some(model) = convert(item) {
            by_slug.insert(model.slug.clone(), model);
        }
    }
    if by_slug.is_empty() {
        return Err(FetchError::new(
            "RankLLMs returned no ranked models; the previous cache was kept.",
        ));
    }
    if by_slug.len() > MAX_MODELS {
        return Err(FetchError::new(
            "RankLLMs returned more models than this cache accepts.",
        ));
    }
    Ok(Fetched {
        index_version: None,
        tier: None,
        models: by_slug.into_values().collect(),
    })
}

fn number(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && value.abs() <= 1_000_000_000.0)
}

fn number_value(value: Option<serde_json::Value>) -> Option<f64> {
    let value = match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    number(Some(value))
}

fn id_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn convert(item: RankModel) -> Option<IntelligenceModel> {
    let slug = item.slug.trim();
    let name = item.name.trim();
    if slug.is_empty() || name.is_empty() || slug.len() > 256 || name.len() > 512 {
        return None;
    }
    let id = id_string(&item.id).unwrap_or_else(|| slug.to_owned());
    if id.len() > 128 {
        return None;
    }
    let creator = item.provider.trim();
    if creator.len() > 256 {
        return None;
    }
    Some(IntelligenceModel {
        id,
        slug: slug.to_owned(),
        openrouter_id: item
            .openrouter_id
            .filter(|id| !id.trim().is_empty() && id.len() <= 512),
        name: name.to_owned(),
        creator: if creator.is_empty() {
            "Unknown".into()
        } else {
            creator.to_owned()
        },
        release_date: None,
        intelligence_index: number(item.intelligence_index),
        coding_index: number(item.coding_index),
        agentic_index: number(item.agentic_index),
        capability_indexes: BTreeMap::new(),
        input_price_usd_per_million: number_value(item.prompt_price_per_1m),
        output_price_usd_per_million: number_value(item.completion_price_per_1m),
        median_output_tokens_per_second: None,
        median_time_to_first_token_seconds: None,
    })
}

#[cfg(test)]
fn parse_page(bytes: &[u8]) -> Result<Vec<IntelligenceModel>, String> {
    let page: RankPage = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    Ok(page.rankings.into_iter().filter_map(convert).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(models: &str) -> RankPage {
        serde_json::from_str(models).unwrap()
    }

    #[test]
    fn missing_scores_stay_null() {
        let parsed = page(
            r#"{"rankings":[
                {"id":7,"name":"Unknown Model","slug":"unknown-model","provider":"Example"}
            ]}"#,
        );
        let model = convert(parsed.rankings.into_iter().next().unwrap()).unwrap();
        assert_eq!(model.intelligence_index, None);
        assert_eq!(model.coding_index, None);
        assert_eq!(model.agentic_index, None);
        assert!(model.capability_indexes.is_empty());
    }

    #[test]
    fn parses_rankllms_fields_and_openrouter_ids() {
        let bytes =
            br#"{"sort_by":"intelligence","dedup":true,"exclude_nulls":true,"count":1,"rankings":[
            {"rank":1,"id":815,"openrouter_id":"aa/openai/gpt-5-5-pro","slug":"gpt-5-5-pro",
             "name":"GPT-5.5 Pro (xhigh)","provider":"OpenAI",
             "intelligence_index":92.0,"coding_index":86.5,"agentic_index":81.0,
             "prompt_price_per_1m":"1.250000","completion_price_per_1m":"10.000000"}
        ]}"#;
        let models = parse_page(bytes).unwrap();
        assert_eq!(models.len(), 1);
        let model = &models[0];
        assert_eq!(model.id, "815");
        assert_eq!(model.slug, "gpt-5-5-pro");
        assert_eq!(
            model.openrouter_id.as_deref(),
            Some("aa/openai/gpt-5-5-pro")
        );
        assert_eq!(model.intelligence_index, Some(92.0));
        assert_eq!(model.coding_index, Some(86.5));
        assert_eq!(model.agentic_index, Some(81.0));
        assert_eq!(model.input_price_usd_per_million, Some(1.25));
        assert_eq!(model.output_price_usd_per_million, Some(10.0));
    }

    #[test]
    fn cache_round_trips_and_rejects_malformed_records() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(CACHE_FILE);
        let stored = StoredCache {
            fetched_at: Some("2026-11-05T10:00:00.000Z".into()),
            index_version: None,
            tier: None,
            models: vec![IntelligenceModel {
                id: "abc".into(),
                slug: "glm-5-3".into(),
                openrouter_id: Some("aa/zai/glm-5-3".into()),
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
    fn rankllms_needs_no_key_and_starts_due() {
        let cache = IntelligenceCache {
            path: Some(PathBuf::from("intelligence-cache.json")),
            ..IntelligenceCache::default()
        };
        assert!(cache.sync_due());
        assert!(cache.catalog().configured);
    }

    #[test]
    fn sync_uses_one_leaderboard_request_without_a_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (requests, received) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            requests.send(request).unwrap();
            let body = r#"{"sort_by":"intelligence","dedup":true,"exclude_nulls":true,"count":2,"rankings":[
                {"rank":1,"id":815,"openrouter_id":"aa/openai/gpt-5-5-pro","slug":"gpt-5-5-pro",
                 "name":"GPT-5.5 Pro (xhigh)","provider":"OpenAI","intelligence_index":92.0,
                 "prompt_price_per_1m":"1.250000","completion_price_per_1m":"10.000000"},
                {"rank":2,"id":7,"name":"Unknown Model","slug":"unknown-model","provider":"Example",
                 "intelligence_index":null}
            ]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let fetched = runtime
            .block_on(fetch_all_from(&format!("http://{address}")))
            .unwrap();
        server.join().unwrap();
        assert_eq!(fetched.models.len(), 2);
        assert_eq!(
            fetched
                .models
                .iter()
                .find(|model| model.slug == "gpt-5-5-pro")
                .and_then(|model| model.intelligence_index),
            Some(92.0)
        );
        assert_eq!(
            fetched
                .models
                .iter()
                .find(|model| model.slug == "unknown-model")
                .and_then(|model| model.intelligence_index),
            None
        );
        let request = received.recv().unwrap().to_ascii_lowercase();
        assert!(request.contains("/leaderboard"));
        assert!(request.contains("limit=500"));
        assert!(!request.contains("x-api-key"));
    }
}
