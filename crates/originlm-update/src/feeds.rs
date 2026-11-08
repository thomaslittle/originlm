//! Optional, read-only release and catalog requests. No installation or telemetry events.
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::Read,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

pub const DEFAULT_BASE_URL: &str = "https://originlm.livestreaming.tools";
pub const RELEASE_PATH: &str = "v1/releases/latest.json";
pub const CATALOG_PATH: &str = "v1/catalog.json";
const MAX_RELEASE: u64 = 256 * 1024;
const MAX_CATALOG: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppIdentity {
    pub name: String,
    pub version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedConfig {
    pub enabled: bool,
    pub base_url: String,
    pub interval_seconds: u64,
    pub app: Option<AppIdentity>,
}
impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: DEFAULT_BASE_URL.into(),
            interval_seconds: 86_400,
            app: None,
        }
    }
}
#[derive(Default, Clone, Debug)]
pub struct FeedEnvironment {
    pub base_url: Option<String>,
    pub do_not_track: Option<String>,
}
impl FeedEnvironment {
    pub fn from_process() -> Self {
        Self {
            base_url: std::env::var("ORIGINLM_FEED_BASE_URL").ok(),
            do_not_track: std::env::var("DO_NOT_TRACK").ok(),
        }
    }
    fn disabled(&self) -> bool {
        self.do_not_track.as_deref().is_some_and(|v| {
            !v.trim().is_empty() && v.trim() != "0" && !v.trim().eq_ignore_ascii_case("false")
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatestRelease {
    pub version: String,
    pub channel: String,
    pub published_at: String,
    pub notes: String,
}
impl LatestRelease {
    /// Compare release metadata without downloading or installing an update.
    pub fn is_newer_than(&self, current: &str) -> Result<bool, semver::Error> {
        Ok(semver::Version::parse(&self.version)? > semver::Version::parse(current)?)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogProvider {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub models: Vec<CatalogModel>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogFeed {
    pub schema_version: u32,
    pub generated_at: String,
    pub providers: Vec<CatalogProvider>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedError {
    InvalidConfiguration,
    Transport,
    HttpStatus(u16),
    TooLarge,
    InvalidFeed,
}
impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfiguration => f.write_str("Invalid optional feed configuration"),
            Self::Transport => f.write_str("Feed request unavailable"),
            Self::HttpStatus(code) => write!(f, "Feed HTTP status {code}"),
            Self::TooLarge => f.write_str("Feed response exceeds its size limit"),
            Self::InvalidFeed => f.write_str("Feed JSON does not match the supported schema"),
        }
    }
}
impl std::error::Error for FeedError {}
#[derive(Clone, Debug, Default)]
pub struct FeedSnapshot {
    pub release: Option<LatestRelease>,
    pub catalog: Option<CatalogFeed>,
    pub errors: Vec<FeedError>,
}
#[derive(Clone, Debug)]
pub enum PollOutcome {
    Skipped,
    Checked(FeedSnapshot),
}

pub struct FeedClient {
    config: FeedConfig,
    base: Option<Url>,
    http: Option<Client>,
    last_attempt: Option<Instant>,
    snapshot: FeedSnapshot,
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:/".contains(&c))
}
fn name(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn timestamp(s: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
}
impl FeedClient {
    /// Inert construction. Environment override wins over explicit config, then the default.
    pub fn new(config: FeedConfig) -> Result<Self, FeedError> {
        Self::with_environment(config, FeedEnvironment::from_process())
    }
    pub fn with_environment(
        config: FeedConfig,
        environment: FeedEnvironment,
    ) -> Result<Self, FeedError> {
        if !config.enabled || environment.disabled() {
            return Ok(Self {
                config,
                base: None,
                http: None,
                last_attempt: None,
                snapshot: FeedSnapshot::default(),
            });
        }
        if !(60..=604_800).contains(&config.interval_seconds) {
            return Err(FeedError::InvalidConfiguration);
        }
        if let Some(app) = &config.app {
            if !identifier(&app.name)
                || app.name.contains([':', '/'])
                || semver::Version::parse(&app.version).is_err()
                || app.version.len() > 128
            {
                return Err(FeedError::InvalidConfiguration);
            }
        }
        let mut base = Url::parse(environment.base_url.as_deref().unwrap_or(&config.base_url))
            .map_err(|_| FeedError::InvalidConfiguration)?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(FeedError::InvalidConfiguration);
        }
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()))
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .redirect(Policy::none())
            .user_agent(concat!("originlm/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| FeedError::Transport)?;
        Ok(Self {
            config,
            base: Some(base),
            http: Some(http),
            last_attempt: None,
            snapshot: FeedSnapshot::default(),
        })
    }
    pub fn is_enabled(&self) -> bool {
        self.base.is_some()
    }
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.config.interval_seconds)
    }
    fn fetch<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        limit: u64,
    ) -> Result<T, FeedError> {
        let url = self
            .base
            .as_ref()
            .ok_or(FeedError::InvalidConfiguration)?
            .join(path)
            .map_err(|_| FeedError::InvalidConfiguration)?;
        let mut request = self
            .http
            .as_ref()
            .ok_or(FeedError::InvalidConfiguration)?
            .get(url);
        if let Some(app) = &self.config.app {
            request = request.header("X-OriginLM-App", format!("{}/{}", app.name, app.version));
        }
        let response = request.send().map_err(|_| FeedError::Transport)?;
        if !response.status().is_success() {
            return Err(FeedError::HttpStatus(response.status().as_u16()));
        }
        if response.content_length().is_some_and(|n| n > limit) {
            return Err(FeedError::TooLarge);
        }
        let mut bytes = Vec::new();
        response
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| FeedError::Transport)?;
        if bytes.len() as u64 > limit {
            return Err(FeedError::TooLarge);
        }
        serde_json::from_slice(&bytes).map_err(|_| FeedError::InvalidFeed)
    }
    pub fn poll_at(&mut self, now: Instant) -> PollOutcome {
        if !self.is_enabled()
            || self
                .last_attempt
                .is_some_and(|last| now.saturating_duration_since(last) < self.interval())
        {
            return PollOutcome::Skipped;
        }
        self.last_attempt = Some(now);
        self.snapshot.errors.clear();
        let release = self
            .fetch::<LatestRelease>(RELEASE_PATH, MAX_RELEASE)
            .and_then(|r| {
                if semver::Version::parse(&r.version).is_err()
                    || !identifier(&r.channel)
                    || !timestamp(&r.published_at)
                    || r.notes.len() > 64 * 1024
                {
                    Err(FeedError::InvalidFeed)
                } else {
                    Ok(r)
                }
            });
        match release {
            Ok(r) => self.snapshot.release = Some(r),
            Err(e) => self.snapshot.errors.push(e),
        }
        let catalog = self
            .fetch::<CatalogFeed>(CATALOG_PATH, MAX_CATALOG)
            .and_then(|c| {
                let mut providers = HashSet::new();
                let mut total = 0;
                if c.schema_version != 1 || !timestamp(&c.generated_at) || c.providers.len() > 256 {
                    return Err(FeedError::InvalidFeed);
                }
                for p in &c.providers {
                    if !identifier(&p.id)
                        || !name(&p.name)
                        || !providers.insert(&p.id)
                        || p.models.len() > 4096
                    {
                        return Err(FeedError::InvalidFeed);
                    }
                    let mut ids = HashSet::new();
                    total += p.models.len();
                    for m in &p.models {
                        if !identifier(&m.id)
                            || !name(&m.name)
                            || !ids.insert(&m.id)
                            || m.context_window == Some(0)
                        {
                            return Err(FeedError::InvalidFeed);
                        }
                    }
                }
                if total > 16_384 {
                    return Err(FeedError::InvalidFeed);
                }
                Ok(c)
            });
        match catalog {
            Ok(c) => self.snapshot.catalog = Some(c),
            Err(e) => self.snapshot.errors.push(e),
        }
        PollOutcome::Checked(self.snapshot.clone())
    }
}
/// Owns a periodic read-only check. Drop signals cancellation; an in-flight bounded
/// poll (at most two bounded requests) may finish; subsequent polls stop.
pub struct FeedWorker {
    stop: mpsc::Sender<()>,
    pub snapshot: Arc<Mutex<Option<FeedSnapshot>>>,
}
impl FeedWorker {
    pub fn start(config: FeedConfig) -> Result<Option<Self>, FeedError> {
        let mut client = FeedClient::new(config)?;
        if !client.is_enabled() {
            return Ok(None);
        }
        let interval = client.interval();
        let (stop, receiver) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(None));
        let output = snapshot.clone();
        thread::Builder::new()
            .name("originlm-feeds".into())
            .spawn(move || {
                loop {
                    if receiver.try_recv().is_ok() {
                        break;
                    }
                    if let PollOutcome::Checked(result) = client.poll_at(Instant::now()) {
                        if let Ok(mut slot) = output.lock() {
                            *slot = Some(result)
                        }
                    }
                    if receiver.recv_timeout(interval) != Err(mpsc::RecvTimeoutError::Timeout) {
                        break;
                    }
                }
            })
            .map_err(|_| FeedError::Transport)?;
        Ok(Some(Self { stop, snapshot }))
    }
}
impl Drop for FeedWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}
