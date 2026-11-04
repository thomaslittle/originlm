//! Bounded, read-only session-history token totals.
//!
//! This is the explicit usage-history path, not passive discovery and not
//! import-time work. It reads only the provider layouts that OriginLM already
//! knows about, never credentials, prompts, or tool output. A file is rejected
//! when it is too large; a malformed line is counted and skipped rather than
//! silently treated as zero usage.
//!
//! Normalized totals:
//! - `input_tokens` is fresh input only. Claude already reports it that way;
//!   Codex counts cache reads and writes inside `inputTokens`, so both are
//!   subtracted here and kept in their own fields.
//! - `cached_input_tokens` and `cache_creation_tokens` are separate.
//! - `reasoning_tokens` is a subset of `output_tokens`, never added to it.

use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

const DEFAULT_MAX_FILES: usize = 4096;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

impl UsageTotals {
    fn add(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }

    pub fn total_input_tokens(self) -> u64 {
        self.input_tokens
            .saturating_add(self.cached_input_tokens)
            .saturating_add(self.cache_creation_tokens)
    }

    fn is_zero(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub provider_id: String,
    pub native_model_id: String,
    pub sessions: u32,
    pub messages: u32,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub provider_id: String,
    pub roots: Vec<String>,
    pub files_read: u32,
    pub files_skipped: u32,
    pub malformed_lines: u32,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
    pub models: Vec<ModelUsage>,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistory {
    pub providers: Vec<ProviderUsage>,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_line_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_FILES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        }
    }
}

/// Read the default Claude Code and Codex histories under one home.
pub fn read_usage(home: &Path, limits: Limits) -> UsageHistory {
    read_roots(
        config_roots(&home.join(".claude"), ["projects", "transcripts"]),
        config_roots(&home.join(".codex"), ["sessions", "archived_sessions"]),
        limits,
    )
}

/// Read the histories selected by a discovery environment, including
/// `CODEX_HOME` and `CLAUDE_CONFIG_DIR`. `CLAUDE_SECURESTORAGE_CONFIG_DIR` is
/// deliberately not used here: it moves the credential store, not `projects/`.
pub fn read_usage_for_environment(
    environment: &originlm_discovery::ScanEnvironment,
    limits: Limits,
) -> UsageHistory {
    let claude_home = environment
        .overrides
        .get("CLAUDE_CONFIG_DIR")
        .cloned()
        .unwrap_or_else(|| environment.home.join(".claude"));
    let codex_home = originlm_discovery::profile_roots("codex", environment)
        .into_iter()
        .next()
        .unwrap_or_else(|| environment.home.join(".codex"));
    read_roots(
        config_roots(&claude_home, ["projects", "transcripts"]),
        config_roots(&codex_home, ["sessions", "archived_sessions"]),
        limits,
    )
}

fn config_roots<const N: usize>(root: &Path, relatives: [&str; N]) -> Vec<PathBuf> {
    existing_roots(relatives.map(|relative| root.join(relative)))
}

fn read_roots(
    claude_roots: Vec<PathBuf>,
    codex_roots: Vec<PathBuf>,
    limits: Limits,
) -> UsageHistory {
    let mut providers = Vec::new();
    if !claude_roots.is_empty() {
        providers.push(read_claude(&claude_roots, limits));
    }
    if !codex_roots.is_empty() {
        providers.push(read_codex(&codex_roots, limits));
    }
    UsageHistory { providers }
}

fn existing_roots(roots: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    roots.into_iter().filter(|root| root.is_dir()).collect()
}

#[derive(Default)]
struct Walk {
    files: Vec<PathBuf>,
    skipped: u32,
    diagnostics: BTreeSet<String>,
    truncated: bool,
}

