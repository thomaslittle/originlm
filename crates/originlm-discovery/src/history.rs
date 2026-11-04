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

use originlm_contracts::{ModelUsage, ProviderUsage, UsageHistory, UsageTotals};
use serde::Deserialize;
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
        config_roots(&home.join(".gemini"), ["tmp"]),
        config_roots(&home.join(".grok"), ["sessions"]),
        config_roots(&home.join(".kiro"), ["sessions/cli"]),
        limits,
    )
}

/// Read the histories selected by a discovery environment, including
/// `CODEX_HOME` and `CLAUDE_CONFIG_DIR`. `CLAUDE_SECURESTORAGE_CONFIG_DIR` is
/// deliberately not used here: it moves the credential store, not `projects/`.
pub fn read_usage_for_environment(
    environment: &crate::ScanEnvironment,
    limits: Limits,
) -> UsageHistory {
    let claude_home = environment
        .overrides
        .get("CLAUDE_CONFIG_DIR")
        .cloned()
        .unwrap_or_else(|| environment.home.join(".claude"));
    let codex_home = crate::profile_roots("codex", environment)
        .into_iter()
        .next()
        .unwrap_or_else(|| environment.home.join(".codex"));
    let gemini_home = crate::profile_roots("gemini_cli", environment)
        .into_iter()
        .next()
        .unwrap_or_else(|| environment.home.join(".gemini"));
    let grok_home = crate::profile_roots("grok", environment)
        .into_iter()
        .next()
        .unwrap_or_else(|| environment.home.join(".grok"));
    let kiro_home = environment.home.join(".kiro");
    read_roots(
        config_roots(&claude_home, ["projects", "transcripts"]),
        config_roots(&codex_home, ["sessions", "archived_sessions"]),
        config_roots(&gemini_home, ["tmp"]),
        config_roots(&grok_home, ["sessions"]),
        config_roots(&kiro_home, ["sessions/cli"]),
        limits,
    )
}

fn config_roots<const N: usize>(root: &Path, relatives: [&str; N]) -> Vec<PathBuf> {
    existing_roots(relatives.map(|relative| root.join(relative)))
}

