//! Opt-in, non-recursive OS notifications. No file contents or credentials are read.
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use originlm_contracts::{DomainError, ErrorCode, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const NAMES: &[&str] = &[
    "auth.json",
    ".credentials.json",
    "credentials.json",
    "settings.json",
    "config.toml",
    "opencode.json",
    "opencode.jsonc",
    "oauth_creds.json",
    "google_accounts.json",
    "models.json",
    "state.vscdb",
];
pub struct MetadataWatcher {
    _watcher: RecommendedWatcher,
    active: Arc<AtomicBool>,
}
fn key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.trim_start_matches(r"\\?\")
            .replace('\\', "/")
            .to_lowercase()
    } else {
        s.into_owned()
    }
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Metadata watch roots are invalid or unavailable",
    )
}
impl MetadataWatcher {
    pub fn start(
        profiles: BTreeMap<String, PathBuf>,
        changed: Arc<dyn Fn(Vec<String>) + Send + Sync>,
    ) -> Result<Self> {
        if profiles.len() > 64 {
            return Err(invalid());
        }
        let mut roots: BTreeMap<String, (PathBuf, BTreeSet<String>)> = BTreeMap::new();
        for (id, path) in profiles {
            if id.is_empty() || id.len() > 256 || !path.is_absolute() {
                return Err(invalid());
            }
            let metadata = std::fs::symlink_metadata(&path).map_err(|_| invalid())?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(invalid());
            }
            let canonical = path.canonicalize().map_err(|_| invalid())?;
            roots
                .entry(key(&canonical))
                .or_insert_with(|| (canonical, BTreeSet::new()))
                .1
                .insert(id);
        }
        let targets = Arc::new(roots);
        let callback_targets = targets.clone();
        let active = Arc::new(AtomicBool::new(true));
        let callback_active = active.clone();
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                if !callback_active.load(Ordering::Acquire) {
                    return;
                }
                let mut ids = BTreeSet::new();
                match result {
                    Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                        for path in event.paths {
                            if let Some((_, connections)) = callback_targets.get(&key(&path)) {
                                ids.extend(connections.iter().cloned());
                            }
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(str::to_ascii_lowercase);
                            if name.as_deref().is_some_and(|n| NAMES.contains(&n))
                                && let Some((_, connections)) =
                                    path.parent().and_then(|p| callback_targets.get(&key(p)))
                            {
                                ids.extend(connections.iter().cloned());
                            }
                        }
                    }
                    Err(_) => {
                        for (_, connections) in callback_targets.values() {
                            ids.extend(connections.iter().cloned());
                        }
                    }
                    _ => {}
                }
                if !ids.is_empty() && callback_active.load(Ordering::Acquire) {
                    changed(ids.into_iter().collect());
                }
            },
            Config::default().with_follow_symlinks(false),
        )
        .map_err(|_| invalid())?;
        for (path, _) in targets.values() {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|_| invalid())?;
        }
        Ok(Self {
            _watcher: watcher,
            active,
        })
    }
}
impl Drop for MetadataWatcher {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

/// Filesystem events are bursty. This wrapper coalesces them into one bounded
/// invalidation pass after a quiet period so a credential refresh that rewrites
/// several files cannot trigger one probe per file.
pub struct DebouncedMetadataWatcher {
    _watcher: MetadataWatcher,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

#[derive(Default)]
struct Batch {
    dirty: BTreeSet<String>,
    last_event: Option<Instant>,
}

impl DebouncedMetadataWatcher {
    pub fn start(
        profiles: BTreeMap<String, PathBuf>,
        delay: Duration,
        changed: Arc<dyn Fn(Vec<String>) + Send + Sync>,
    ) -> Result<Self> {
        let delay = delay.clamp(Duration::from_millis(50), Duration::from_secs(5));
        let batch = Arc::new(Mutex::new(Batch::default()));
        let callback_batch = batch.clone();
        let watcher = MetadataWatcher::start(
            profiles,
            Arc::new(move |ids| {
                if let Ok(mut batch) = callback_batch.lock() {
                    batch.dirty.extend(ids);
                    batch.last_event = Some(Instant::now());
                }
            }),
        )?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("originlm-metadata-debounce".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(50));
                    let ready = batch.lock().ok().and_then(|mut batch| {
                        let quiet = batch.last_event.is_some_and(|at| at.elapsed() >= delay);
                        if quiet && !batch.dirty.is_empty() {
                            batch.last_event = None;
                            Some(std::mem::take(&mut batch.dirty).into_iter().collect())
                        } else {
                            None
                        }
                    });
                    if let Some(ids) = ready {
                        changed(ids);
                    }
                }
            })
            .map_err(|_| invalid())?;
        Ok(Self {
            _watcher: watcher,
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for DebouncedMetadataWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
