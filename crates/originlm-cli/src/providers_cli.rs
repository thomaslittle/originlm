//! `originlm providers check|update`: provider CLI update status and explicit
//! updates. Local operator commands only; no broker, SDK or MCP method reaches
//! them, and nothing here runs during discovery or refresh.
use crate::provider_updates::{
    self, Checked,
    host::{Host, SystemHost},
    resolve::Install,
};
use originlm_contracts::{
    DomainError, ErrorCode, ProviderRediscovery, ProviderUpdateCheck, ProviderUpdateOutcome,
    ProviderUpdateResult, Result,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const HELP: &str = "  originlm providers check [--json] [--provider PROVIDER]... [--home ABSOLUTE_DIRECTORY]
      Probes each discovered provider CLI with --version, asks the installer that owns it
      for the latest version (npm registry, brew info, mise latest, choco search, or the
      CLI's own release channel) and reports current / behind_latest / manual_only / unknown.
      Contacts those registries; never updates anything. Prints private executable paths.
  originlm providers update (--provider PROVIDER | --all) [--executable ABSOLUTE_PATH] [--dry-run] [--json]
      Runs only an installer proven to own the binary, one at a time per install, after
      re-reading ownership; success means the re-probed version is current. --dry-run prints
      the exact command and ownership proof and changes nothing. Manual-only installs are
      never run. Running sessions keep the old CLI until they end.";

fn invalid() -> DomainError {
    DomainError::new(ErrorCode::InvalidRequest, "Invalid providers arguments")
}

#[derive(Debug, Default)]
struct Options {
    update: bool,
    json: bool,
    all: bool,
    dry_run: bool,
    providers: BTreeSet<String>,
    executable: Option<PathBuf>,
    home: Option<PathBuf>,
}

fn absolute(value: Option<OsString>) -> Result<PathBuf> {
    let path = PathBuf::from(value.ok_or_else(invalid)?);
    if !path.is_absolute() || path.to_string_lossy().chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(path)
}

/// A provider id or one of its command names (`claude_code` or `claude`).
fn provider_id(value: &str) -> Option<&'static str> {
    originlm_discovery::definitions()
        .iter()
        .find(|definition| definition.id == value || definition.commands.contains(&value))
        .map(|definition| definition.id)
}

fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Options> {
    let mut input = arguments.into_iter();
    let mut options = Options {
        update: match input.next().as_ref().and_then(|value| value.to_str()) {
            Some("check") => false,
            Some("update") => true,
            _ => return Err(invalid()),
        },
        ..Options::default()
    };
    let mut seen = BTreeSet::new();
    while let Some(flag) = input.next() {
        let flag = flag.into_string().map_err(|_| invalid())?;
        if flag != "--provider" && !seen.insert(flag.clone()) {
            return Err(invalid());
        }
        match flag.as_str() {
            "--json" => options.json = true,
            "--home" => options.home = Some(absolute(input.next())?),
            "--provider" => {
                let value = input
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .ok_or_else(invalid)?;
                options
                    .providers
                    .insert(provider_id(&value).ok_or_else(invalid)?.to_owned());
            }
            "--all" if options.update => options.all = true,
            "--dry-run" if options.update => options.dry_run = true,
            "--executable" if options.update => options.executable = Some(absolute(input.next())?),
            _ => return Err(invalid()),
        }
    }
    // An update names its target explicitly: one or more providers, or --all.
    if options.update && (options.all == !options.providers.is_empty()) {
        return Err(invalid());
    }
    Ok(options)
}

/// Discovered provider CLI binaries, one per provider and executable.
fn discover(home: &Option<PathBuf>) -> Result<Vec<Install>> {
    let result = crate::inventory::scan(
        home,
        BTreeSet::new(),
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    let mut seen = BTreeSet::new();
    let mut installs = vec![];
    for located in result.locations.values() {
        let Some(executable) = &located.executable else {
            continue;
        };
        let key = (
            located.provider_id.clone(),
            provider_updates::layout::normalize(&executable.to_string_lossy()),
        );
        if !seen.insert(key) {
            continue;
        }
        let name = originlm_discovery::definitions()
            .iter()
            .find(|definition| definition.id == located.provider_id)
            .map(|definition| definition.name)
            .unwrap_or(located.provider_id.as_str())
            .to_owned();
        installs.push(Install {
            provider_id: located.provider_id.clone(),
            name,
            executable: executable.clone(),
        });
    }
    installs.sort_by(|a, b| {
        (a.provider_id.as_str(), a.executable.as_path())
            .cmp(&(b.provider_id.as_str(), b.executable.as_path()))
    });
    Ok(installs)
}

/// After an update, find the provider's connections again and drop their
/// cached metadata (model lists), so the next probe asks the new CLI and
/// version-gated models appear only once that CLI is installed.
fn rediscover(home: &Option<PathBuf>, provider_id: &str) -> ProviderRediscovery {
    let connections: Vec<String> = crate::inventory::scan(
        home,
        BTreeSet::new(),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .map(|result| {
        result
            .locations
            .iter()
            .filter(|(_, located)| located.provider_id == provider_id)
            .map(|(connection, _)| connection.clone())
            .collect()
    })
    .unwrap_or_default();
    let database = originlm_platform::default_state_dir(home.as_deref())
        .map(|directory| directory.join("state.db"))
        .filter(|database| database.is_file());
    let (invalidated, note) = match database {
        None => (
            0,
            Some("No metadata cache exists; the next metadata probe reads the new CLI.".into()),
        ),
        Some(database) => match invalidate_metadata(&database, &connections) {
            Some(invalidated) => (
                invalidated,
                Some(
                    "A running OriginLM broker keeps its in-memory model list until its next metadata refresh."
                        .into(),
                ),
            ),
            None => (
                0,
                Some(
                    "The metadata cache could not be opened; refresh metadata to see new models."
                        .into(),
                ),
            ),
        },
    };
    ProviderRediscovery {
        connections,
        invalidated_metadata: invalidated,
        note,
    }
}

/// Drop cached metadata for these connections; the count is of rows that existed.
fn invalidate_metadata(database: &Path, connections: &[String]) -> Option<u32> {
    let (store, _) = originlm_store::StateStore::open(database).ok()?;
    let cached = store.load().ok()?.metadata;
    let mut invalidated = 0;
    for connection in connections {
        if cached
            .iter()
            .any(|entry| &entry.report.connection_id == connection)
            && store.remove_metadata(connection).is_ok()
        {
            invalidated += 1;
        }
    }
    Some(invalidated)
}

fn text_check(checked: &[Checked]) {
    if checked.is_empty() {
        println!("No provider CLI binaries were discovered.");
    }
    for item in checked {
        let report = &item.report;
        let versions = format!(
            "{} -> {}",
            report.installed_version.as_deref().unwrap_or("?"),
            report.latest_version.as_deref().unwrap_or("?")
        );
        let status = serde_json::to_value(report.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        println!(
            "{:<15} {:<22} {:<14} {}",
            report.name, versions, status, report.executable
        );
        if let Some(command) = &report.update_command {
            println!("    update: {command}");
        }
        if let Some(command) = &report.manual_command {
            println!("    manual: {command}");
        }
        if let Some(reason) = &report.reason {
            println!("    reason: {reason}");
        }
    }
}

fn text_outcomes(outcomes: &[ProviderUpdateOutcome]) {
    if outcomes.is_empty() {
        println!("No matching provider installs were discovered.");
    }
    for outcome in outcomes {
        let result = serde_json::to_value(outcome.result)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        println!("{} {} ({result})", outcome.provider_id, outcome.executable);
        if let Some(command) = &outcome.command {
            println!("    command: {command}");
        }
        if let Some(proof) = &outcome.ownership_proof {
            println!("    owner:   {proof}");
        }
        println!(
            "    version: {} -> {} (latest {})",
            outcome.previous_version.as_deref().unwrap_or("?"),
            outcome.installed_version.as_deref().unwrap_or("?"),
            outcome.latest_version.as_deref().unwrap_or("?")
        );
        if let Some(command) = &outcome.manual_command {
            println!("    manual:  {command}");
        }
        if let Some(reason) = &outcome.reason {
            println!("    reason:  {reason}");
        }
        for note in &outcome.notes {
            println!("    note:    {note}");
        }
    }
}

fn now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    chrono_like(seconds)
}

/// RFC 3339 UTC without pulling chrono into this crate.
fn chrono_like(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// Decide and run updates for already-checked installs.
pub fn update(
    host: &dyn Host,
    checked: &[Checked],
    lock_dir: Option<&Path>,
    dry_run: bool,
    mut rediscover: impl FnMut(&str) -> ProviderRediscovery,
) -> Vec<ProviderUpdateOutcome> {
    let mut ran: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut outcomes: Vec<ProviderUpdateOutcome> = vec![];
    for item in checked {
        let mut outcome = provider_updates::preflight(item, dry_run);
        if outcome.result == ProviderUpdateResult::Failed {
            // Behind with a proven installer: run it once per shared install.
            if let Some(key) = provider_updates::shared_key(item)
                && let Some(&index) = ran.get(&key)
            {
                outcome = outcomes[index].clone();
                outcome.executable = item.report.executable.clone();
                outcome.notes.push(
                    "Shares its install location and update command with the entry above; run once."
                        .into(),
                );
                outcomes.push(outcome);
                continue;
            }
            outcome = match lock_dir {
                Some(lock_dir) => provider_updates::run_update(host, item, lock_dir),
                None => {
                    let mut refused = outcome;
                    refused.reason = Some(
                        "refused: the OriginLM state directory is unavailable for the update lock"
                            .into(),
                    );
                    refused
                }
            };
            if outcome.result == ProviderUpdateResult::Updated {
                outcome.rediscovery = Some(rediscover(&item.report.provider_id));
            }
            if let Some(key) = provider_updates::shared_key(item) {
                ran.insert(key, outcomes.len());
            }
        }
        outcomes.push(outcome);
    }
    outcomes
}

pub fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<u8> {
    let options = parse(arguments)?;
    let host = SystemHost;
    let mut installs = discover(&options.home)?;
    if !options.providers.is_empty() {
        installs.retain(|install| options.providers.contains(&install.provider_id));
    }
    if let Some(executable) = &options.executable {
        let wanted = provider_updates::layout::normalize(&executable.to_string_lossy());
        installs.retain(|install| {
            provider_updates::layout::normalize(&install.executable.to_string_lossy()) == wanted
        });
    }
    let checked = provider_updates::check(&host, &installs);
    if !options.update {
        let report = ProviderUpdateCheck {
            checked_at: now(),
            providers: checked.iter().map(|item| item.report.clone()).collect(),
        };
        if options.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|_| invalid())?
            );
        } else {
            text_check(&checked);
        }
        return Ok(0);
    }
    let lock_dir = originlm_platform::default_state_dir(options.home.as_deref())
        .map(|directory| directory.join("provider-updates"));
    let home = options.home.clone();
    let outcomes = update(
        &host,
        &checked,
        lock_dir.as_deref(),
        options.dry_run,
        |provider| rediscover(&home, provider),
    );
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "dryRun": options.dry_run,
                "outcomes": outcomes,
            }))
            .map_err(|_| invalid())?
        );
    } else {
        if options.dry_run {
            println!("Dry run: nothing will be changed.");
        }
        text_outcomes(&outcomes);
    }
    Ok(u8::from(outcomes.iter().any(|outcome| {
        outcome.result == ProviderUpdateResult::Failed
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn update_requires_an_explicit_target() {
        assert!(parse(args(&["update"])).is_err());
        assert!(parse(args(&["update", "--all", "--provider", "codex"])).is_err());
        assert!(parse(args(&["update", "--provider", "nope"])).is_err());
        let options = parse(args(&["update", "--provider", "claude", "--dry-run"])).unwrap();
        assert!(options.providers.contains("claude_code") && options.dry_run);
        assert!(parse(args(&["update", "--all"])).unwrap().all);
    }

    #[test]
    fn check_rejects_update_flags() {
        assert!(parse(args(&["check", "--all"])).is_err());
        assert!(parse(args(&["check", "--dry-run"])).is_err());
        assert!(parse(args(&["check", "--home", "relative"])).is_err());
        assert!(parse(args(&["check", "--json", "--json"])).is_err());
        assert!(parse(args(&["check", "--json"])).unwrap().json);
        assert!(parse(args(&["bump"])).is_err());
    }

    #[test]
    fn rediscovery_drops_only_the_updated_provider_cache() {
        use originlm_contracts::{ConnectionMetadata, Observation};
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("state.db");
        let report = |id: &str| ConnectionMetadata {
            connection_id: id.into(),
            route_id: "route".into(),
            version: Observation::unknown("TEST"),
            auth: Observation::unknown("TEST"),
            plan: Observation::unknown("TEST"),
            billing: Observation::unknown("TEST"),
            account: None,
            models: vec![],
            additional_routes: vec![],
            providers: vec![],
            quota_pools: vec![],
            included_usage_allowed: Observation::unknown("TEST"),
            diagnostics: vec![],
        };
        {
            let (store, _) = originlm_store::StateStore::open(&database).unwrap();
            store.put_metadata(1, &report("codex-a")).unwrap();
            store.put_metadata(1, &report("claude-a")).unwrap();
        }
        let removed = invalidate_metadata(&database, &["codex-a".into(), "codex-b".into()]);
        assert_eq!(removed, Some(1));
        let (store, _) = originlm_store::StateStore::open(&database).unwrap();
        let left: Vec<String> = store
            .load()
            .unwrap()
            .metadata
            .into_iter()
            .map(|entry| entry.report.connection_id)
            .collect();
        assert_eq!(left, vec!["claude-a"]);
    }

    #[test]
    fn timestamps_are_rfc3339_utc() {
        assert_eq!(chrono_like(0), "1970-01-01T00:00:00Z");
        assert_eq!(chrono_like(1_762_905_600), "2025-11-12T00:00:00Z");
    }
}