fn read_roots(
    claude_roots: Vec<PathBuf>,
    codex_roots: Vec<PathBuf>,
    gemini_roots: Vec<PathBuf>,
    grok_roots: Vec<PathBuf>,
    kiro_roots: Vec<PathBuf>,
    limits: Limits,
) -> UsageHistory {
    let mut providers = Vec::new();
    if !claude_roots.is_empty() {
        providers.push(read_claude(&claude_roots, limits));
    }
    if !codex_roots.is_empty() {
        providers.push(read_codex(&codex_roots, limits));
    }
    if !gemini_roots.is_empty() {
        providers.push(read_gemini(&gemini_roots, limits));
    }
    if !grok_roots.is_empty() {
        providers.push(read_grok(&grok_roots, limits));
    }
    if !kiro_roots.is_empty() {
        providers.push(read_kiro(&kiro_roots, limits));
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
    walk_matching(root, limits, walk, &|name| name.ends_with(".jsonl"));
}

fn walk_json(root: &Path, limits: Limits, walk: &mut Walk) {
    walk_matching(root, limits, walk, &|name| name.ends_with(".json"));
}

fn walk_matching(root: &Path, limits: Limits, walk: &mut Walk, matches: &dyn Fn(&str) -> bool) {
    fn recurse(
        path: &Path,
        depth: usize,
        limits: Limits,
        walk: &mut Walk,
        matches: &dyn Fn(&str) -> bool,
    ) {
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
                recurse(&path, depth + 1, limits, walk, matches);
            } else if file_type.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if matches(&name) {
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
    recurse(root, 0, limits, walk, matches);
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

/// Only the fields usage accounting needs. `content`, tool results and
/// attachments are deliberately absent, so serde skips them instead of
/// materializing prompt text.
#[derive(Debug, Deserialize)]
struct ClaudeLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<ClaudeMessage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Debug, Default, Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    output_tokens_details: Option<ClaudeOutputDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct ClaudeOutputDetails {
    thinking_tokens: Option<u64>,
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
            let Ok(entry) = serde_json::from_str::<ClaudeLine>(line) else {
                return false;
            };
            if entry.kind.as_deref() != Some("assistant") {
                return true;
            }
            let Some(message) = entry.message else {
                return true;
            };
            let Some(usage) = message.usage else {
                return true;
            };
            let model = message
                .model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty() && model.len() <= 256)
                .unwrap_or("unknown")
                .to_owned();
            // Claude Code writes local error placeholders as assistant turns;
            // they are not billed requests and must not inflate a model row.
            if model == "<synthetic>" {
                return true;
            }
            let session = entry
                .session_id
                .as_deref()
                .map(str::trim)
                .filter(|session| !session.is_empty() && session.len() <= 256)
                .map(str::to_owned)
                .or_else(|| {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "unknown".into());
            let request = entry.request_id.as_deref().unwrap_or("");
            let message_id = message.id.as_deref().unwrap_or("");
            let key = if request.is_empty() && message_id.is_empty() {
                format!("{}:{}", path.display(), records.len())
            } else {
                format!("{request}:{message_id}")
            };
            let reports_split = usage.cache_read_input_tokens.is_some()
                || usage.cache_creation_input_tokens.is_some();
            let total = UsageTotals {
                input_tokens: usage.input_tokens.unwrap_or(0),
                cached_input_tokens: usage.cache_read_input_tokens.unwrap_or(0),
                cache_creation_tokens: usage.cache_creation_input_tokens.unwrap_or(0),
                output_tokens: usage.output_tokens.unwrap_or(0),
                reasoning_tokens: usage
                    .output_tokens_details
                    .and_then(|details| details.thinking_tokens)
                    .unwrap_or(0),
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
    let mut by_model: BTreeMap<String, AggregatedModel> = BTreeMap::new();
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
    if files_read == 0 {
        walk.diagnostics.insert("NO_SESSION_FILES_READ".into());
    }
    if models.is_empty() {
        walk.diagnostics.insert("NO_USAGE_RECORDS".into());
    }
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
    #[cfg(test)]
    fn from_json(value: &serde_json::Value) -> Option<Self> {
        // Codex app-server events use camelCase; rollout JSONL files use
        // snake_case. Both are accepted, and the rollout spelling wins when a
        // file carries both.
        let count =
            |camel: &str, snake: &str| value[snake].as_u64().or_else(|| value[camel].as_u64());
        let input = count("inputTokens", "input_tokens")?;
        let output = count("outputTokens", "output_tokens")?;
        Some(Self {
            input,
            cached: count("cachedInputTokens", "cached_input_tokens")
                .or_else(|| value["cache_read_input_tokens"].as_u64())
                .unwrap_or(0),
            cache_write: count("cacheWriteInputTokens", "cache_write_input_tokens")
                .or_else(|| value["cache_write_tokens"].as_u64()),
            output,
            reasoning: count("reasoningOutputTokens", "reasoning_output_tokens").unwrap_or(0),
        })
    }

    fn from_usage(value: &CodexUsage) -> Option<Self> {
        Some(Self {
            input: value.input_tokens?,
            cached: value.cached_input_tokens.unwrap_or(0),
            cache_write: value.cache_write_input_tokens,
            output: value.output_tokens?,
            reasoning: value.reasoning_output_tokens.unwrap_or(0),
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

#[derive(Debug, Deserialize)]
struct CodexLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    payload: Option<CodexPayload>,
    info: Option<CodexInfo>,
}

#[derive(Debug, Deserialize)]
struct CodexPayload {
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    model: Option<String>,
    info: Option<CodexInfo>,
}

#[derive(Debug, Default, Deserialize)]
struct CodexInfo {
    last_token_usage: Option<CodexUsage>,
    total_token_usage: Option<CodexUsage>,
}

#[derive(Debug, Default, Deserialize)]
struct CodexUsage {
    #[serde(default, alias = "inputTokens")]
    input_tokens: Option<u64>,
    #[serde(
        default,
        alias = "cachedInputTokens",
        alias = "cache_read_input_tokens"
    )]
    cached_input_tokens: Option<u64>,
    #[serde(default, alias = "cacheWriteInputTokens", alias = "cache_write_tokens")]
    cache_write_input_tokens: Option<u64>,
    #[serde(default, alias = "outputTokens")]
    output_tokens: Option<u64>,
    #[serde(default, alias = "reasoningOutputTokens")]
    reasoning_output_tokens: Option<u64>,
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
            let Ok(entry) = serde_json::from_str::<CodexLine>(line) else {
                return false;
            };
            let kind = entry.kind.as_deref();
            let payload = entry.payload.as_ref();
            if kind == Some("session_meta") {
                if let Some(id) = payload
                    .and_then(|payload| payload.id.as_deref())
                    .map(str::trim)
                    .filter(|id| !id.is_empty() && id.len() <= 256)
                {
                    session = id.to_owned();
                }
                return true;
            }
            if kind == Some("turn_context") {
                if let Some(name) = payload
                    .and_then(|payload| payload.model.as_deref())
                    .map(str::trim)
                    .filter(|name| !name.is_empty() && name.len() <= 256)
                {
                    model = name.to_owned();
                }
                return true;
            }
            let info = if kind == Some("event_msg")
                && payload.and_then(|payload| payload.kind.as_deref()) == Some("token_count")
            {
                payload.and_then(|payload| payload.info.as_ref())
            } else if kind == Some("token_count") {
                entry.info.as_ref()
            } else {
                None
            };
            let Some(info) = info else {
                return true;
            };
            let total = info
                .total_token_usage
                .as_ref()
                .and_then(Breakdown::from_usage);
            let last = info
                .last_token_usage
                .as_ref()
                .and_then(Breakdown::from_usage);
            if total.is_none() && last.is_none() {
                return true;
            }
            let timestamp = entry
                .timestamp
                .as_deref()
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
    let mut by_model: BTreeMap<String, AggregatedModel> = BTreeMap::new();
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
    if files_read == 0 {
        walk.diagnostics.insert("NO_SESSION_FILES_READ".into());
    }
    if models.is_empty() {
        walk.diagnostics.insert("NO_USAGE_RECORDS".into());
    }
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

type AggregatedModel = (UsageTotals, BTreeSet<String>, u32);
type GrokRecordKey = (String, String, u64, u64);

fn add_usage(
    by_model: &mut BTreeMap<String, AggregatedModel>,
    session: &str,
    model: &str,
    totals: UsageTotals,
) {
    if totals.is_zero() {
        return;
    }
    let entry = by_model.entry(model.to_owned()).or_default();
    entry.0.add(totals);
    entry.1.insert(session.to_owned());
    entry.2 = entry.2.saturating_add(1);
}

fn model_rows(
    provider_id: &str,
    roots: &[PathBuf],
    walk: &Walk,
    by_model: BTreeMap<String, AggregatedModel>,
    malformed_lines: u32,
    files_read: u32,
    files_skipped: u32,
) -> ProviderUsage {
    let mut models = Vec::new();
    let mut totals = UsageTotals::default();
    for (model, (usage, sessions, messages)) in by_model {
        totals.add(usage);
        models.push(ModelUsage {
            provider_id: provider_id.into(),
            native_model_id: model,
            sessions: sessions.len().try_into().unwrap_or(u32::MAX),
            messages,
            totals: usage,
        });
    }
    models.sort_by(|left, right| left.native_model_id.cmp(&right.native_model_id));
    let mut diagnostics: Vec<String> = walk.diagnostics.iter().cloned().collect();
    if files_read == 0 {
        diagnostics.push("NO_SESSION_FILES_READ".into());
    }
    if models.is_empty() {
        diagnostics.push("NO_USAGE_RECORDS".into());
    }
    diagnostics.sort();
    diagnostics.dedup();
    ProviderUsage {
        provider_id: provider_id.into(),
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

fn positive(value: Option<i64>) -> u64 {
    value.unwrap_or(0).max(0).try_into().unwrap_or(u64::MAX)
}

fn positive_value(value: Option<i64>) -> Option<u64> {
    value.map(|value| value.max(0).try_into().unwrap_or(u64::MAX))
}

#[derive(Debug, Default, Deserialize)]
struct GeminiTokens {
    #[serde(
        default,
        alias = "prompt",
        alias = "input_tokens",
        alias = "prompt_tokens",
        alias = "promptTokenCount"
    )]
    input: Option<i64>,
    #[serde(
        default,
        alias = "candidates",
        alias = "output_tokens",
        alias = "completion_tokens",
        alias = "candidatesTokenCount"
    )]
    output: Option<i64>,
    #[serde(default, alias = "cached_tokens", alias = "cachedContentTokenCount")]
    cached: Option<i64>,
    #[serde(default, alias = "reasoning", alias = "thoughts_tokens")]
    thoughts: Option<i64>,
    #[serde(default, alias = "tool_tokens")]
    tool: Option<i64>,
    #[serde(default, alias = "totalTokenCount", alias = "total_tokens")]
    total: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GeminiMessage {
    id: Option<String>,
    model: Option<String>,
    tokens: Option<GeminiTokens>,
}

#[derive(Debug, Deserialize)]
struct GeminiSession {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "session_id")]
    session_id_snake: Option<String>,
    messages: Option<Vec<GeminiMessage>>,
}

#[derive(Debug, Deserialize)]
struct GeminiLine {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "session_id")]
    session_id_snake: Option<String>,
    model: Option<String>,
    id: Option<String>,
    tokens: Option<GeminiTokens>,
}

fn model_name(name: &Option<String>) -> Option<String> {
    name.as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.len() <= 256)
        .map(str::to_owned)
}