fn walk_jsonl(root: &Path, limits: Limits, walk: &mut Walk) {
    fn recurse(path: &Path, depth: usize, limits: Limits, walk: &mut Walk) {
        if depth > 8 || walk.files.len() >= limits.max_files {
            walk.truncated = true;
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            walk.diagnostics
                .insert("SESSION_DIRECTORY_UNREADABLE".into());
            return;
        };
        for entry in entries.flatten() {
            if walk.files.len() >= limits.max_files {
                walk.truncated = true;
                return;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                walk.diagnostics
                    .insert("SESSION_ENTRY_SYMLINK_SKIPPED".into());
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                recurse(&path, depth + 1, limits, walk);
            } else if file_type.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".jsonl") {
                    match entry.metadata() {
                        Ok(metadata) if metadata.len() > limits.max_file_bytes => {
                            walk.skipped += 1;
                            walk.diagnostics
                                .insert("SESSION_FILE_TOO_LARGE_SKIPPED".into());
                        }
                        Ok(_) => walk.files.push(path),
                        Err(_) => {
                            walk.skipped += 1;
                            walk.diagnostics
                                .insert("SESSION_FILE_METADATA_UNAVAILABLE".into());
                        }
                    }
                } else if name.ends_with(".jsonl.zst") {
                    walk.skipped += 1;
                    walk.diagnostics.insert("COMPRESSED_SESSION_SKIPPED".into());
                }
            }
        }
    }
    recurse(root, 0, limits, walk);
    walk.files.sort();
}

/// Read one JSONL file. The callback returns false for a line it cannot use;
/// those are counted, while the remaining lines still contribute.
fn read_lines(
    path: &Path,
    limits: Limits,
    budget: &mut u64,
    mut callback: impl FnMut(&str) -> bool,
) -> (bool, u32) {
    let Ok(metadata) = fs::metadata(path) else {
        return (false, 0);
    };
    if metadata.len() > limits.max_file_bytes || metadata.len() > *budget {
        return (false, 0);
    }
    *budget = budget.saturating_sub(metadata.len());
    let Ok(file) = fs::File::open(path) else {
        return (false, 0);
    };
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut rejected = 0;
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => return (false, rejected),
        }
        if line.len() > limits.max_line_bytes {
            rejected = rejected.saturating_add(1);
            continue;
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.trim().is_empty() {
            continue;
        }
        if !callback(text) {
            rejected = rejected.saturating_add(1);
        }
    }
    (true, rejected)
}

#[derive(Debug, Clone)]
struct ClaudeRecord {
    session: String,
    model: String,
    totals: UsageTotals,
    reports_split: bool,
}

