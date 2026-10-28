//! Bounded, local usage history.
//!
//! The broker exposes quota windows as point-in-time observations. Nothing in
//! the protocol fabricates a time series, so the desktop client keeps the
//! observations it actually saw, and labels them as locally collected.

use chrono::{DateTime, Duration, Utc};
use originlm_contracts::{Observation, Snapshot, UsageReading};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
};

pub const MAX_SAMPLES: usize = 6000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub t: i64,
    pub pool: String,
    pub pool_label: String,
    pub window: String,
    pub window_label: String,
    /// Remaining share of the window, 0..=1. Absent when the provider only
    /// reports a raw reading without a limit.
    pub remaining: Option<f64>,
    pub resets_at: Option<String>,
    pub unit: Option<String>,
    pub used: Option<String>,
    pub limit: Option<String>,
}

pub fn state_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LAI_DESKTOP_STATE_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Some(dir);
    }
    originlm_platform::default_state_dir(None).map(|path| path.join("desktop"))
}

pub struct History {
    pub samples: Vec<Sample>,
    path: Option<PathBuf>,
}

impl History {
    pub fn load() -> Self {
        let path = state_dir().map(|path| path.join("usage-history.jsonl"));
        let mut samples = Vec::new();
        let cutoff = Utc::now() - Duration::days(30);
        if let Some(file) = path.as_ref().and_then(|path| fs::File::open(path).ok()) {
            for line in BufReader::new(file.take(16 * 1024 * 1024))
                .lines()
                .map_while(Result::ok)
            {
                if let Ok(sample) = serde_json::from_str::<Sample>(&line) {
                    let recent =
                        DateTime::from_timestamp(sample.t, 0).is_some_and(|stamp| stamp > cutoff);
                    if recent {
                        samples.push(sample);
                    }
                }
            }
        }
        if samples.len() > MAX_SAMPLES {
            let excess = samples.len() - MAX_SAMPLES;
            samples.drain(0..excess);
        }
        Self { samples, path }
    }

    /// Record one observation per quota window. A sample is only appended when
    /// the value changed, or when the previous sample for that window is more
    /// than five minutes old, so idle polling does not inflate the file.
    pub fn observe(&mut self, snapshot: &Snapshot, now: i64) {
        if snapshot.quota_pools.is_empty() {
            return;
        }
        let mut appended = Vec::new();
        for pool in &snapshot.quota_pools {
            let pool_label = pool_label(snapshot, &pool.id, &pool.connection_ids);
            for window in &pool.windows {
                let Observation::Known {
                    value,
                    freshness,
                    observed_at,
                    ..
                } = &window.reading
                else {
                    continue;
                };
                if *freshness != originlm_contracts::Freshness::Fresh {
                    continue;
                }
                let Ok(observed) = DateTime::parse_from_rfc3339(observed_at.as_str()) else {
                    continue;
                };
                let observed = observed.timestamp();
                if now - observed > 30 * 86400 {
                    continue;
                }
                let UsageReading {
                    unit,
                    used,
                    limit,
                    remaining: _,
                    used_fraction,
                } = value;
                let remaining_share =
                    used_fraction.map(|fraction| (1.0 - fraction).clamp(0.0, 1.0));
                let previous = self
                    .samples
                    .iter()
                    .rev()
                    .find(|sample| sample.pool == pool.id && sample.window == window.id);
                let changed = previous.is_none_or(|last| {
                    last.remaining != remaining_share
                        || last.used.as_deref() != Some(used.as_str())
                        || last.limit.as_deref() != limit.as_ref().map(|limit| limit.as_str())
                        || last.resets_at.as_deref()
                            != window.resets_at.as_ref().map(|at| at.as_str())
                });
                let fresh_enough = previous.is_none_or(|last| observed - last.t >= 300);
                if !changed && !fresh_enough {
                    continue;
                }
                let sample = Sample {
                    t: observed,
                    pool: pool.id.clone(),
                    pool_label: pool_label.clone(),
                    window: window.id.clone(),
                    window_label: window.label.clone(),
                    remaining: remaining_share,
                    resets_at: window.resets_at.as_ref().map(|t| t.as_str().to_string()),
                    unit: Some(unit.clone()),
                    used: Some(used.as_str().to_string()),
                    limit: limit.as_ref().map(|l| l.as_str().to_string()),
                };
                self.samples.push(sample.clone());
                appended.push(sample);
            }
        }
        if self.samples.len() > MAX_SAMPLES {
            let excess = self.samples.len() - MAX_SAMPLES;
            self.samples.drain(0..excess);
        }
        if !appended.is_empty() {
            self.persist();
        }
    }

    fn persist(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let temporary = parent.join(format!(
            ".usage-history-{}-{}.tmp",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| -> std::io::Result<()> {
            let mut file = options.open(&temporary)?;
            for sample in &self.samples {
                serde_json::to_writer(&mut file, sample)?;
                writeln!(file)?;
            }
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
    }
}

fn pool_label(snapshot: &Snapshot, pool_id: &str, connection_ids: &[String]) -> String {
    let provider = connection_ids
        .iter()
        .find_map(|id| snapshot.connections.iter().find(|c| &c.id == id))
        .and_then(|c| {
            snapshot
                .providers
                .iter()
                .find(|p| p.id == c.provider_id)
                .map(|p| p.name.clone())
                .or_else(|| Some(c.provider_id.clone()))
        });
    match provider {
        Some(name) => name,
        None => pool_id
            .strip_prefix("pool_")
            .map(|rest| format!("Pool {}", &rest[..rest.len().min(6)]))
            .unwrap_or_else(|| pool_id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_poll_of_multiple_windows_does_not_invent_history() {
        let snapshot: Snapshot =
            serde_json::from_str(include_str!("../assets/example-snapshot.json")).unwrap();
        let mut history = History {
            samples: Vec::new(),
            path: None,
        };
        let now = snapshot
            .quota_pools
            .iter()
            .flat_map(|pool| &pool.windows)
            .filter_map(|window| match &window.reading {
                Observation::Known { observed_at, .. } => {
                    DateTime::parse_from_rfc3339(observed_at.as_str())
                        .ok()
                        .map(|at| at.timestamp())
                }
                _ => None,
            })
            .max()
            .unwrap();
        history.observe(&snapshot, now);
        let count = history.samples.len();
        assert!(count > 0);
        history.observe(&snapshot, now + 60);
        history.observe(&snapshot, now + 600);
        assert_eq!(
            history.samples.len(),
            count,
            "unchanged observation timestamps must not gain fresh timestamps from polling"
        );
    }
    #[test]
    fn stale_observations_are_not_recorded_as_new_usage() {
        let mut snapshot: Snapshot =
            serde_json::from_str(include_str!("../assets/example-snapshot.json")).unwrap();
        for pool in &mut snapshot.quota_pools {
            for window in &mut pool.windows {
                if let Observation::Known { freshness, .. } = &mut window.reading {
                    *freshness = originlm_contracts::Freshness::Stale;
                }
            }
        }
        let mut history = History {
            samples: Vec::new(),
            path: None,
        };
        history.observe(&snapshot, Utc::now().timestamp());
        assert!(history.samples.is_empty());
    }
}