fn gemini_totals(tokens: &GeminiTokens) -> UsageTotals {
    let input = positive(tokens.input);
    let output = positive(tokens.output);
    let cached = positive(tokens.cached);
    let thoughts = positive(tokens.thoughts);
    let tool = positive(tokens.tool);
    let total = positive_value(tokens.total);
    // Gemini sometimes reports a total where cache-read tokens are included in
    // the prompt count and sometimes disjoint. Only subtract when the total
    // proves the inclusive shape; otherwise preserve the provider value.
    let inclusive = total
        .is_some_and(|total| total == input.saturating_add(output).saturating_add(thoughts) + tool);
    let fresh = if cached > 0 && inclusive {
        input.saturating_sub(cached)
    } else {
        input
    };
    UsageTotals {
        input_tokens: fresh.saturating_add(tool),
        cached_input_tokens: cached,
        cache_creation_tokens: 0,
        // Gemini thought tokens are generated output; keep them whole and flag
        // the subset so output == total generation.
        output_tokens: output.saturating_add(thoughts),
        reasoning_tokens: thoughts,
    }
}

fn read_gemini(roots: &[PathBuf], limits: Limits) -> ProviderUsage {
    let mut walk = Walk::default();
    for root in roots {
        walk_json(root, limits, &mut walk);
        walk_jsonl(root, limits, &mut walk);
    }
    let mut records: BTreeMap<String, (String, String, UsageTotals)> = BTreeMap::new();
    let mut malformed_lines = 0;
    let mut files_read = 0;
    let mut files_skipped = walk.skipped;
    let mut budget = limits.max_total_bytes;
    for path in &walk.files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let in_chats = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|parent| parent.to_str())
            == Some("chats");
        if !name.starts_with("session-") && !in_chats {
            continue;
        }
        let is_jsonl = path.extension().and_then(|extension| extension.to_str()) == Some("jsonl");
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        if is_jsonl {
            let mut session = stem.clone();
            let mut model = "unknown".to_owned();
            let (read, rejected) = read_lines(path, limits, &mut budget, |line| {
                let Ok(value) = serde_json::from_str::<GeminiLine>(line) else {
                    return false;
                };
                if let Some(id) = value
                    .session_id
                    .as_deref()
                    .or(value.session_id_snake.as_deref())
                    .filter(|id| !id.is_empty() && id.len() <= 256)
                {
                    session = id.to_owned();
                }
                if let Some(name) = model_name(&value.model) {
                    model = name;
                }
                let Some(tokens) = value.tokens.as_ref() else {
                    return true;
                };
                let key = value
                    .id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .map(|id| format!("{session}:{id}"))
                    .unwrap_or_else(|| format!("{}:{}", path.display(), records.len()));
                records.insert(key, (session.clone(), model.clone(), gemini_totals(tokens)));
                true
            });
            if read {
                files_read += 1;
                malformed_lines += rejected;
            } else {
                files_skipped += 1;
            }
            continue;
        }
        let Ok(bytes) = fs::read(path) else {
            files_skipped += 1;
            continue;
        };
        if bytes.len() as u64 > limits.max_file_bytes || bytes.len() as u64 > budget {
            files_skipped += 1;
            continue;
        }
        budget = budget.saturating_sub(bytes.len() as u64);
        let Ok(value) = serde_json::from_slice::<GeminiSession>(&bytes) else {
            malformed_lines += 1;
            continue;
        };
        let session = value
            .session_id
            .as_deref()
            .or(value.session_id_snake.as_deref())
            .filter(|id| !id.is_empty() && id.len() <= 256)
            .unwrap_or(&stem)
            .to_owned();
        let Some(messages) = value.messages.as_ref() else {
            malformed_lines += 1;
            continue;
        };
        for (index, message) in messages.iter().enumerate() {
            let Some(tokens) = message.tokens.as_ref() else {
                continue;
            };
            let model = model_name(&message.model).unwrap_or_else(|| "unknown".into());
            let key = message
                .id
                .as_deref()
                .filter(|id| !id.is_empty())
                .map(|id| format!("{session}:{id}"))
                .unwrap_or_else(|| format!("{}:{index}", path.display()));
            records.insert(key, (session.clone(), model, gemini_totals(tokens)));
        }
        files_read += 1;
    }
    let mut by_model: BTreeMap<String, AggregatedModel> = BTreeMap::new();
    for (session, model, totals) in records.into_values() {
        add_usage(&mut by_model, &session, &model, totals);
    }
    model_rows(
        "gemini_cli",
        roots,
        &walk,
        by_model,
        malformed_lines,
        files_read,
        files_skipped,
    )
}