fn read_claude(roots: &[PathBuf], limits: Limits) -> ProviderUsage {
    let mut walk = Walk::default();
    for root in roots {
        walk_jsonl(root, limits, &mut walk);
    }
    let mut records: BTreeMap<String, ClaudeRecord> = BTreeMap::new();
    let mut malformed_lines = 0;
    let mut files_read = 0;
    let mut files_skipped = walk.skipped;
    let mut budget = limits.max_total_bytes;
    for path in &walk.files {
        let (read, rejected) = read_lines(path, limits, &mut budget, |line| {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                return false;
            };
            if value["type"].as_str() != Some("assistant") {
                return true;
            }
            let usage = &value["message"]["usage"];
            if !usage.is_object() {
                return true;
            }
            let model = value["message"]["model"]
                .as_str()
                .map(str::trim)
                .filter(|model| !model.is_empty() && model.len() <= 256)
                .unwrap_or("unknown")
                .to_owned();
            let session = value["sessionId"]
                .as_str()
                .map(str::trim)
                .filter(|session| !session.is_empty() && session.len() <= 256)
                .map(str::to_owned)
                .or_else(|| {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "unknown".into());
            let request = value["requestId"].as_str().unwrap_or("");
            let message = value["message"]["id"].as_str().unwrap_or("");
            let key = if request.is_empty() && message.is_empty() {
                format!("{}:{}", path.display(), records.len())
            } else {
                format!("{request}:{message}")
            };
            let reports_split = usage.get("cache_read_input_tokens").is_some()
                || usage.get("cache_creation_input_tokens").is_some();
            let count = |key: &str| usage[key].as_u64().unwrap_or(0);
            let reasoning = usage["output_tokens_details"]["thinking_tokens"]
                .as_u64()
                .unwrap_or(0);
            let total = UsageTotals {
                input_tokens: count("input_tokens"),
                cached_input_tokens: count("cache_read_input_tokens"),
                cache_creation_tokens: count("cache_creation_input_tokens"),
                output_tokens: count("output_tokens"),
                reasoning_tokens: reasoning,
            };
            if let Some(existing) = records.get_mut(&key) {
                // Streaming writes the same message several times. Each field
                // is a partial snapshot; take the max. A cache-split snapshot
                // supersedes an earlier whole-prompt snapshot's input count.
                if reports_split && !existing.reports_split {
                    existing.totals.input_tokens = total.input_tokens;
                } else if reports_split == existing.reports_split {
                    existing.totals.input_tokens =
                        existing.totals.input_tokens.max(total.input_tokens);
                }
                existing.totals.cached_input_tokens = existing
                    .totals
                    .cached_input_tokens
                    .max(total.cached_input_tokens);
                existing.totals.cache_creation_tokens = existing
                    .totals
                    .cache_creation_tokens
                    .max(total.cache_creation_tokens);
                existing.totals.output_tokens =
                    existing.totals.output_tokens.max(total.output_tokens);
                existing.totals.reasoning_tokens =
                    existing.totals.reasoning_tokens.max(total.reasoning_tokens);
                existing.reports_split |= reports_split;
            } else if !total.is_zero() {
                records.insert(
                    key,
                    ClaudeRecord {
                        session,
                        model,
                        totals: total,
                        reports_split,
                    },
                );
            }
            true
        });
        if read {
            files_read += 1;
            malformed_lines += rejected;
        } else {
            files_skipped += 1;
            walk.diagnostics.insert("SESSION_FILE_SKIPPED".into());
        }
    }
    let mut by_model: BTreeMap<String, (UsageTotals, BTreeSet<String>, u32)> = BTreeMap::new();
    for record in records.into_values() {
        let entry = by_model.entry(record.model).or_default();
        entry.0.add(record.totals);
        entry.1.insert(record.session);
        entry.2 = entry.2.saturating_add(1);
    }
    let mut models = Vec::new();
    let mut totals = UsageTotals::default();
    for (model, (usage, sessions, messages)) in by_model {
        totals.add(usage);
        models.push(ModelUsage {
            provider_id: "claude_code".into(),
            native_model_id: model,
            sessions: sessions.len().try_into().unwrap_or(u32::MAX),
            messages,
            totals: usage,
        });
    }
    models.sort_by(|left, right| left.native_model_id.cmp(&right.native_model_id));
    ProviderUsage {
        provider_id: "claude_code".into(),
        roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        files_read,
        files_skipped,
        malformed_lines,
        truncated: walk.truncated,
        diagnostics: walk.diagnostics.into_iter().collect(),
        models,
        totals,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Breakdown {
    input: u64,
    cached: u64,
    cache_write: Option<u64>,
    output: u64,
    reasoning: u64,
}

impl Breakdown {
    fn from_json(value: &serde_json::Value) -> Option<Self> {
        let input = value["inputTokens"].as_u64()?;
        let output = value["outputTokens"].as_u64()?;
        Some(Self {
            input,
            cached: value["cachedInputTokens"].as_u64().unwrap_or(0),
            cache_write: value["cacheWriteInputTokens"].as_u64(),
            output,
            reasoning: value["reasoningOutputTokens"].as_u64().unwrap_or(0),
        })
    }

    fn checked_sub(self, previous: Self) -> Option<Self> {
        Some(Self {
            input: self.input.checked_sub(previous.input)?,
            cached: self.cached.checked_sub(previous.cached)?,
            cache_write: match (self.cache_write, previous.cache_write) {
                (Some(current), Some(previous)) => Some(current.checked_sub(previous)?),
                _ => None,
            },
            output: self.output.checked_sub(previous.output)?,
            reasoning: self.reasoning.checked_sub(previous.reasoning)?,
        })
    }

    fn normalized(self) -> UsageTotals {
        let cached = self.cached.min(self.input);
        let cache_write = self
            .cache_write
            .unwrap_or(0)
            .min(self.input.saturating_sub(cached));
        UsageTotals {
            input_tokens: self
                .input
                .saturating_sub(cached)
                .saturating_sub(cache_write),
            cached_input_tokens: cached,
            cache_creation_tokens: cache_write,
            output_tokens: self.output,
            reasoning_tokens: self.reasoning.min(self.output),
        }
    }
}

#[derive(Debug, Clone)]
struct CodexEvent {
    session: String,
    timestamp: String,
    sequence: usize,
    model: String,
    total: Option<Breakdown>,
    last: Option<Breakdown>,
}

fn read_codex(roots: &[PathBuf], limits: Limits) -> ProviderUsage {
    let mut walk = Walk::default();
    for root in roots {
        walk_jsonl(root, limits, &mut walk);
    }
    let mut events =
        BTreeMap::<(String, String, Option<Breakdown>, Option<Breakdown>), CodexEvent>::new();
    let mut malformed_lines = 0;
    let mut files_read = 0;
    let mut files_skipped = walk.skipped;
    let mut budget = limits.max_total_bytes;
    let mut sequence = 0usize;
    for path in &walk.files {
        let fallback_session = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        let mut session = fallback_session;
        let mut model = "unknown".to_owned();
        let (read, rejected) = read_lines(path, limits, &mut budget, |line| {
            sequence = sequence.saturating_add(1);
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                return false;
            };
            let payload = &value["payload"];
            if value["type"].as_str() == Some("session_meta") {
                if let Some(id) = payload["id"]
                    .as_str()
                    .map(str::trim)
                    .filter(|id| !id.is_empty() && id.len() <= 256)
                {
                    session = id.to_owned();
                }
                return true;
            }
            if value["type"].as_str() == Some("turn_context") {
                if let Some(name) = payload["model"]
                    .as_str()
                    .map(str::trim)
                    .filter(|name| !name.is_empty() && name.len() <= 256)
                {
                    model = name.to_owned();
                }
                return true;
            }
            let token_count = if value["type"].as_str() == Some("event_msg")
                && payload["type"].as_str() == Some("token_count")
            {
                Some(payload)
            } else if value["type"].as_str() == Some("token_count") {
                Some(&value)
            } else {
                None
            };
            let Some(token_count) = token_count else {
                return true;
            };
            let info = token_count.get("info").unwrap_or(token_count);
            let total = Breakdown::from_json(&info["total_token_usage"]);
            let last = Breakdown::from_json(&info["last_token_usage"]);
            if total.is_none() && last.is_none() {
                return true;
            }
            let timestamp = value["timestamp"]
                .as_str()
                .map(str::trim)
                .filter(|stamp| !stamp.is_empty() && stamp.len() <= 64)
                .unwrap_or("")
                .to_owned();
            let map_key = (session.clone(), timestamp.clone(), total, last);
            events.entry(map_key).or_insert_with(|| CodexEvent {
                session: session.clone(),
                timestamp,
                sequence,
                model: model.clone(),
                total,
                last,
            });
            true
        });
        if read {
            files_read += 1;
            malformed_lines += rejected;
        } else {
            files_skipped += 1;
            walk.diagnostics.insert("SESSION_FILE_SKIPPED".into());
        }
    }
    let mut ordered: Vec<_> = events.into_values().collect();
    ordered.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.sequence.cmp(&right.sequence))
    });
    let mut baselines: BTreeMap<String, Breakdown> = BTreeMap::new();
    let mut current_models: BTreeMap<String, String> = BTreeMap::new();
    let mut by_model: BTreeMap<String, (UsageTotals, BTreeSet<String>, u32)> = BTreeMap::new();
    for event in ordered {
        let model = if event.model == "unknown" {
            current_models
                .get(&event.session)
                .cloned()
                .unwrap_or_else(|| "unknown".into())
        } else {
            current_models.insert(event.session.clone(), event.model.clone());
            event.model
        };
        let previous = baselines.get(&event.session).copied();
        let delta = match (event.total, event.last) {
            (Some(total), last) => {
                let delta = previous
                    .and_then(|previous| total.checked_sub(previous))
                    .or(last)
                    .unwrap_or(total);
                baselines.insert(event.session.clone(), total);
                delta
            }
            (None, Some(last)) => last,
            (None, None) => continue,
        };
        if delta.input == 0 && delta.output == 0 {
            continue;
        }
        let entry = by_model.entry(model).or_default();
        entry.0.add(delta.normalized());
        entry.1.insert(event.session);
        entry.2 = entry.2.saturating_add(1);
    }
    let mut models = Vec::new();
    let mut totals = UsageTotals::default();
    for (model, (usage, sessions, messages)) in by_model {
        totals.add(usage);
        models.push(ModelUsage {
            provider_id: "codex".into(),
            native_model_id: model,
            sessions: sessions.len().try_into().unwrap_or(u32::MAX),
            messages,
            totals: usage,
        });
    }
    models.sort_by(|left, right| left.native_model_id.cmp(&right.native_model_id));
    let mut diagnostics: Vec<String> = walk.diagnostics.into_iter().collect();
    diagnostics.sort();
    ProviderUsage {
        provider_id: "codex".into(),
        roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        files_read,
        files_skipped,
        malformed_lines,
        truncated: walk.truncated,
        diagnostics,
        models,
        totals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, lines: &[&str]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    #[test]
    fn claude_streaming_duplicates_merge_but_do_not_double_count() {
        let root = tempfile::tempdir().unwrap();
        let projects = root.path().join(".claude/projects/p");
        write(
            &projects.join("one.jsonl"),
            &[
                r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":2,"cache_read_input_tokens":30,"cache_creation_input_tokens":5}}}"#,
                r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":7,"cache_read_input_tokens":30,"cache_creation_input_tokens":5}}}"#,
                r#"{"type":"assistant","sessionId":"s1","requestId":"r2","message":{"id":"m2","model":"claude-x","usage":{"input_tokens":4,"output_tokens":3}}}"#,
            ],
        );
        let usage = read_usage(root.path(), Limits::default());
        let claude = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "claude_code")
            .unwrap();
        assert_eq!(
            claude.totals,
            UsageTotals {
                input_tokens: 14,
                cached_input_tokens: 30,
                cache_creation_tokens: 5,
                output_tokens: 10,
                reasoning_tokens: 0,
            }
        );
        assert_eq!(claude.models[0].messages, 2);
    }

    #[test]
    fn claude_split_snapshot_supersedes_a_whole_prompt_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let projects = root.path().join(".claude/projects/p");
        write(
            &projects.join("one.jsonl"),
            &[
                r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":100,"output_tokens":2}}}"#,
                r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":10,"output_tokens":2,"cache_read_input_tokens":90}}}"#,
            ],
        );
        let usage = read_usage(root.path(), Limits::default());
        let totals = usage.providers[0].totals;
        assert_eq!(totals.input_tokens, 10);
        assert_eq!(totals.cached_input_tokens, 90);
        assert_eq!(totals.total_input_tokens(), 100);
    }

    #[test]
    fn codex_resumed_files_and_mixed_models_are_counted_once() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join(".codex/sessions");
        write(
            &sessions.join("one.jsonl"),
            &[
                r#"{"type":"session_meta","payload":{"id":"thread-1"}}"#,
                r#"{"type":"turn_context","payload":{"model":"gpt-a"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"inputTokens":100,"cachedInputTokens":40,"cacheWriteInputTokens":5,"outputTokens":20,"reasoningOutputTokens":4},"last_token_usage":{"inputTokens":100,"cachedInputTokens":40,"cacheWriteInputTokens":5,"outputTokens":20,"reasoningOutputTokens":4}}}}"#,
                r#"{"type":"turn_context","payload":{"model":"gpt-b"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"inputTokens":150,"cachedInputTokens":60,"cacheWriteInputTokens":7,"outputTokens":35,"reasoningOutputTokens":6},"last_token_usage":{"inputTokens":50,"cachedInputTokens":20,"cacheWriteInputTokens":2,"outputTokens":15,"reasoningOutputTokens":2}}}}"#,
            ],
        );
        // A resumed copy repeats the first record; it must not add usage.
        write(
            &sessions.join("two.jsonl"),
            &[
                r#"{"type":"session_meta","payload":{"id":"thread-1"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"inputTokens":100,"cachedInputTokens":40,"cacheWriteInputTokens":5,"outputTokens":20,"reasoningOutputTokens":4},"last_token_usage":{"inputTokens":100,"cachedInputTokens":40,"cacheWriteInputTokens":5,"outputTokens":20,"reasoningOutputTokens":4}}}}"#,
            ],
        );
        let usage = read_usage(root.path(), Limits::default());
        let codex = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "codex")
            .unwrap();
        assert_eq!(
            codex.totals,
            UsageTotals {
                input_tokens: 150 - 60 - 7,
                cached_input_tokens: 60,
                cache_creation_tokens: 7,
                output_tokens: 35,
                reasoning_tokens: 6,
            }
        );
        assert_eq!(codex.models.len(), 2);
        assert_eq!(
            codex
                .models
                .iter()
                .map(|model| model.native_model_id.as_str())
                .collect::<Vec<_>>(),
            vec!["gpt-a", "gpt-b"]
        );
    }

    #[test]
    fn malformed_lines_and_oversize_files_are_reported_not_silent() {
        let root = tempfile::tempdir().unwrap();
        let projects = root.path().join(".claude/projects/p");
        write(
            &projects.join("one.jsonl"),
            &[
                "{not json",
                r#"{"type":"assistant","message":{"model":"m","usage":{"input_tokens":1}}}"#,
            ],
        );
        let limits = Limits {
            max_file_bytes: 1,
            ..Limits::default()
        };
        let usage = read_usage(root.path(), limits);
        let claude = &usage.providers[0];
        assert!(claude.files_skipped > 0 || claude.malformed_lines > 0);
    }
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    use std::{collections::BTreeMap, io::Write};

    fn write(path: &Path, lines: &[&str]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    #[test]
    fn custom_homes_are_honored_and_the_secure_storage_override_is_not_a_session_root() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let claude = root.path().join("custom-claude");
        let codex = root.path().join("custom-codex");
        let secure = root.path().join("secure-only");
        write(
            &claude.join("projects/p/one.jsonl"),
            &[
                r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":3,"output_tokens":4}}}"#,
            ],
        );
        write(
            &codex.join("sessions/s/one.jsonl"),
            &[
                r#"{"type":"session_meta","payload":{"id":"x1"}}"#,
                r#"{"type":"turn_context","payload":{"model":"gpt-x"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"inputTokens":9,"cachedInputTokens":2,"outputTokens":5},"last_token_usage":{"inputTokens":9,"cachedInputTokens":2,"outputTokens":5}}}}"#,
            ],
        );
        std::fs::create_dir_all(&secure).unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("HOME".into(), home.to_string_lossy().into_owned());
        vars.insert(
            "CLAUDE_CONFIG_DIR".into(),
            claude.to_string_lossy().into_owned(),
        );
        vars.insert(
            "CLAUDE_SECURESTORAGE_CONFIG_DIR".into(),
            secure.to_string_lossy().into_owned(),
        );
        vars.insert("CODEX_HOME".into(), codex.to_string_lossy().into_owned());
        let environment = originlm_discovery::ScanEnvironment::from_map(
            originlm_discovery::HostPlatform::Linux,
            &vars,
        )
        .unwrap();
        let usage = read_usage_for_environment(&environment, Limits::default());
        let claude_usage = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "claude_code")
            .unwrap();
        assert_eq!(claude_usage.totals.input_tokens, 3);
        let codex_usage = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "codex")
            .unwrap();
        assert_eq!(codex_usage.totals.input_tokens, 7);
        assert_eq!(codex_usage.totals.cached_input_tokens, 2);
    }
}