#[derive(Debug, Default, Deserialize)]
struct GrokUsage {
    #[serde(default, alias = "inputTokens", alias = "promptTokens")]
    input_tokens: Option<i64>,
    #[serde(default, alias = "outputTokens", alias = "completionTokens")]
    output_tokens: Option<i64>,
    #[serde(
        default,
        alias = "cachedReadTokens",
        alias = "cacheReadTokens",
        alias = "cache_read_input_tokens"
    )]
    cached_read_tokens: Option<i64>,
    #[serde(
        default,
        alias = "cachedWriteTokens",
        alias = "cacheWriteTokens",
        alias = "cacheCreationTokens",
        alias = "cache_creation_input_tokens"
    )]
    cached_write_tokens: Option<i64>,
    #[serde(
        default,
        alias = "reasoningTokens",
        alias = "thoughtTokens",
        alias = "thinkingTokens"
    )]
    reasoning_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GrokUpdate {
    #[serde(rename = "sessionUpdate")]
    session_update: Option<String>,
    usage: Option<GrokUsage>,
    model: Option<String>,
    #[serde(rename = "modelID")]
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrokParams {
    update: Option<GrokUpdate>,
}

#[derive(Debug, Deserialize)]
struct GrokLine {
    timestamp: Option<String>,
    params: Option<GrokParams>,
}

fn grok_totals(usage: &GrokUsage) -> UsageTotals {
    let input = positive(usage.input_tokens);
    let output = positive(usage.output_tokens);
    let cached = positive(usage.cached_read_tokens);
    let cache_write = positive(usage.cached_write_tokens);
    let reasoning = positive(usage.reasoning_tokens);
    // Grok's input includes cache reads; output includes reasoning. Keep the
    // contract's reasoning-as-subset shape.
    UsageTotals {
        input_tokens: input.saturating_sub(cached),
        cached_input_tokens: cached,
        cache_creation_tokens: cache_write,
        output_tokens: output,
        reasoning_tokens: reasoning.min(output),
    }
}

fn read_grok(roots: &[PathBuf], limits: Limits) -> ProviderUsage {
    let mut walk = Walk::default();
    for root in roots {
        walk_jsonl(root, limits, &mut walk);
    }
    let mut records: BTreeMap<GrokRecordKey, (String, String, UsageTotals)> = BTreeMap::new();
    let mut malformed_lines = 0;
    let mut files_read = 0;
    let mut files_skipped = walk.skipped;
    let mut budget = limits.max_total_bytes;
    for path in &walk.files {
        if path.file_name().and_then(|name| name.to_str()) != Some("updates.jsonl") {
            continue;
        }
        let session = path
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        let (read, rejected) = read_lines(path, limits, &mut budget, |line| {
            let Ok(value) = serde_json::from_str::<GrokLine>(line) else {
                return false;
            };
            let Some(update) = value
                .params
                .as_ref()
                .and_then(|params| params.update.as_ref())
            else {
                return true;
            };
            if update.session_update.as_deref() != Some("turn_completed") {
                return true;
            }
            let Some(usage) = update.usage.as_ref() else {
                return true;
            };
            let totals = grok_totals(usage);
            if totals.is_zero() {
                return true;
            }
            let model = model_name(&update.model)
                .or_else(|| model_name(&update.model_id))
                .unwrap_or_else(|| "unknown".into());
            let timestamp = value.timestamp.clone().unwrap_or_default();
            let key = (
                session.clone(),
                timestamp,
                totals.input_tokens,
                totals.output_tokens,
            );
            records.insert(key, (session.clone(), model, totals));
            true
        });
        if read {
            files_read += 1;
            malformed_lines += rejected;
        } else {
            files_skipped += 1;
        }
    }
    let mut by_model: BTreeMap<String, AggregatedModel> = BTreeMap::new();
    for (session, model, totals) in records.into_values() {
        add_usage(&mut by_model, &session, &model, totals);
    }
    model_rows(
        "grok",
        roots,
        &walk,
        by_model,
        malformed_lines,
        files_read,
        files_skipped,
    )
}

#[derive(Debug, Default, Deserialize)]
struct KiroTurn {
    #[serde(default, alias = "uncached_input_tokens")]
    input_token_count: Option<i64>,
    #[serde(default, alias = "output_tokens")]
    output_token_count: Option<i64>,
    #[serde(default, alias = "cache_read_tokens")]
    cache_read_input_token_count: Option<i64>,
    #[serde(default, alias = "cache_creation_tokens")]
    cache_write_input_token_count: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct KiroConversation {
    #[serde(default)]
    user_turn_metadatas: Vec<KiroTurn>,
}

#[derive(Debug, Deserialize)]
struct KiroModelInfo {
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KiroModelState {
    model_info: Option<KiroModelInfo>,
}

#[derive(Debug, Deserialize)]
struct KiroState {
    rts_model_state: Option<KiroModelState>,
    conversation_metadata: Option<KiroConversation>,
}

#[derive(Debug, Deserialize)]
struct KiroSession {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "session_id")]
    session_id_snake: Option<String>,
    model: Option<String>,
    session_state: Option<KiroState>,
    conversation_metadata: Option<KiroConversation>,
}

fn kiro_totals(turn: &KiroTurn) -> UsageTotals {
    UsageTotals {
        input_tokens: positive(turn.input_token_count),
        cached_input_tokens: positive(turn.cache_read_input_token_count),
        cache_creation_tokens: positive(turn.cache_write_input_token_count),
        output_tokens: positive(turn.output_token_count),
        reasoning_tokens: 0,
    }
}

fn read_kiro(roots: &[PathBuf], limits: Limits) -> ProviderUsage {
    let mut walk = Walk::default();
    for root in roots {
        walk_json(root, limits, &mut walk);
    }
    let mut records: BTreeMap<String, (String, String, UsageTotals)> = BTreeMap::new();
    let mut malformed_lines = 0;
    let mut files_read = 0;
    let mut files_skipped = walk.skipped;
    let mut budget = limits.max_total_bytes;
    for path in &walk.files {
        let Ok(bytes) = fs::read(path) else {
            files_skipped += 1;
            continue;
        };
        if bytes.len() as u64 > limits.max_file_bytes || bytes.len() as u64 > budget {
            files_skipped += 1;
            continue;
        }
        budget = budget.saturating_sub(bytes.len() as u64);
        let Ok(value) = serde_json::from_slice::<KiroSession>(&bytes) else {
            malformed_lines += 1;
            continue;
        };
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        let session = value
            .session_id
            .as_deref()
            .or(value.session_id_snake.as_deref())
            .filter(|id| !id.is_empty() && id.len() <= 256)
            .unwrap_or(&stem)
            .to_owned();
        let state = value.session_state.as_ref();
        let model = state
            .and_then(|state| state.rts_model_state.as_ref())
            .and_then(|model| model.model_info.as_ref())
            .and_then(|info| model_name(&info.model_id))
            .or_else(|| model_name(&value.model))
            .unwrap_or_else(|| "unknown".into());
        let turns = state
            .and_then(|state| state.conversation_metadata.as_ref())
            .or(value.conversation_metadata.as_ref())
            .map(|conversation| conversation.user_turn_metadatas.as_slice())
            .unwrap_or_default();
        for (index, turn) in turns.iter().enumerate() {
            let totals = kiro_totals(turn);
            if totals.is_zero() {
                continue;
            }
            records.insert(
                format!("{session}:{index}"),
                (session.clone(), model.clone(), totals),
            );
        }
        files_read += 1;
    }
    let mut by_model: BTreeMap<String, AggregatedModel> = BTreeMap::new();
    for (session, model, totals) in records.into_values() {
        add_usage(&mut by_model, &session, &model, totals);
    }
    model_rows(
        "kiro",
        roots,
        &walk,
        by_model,
        malformed_lines,
        files_read,
        files_skipped,
    )
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
    fn codex_rollout_and_app_server_token_shapes_normalize_the_same() {
        let rollout = Breakdown::from_json(&serde_json::json!({
            "input_tokens": 100,
            "cached_input_tokens": 40,
            "cache_write_input_tokens": 5,
            "output_tokens": 20,
            "reasoning_output_tokens": 4
        }))
        .unwrap();
        let app_server = Breakdown::from_json(&serde_json::json!({
            "inputTokens": 100,
            "cachedInputTokens": 40,
            "cacheWriteInputTokens": 5,
            "outputTokens": 20,
            "reasoningOutputTokens": 4
        }))
        .unwrap();
        assert_eq!(rollout, app_server);
        assert_eq!(
            rollout.normalized(),
            UsageTotals {
                input_tokens: 55,
                cached_input_tokens: 40,
                cache_creation_tokens: 5,
                output_tokens: 20,
                reasoning_tokens: 4,
            }
        );
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
    fn synthetic_claude_turns_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        let projects = root.path().join(".claude/projects/p");
        write(
            &projects.join("one.jsonl"),
            &[
                r#"{"type":"assistant","sessionId":"s1","requestId":"r1","message":{"id":"m1","model":"<synthetic>","usage":{"input_tokens":9,"output_tokens":9}}}"#,
            ],
        );
        let usage = read_usage(root.path(), Limits::default());
        assert!(usage.providers.is_empty() || usage.providers[0].totals.is_zero());
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
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":40,"cache_write_input_tokens":5,"output_tokens":20,"reasoning_output_tokens":4},"last_token_usage":{"input_tokens":100,"cached_input_tokens":40,"cache_write_input_tokens":5,"output_tokens":20,"reasoning_output_tokens":4}}}}"#,
                r#"{"type":"turn_context","payload":{"model":"gpt-b"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":150,"cached_input_tokens":60,"cache_write_input_tokens":7,"output_tokens":35,"reasoning_output_tokens":6},"last_token_usage":{"input_tokens":50,"cached_input_tokens":20,"cache_write_input_tokens":2,"output_tokens":15,"reasoning_output_tokens":2}}}}"#,
            ],
        );
        // A resumed copy repeats the first record; it must not add usage.
        write(
            &sessions.join("two.jsonl"),
            &[
                r#"{"type":"session_meta","payload":{"id":"thread-1"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":40,"cache_write_input_tokens":5,"output_tokens":20,"reasoning_output_tokens":4},"last_token_usage":{"input_tokens":100,"cached_input_tokens":40,"cache_write_input_tokens":5,"output_tokens":20,"reasoning_output_tokens":4}}}}"#,
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
    fn gemini_session_json_and_headless_jsonl_are_counted_once() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        write(
            &home.join(".gemini/tmp/p/chats/session-1.json"),
            &[
                r#"{"sessionId":"g1","messages":[{"id":"m1","model":"gemini-x","tokens":{"input":10,"output":4,"cached":5,"thoughts":2,"total":21}},{"id":"m2","model":"gemini-x","tokens":{"input":3,"output":1}}]}"#,
            ],
        );
        write(
            &home.join(".gemini/tmp/p/session-2.jsonl"),
            &[
                r#"{"type":"init","session_id":"g2","model":"gemini-y"}"#,
                r#"{"type":"gemini","id":"m3","tokens":{"input_tokens":7,"output_tokens":2,"cachedContentTokenCount":3}}"#,
            ],
        );
        let usage = read_usage(home, Limits::default());
        let gemini = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "gemini_cli")
            .unwrap();
        assert_eq!(gemini.totals.input_tokens, 10 + 3 + 7);
        assert_eq!(gemini.totals.cached_input_tokens, 5 + 3);
        assert_eq!(gemini.totals.output_tokens, 6 + 1 + 2);
        assert_eq!(gemini.totals.reasoning_tokens, 2);
    }

    #[test]
    fn grok_turn_completed_updates_keep_cache_and_reasoning_separate() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        write(
            &home.join(".grok/sessions/ws/session-a/updates.jsonl"),
            &[
                r#"{"timestamp":"2026-11-01T00:00:00Z","params":{"update":{"sessionUpdate":"turn_completed","model":"grok-x","usage":{"inputTokens":100,"outputTokens":20,"cachedReadTokens":40,"cachedWriteTokens":5,"reasoningTokens":4}}}}"#,
                r#"{"timestamp":"2026-11-01T00:00:01Z","params":{"update":{"sessionUpdate":"turn_completed","model":"grok-y","usage":{"inputTokens":50,"outputTokens":10,"cachedReadTokens":20,"reasoningTokens":2}}}}"#,
            ],
        );
        let usage = read_usage(home, Limits::default());
        let grok = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "grok")
            .unwrap();
        assert_eq!(grok.totals.input_tokens, 60 + 30);
        assert_eq!(grok.totals.cached_input_tokens, 60);
        assert_eq!(grok.totals.cache_creation_tokens, 5);
        assert_eq!(grok.totals.output_tokens, 30);
        assert_eq!(grok.totals.reasoning_tokens, 6);
    }

    #[test]
    fn kiro_turn_metadata_uses_only_recorded_token_counts() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        write(
            &home.join(".kiro/sessions/cli/session-1.json"),
            &[
                r#"{"session_id":"k1","session_state":{"rts_model_state":{"model_info":{"model_id":"claude-sonnet-4.5"}},"conversation_metadata":{"user_turn_metadatas":[{"input_token_count":10,"output_token_count":4,"cache_read_input_token_count":2,"cache_write_input_token_count":1},{"input_token_count":0,"output_token_count":0,"user_prompt_length":100}]}}}"#,
            ],
        );
        let usage = read_usage(home, Limits::default());
        let kiro = usage
            .providers
            .iter()
            .find(|provider| provider.provider_id == "kiro")
            .unwrap();
        assert_eq!(kiro.totals.input_tokens, 10);
        assert_eq!(kiro.totals.cached_input_tokens, 2);
        assert_eq!(kiro.totals.cache_creation_tokens, 1);
        assert_eq!(kiro.totals.output_tokens, 4);
        assert_eq!(kiro.models[0].messages, 1);
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
                r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":9,"cached_input_tokens":2,"output_tokens":5},"last_token_usage":{"input_tokens":9,"cached_input_tokens":2,"output_tokens":5}}}}"#,
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
        let environment =
            crate::ScanEnvironment::from_map(crate::HostPlatform::Linux, &vars).unwrap();
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
