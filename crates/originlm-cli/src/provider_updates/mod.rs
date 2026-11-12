//! Provider CLI update status and explicit, operator-run updates.
//!
//! `check` probes each discovered provider CLI's installed version, asks the
//! installer that owns it for the latest version, and reports the standing.
//! `update` runs only an installer proven to own the binary, one at a time per
//! install location, re-reads ownership immediately before running, and counts
//! success only when the re-probed version is installed, readable and current.
//! Nothing here runs on discovery, refresh, or any broker/SDK/MCP request.
pub mod host;
pub mod layout;
pub mod resolve;
pub mod version;

#[cfg(test)]
mod tests;

use host::Host;
use originlm_contracts::{
    ProviderUpdateOutcome, ProviderUpdateReport, ProviderUpdateResult, ProviderUpdateStatus,
    VersionEvidence,
};
use resolve::{Install, Plan};
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

/// Native updaters such as `claude update` download hundreds of MB without
/// progress; killing one mid-download leaves a truncated binary.
pub const UPDATE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// A report plus the private plan behind it, kept for the update step.
#[derive(Debug, Clone)]
pub struct Checked {
    pub install: Install,
    pub plan: Plan,
    pub report: ProviderUpdateReport,
}

/// Environment for a version probe. A status probe must never update anything,
/// and the Muse Code launcher refreshes itself on start unless told not to.
fn probe_env(provider_id: &str) -> BTreeMap<String, String> {
    match provider_id {
        "muse" => BTreeMap::from([("MUSE_NO_AUTO_UPDATE".to_owned(), "1".to_owned())]),
        _ => BTreeMap::new(),
    }
}

/// Run `<exe> --version` and read the first version token.
pub fn probe_version(
    host: &dyn Host,
    provider_id: &str,
    executable: &Path,
) -> (Option<String>, VersionEvidence) {
    let command = format!("{} --version", executable.to_string_lossy());
    let output = host.run(
        executable,
        &["--version".to_owned()],
        &probe_env(provider_id),
        resolve::PROBE_TIMEOUT,
    );
    let Some(output) = output else {
        return (
            None,
            VersionEvidence {
                command,
                exit_ok: false,
                output: "could not start".into(),
            },
        );
    };
    let first_line = |text: &str| {
        text.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| line.chars().take(200).collect::<String>())
    };
    let line = first_line(&output.stdout)
        .or_else(|| first_line(&output.stderr))
        .unwrap_or_default();
    let version = if output.success {
        version::extract(&output.stdout).or_else(|| version::extract(&output.stderr))
    } else {
        None
    };
    (
        version,
        VersionEvidence {
            command,
            exit_ok: output.success,
            output: if output.timed_out {
                "timed out".into()
            } else {
                line
            },
        },
    )
}

fn status(
    provider_id: &str,
    installed: Option<&str>,
    latest: Option<&str>,
    plan: &Plan,
) -> (ProviderUpdateStatus, Option<String>) {
    let Some(installed) = installed else {
        return (
            ProviderUpdateStatus::Unknown,
            Some("the installed version could not be read from `--version`".into()),
        );
    };
    let Some(latest) = latest else {
        let reason = plan
            .reason
            .clone()
            .unwrap_or_else(|| "the owning installer did not report a latest version".into());
        return (ProviderUpdateStatus::Unknown, Some(reason));
    };
    match version::compare(provider_id, installed, latest) {
        None => (
            ProviderUpdateStatus::Unknown,
            Some(format!(
                "versions {installed} and {latest} are not comparable"
            )),
        ),
        Some(Ordering::Less) if plan.update.is_some() => {
            (ProviderUpdateStatus::BehindLatest, None)
        }
        Some(Ordering::Less) => (
            ProviderUpdateStatus::ManualOnly,
            Some(plan.reason.clone().unwrap_or_else(|| {
                "no installer is proven to own this binary; update it with the tool that installed it"
                    .into()
            })),
        ),
        Some(_) => (ProviderUpdateStatus::Current, plan.reason.clone()),
    }
}

/// The exact action for a known latest version: pnpm pins it (see `Plan::pin_latest`).
fn final_action(plan: &Plan, latest: Option<&str>) -> Option<resolve::UpdateAction> {
    let mut update = plan.update.clone()?;
    if plan.pin_latest
        && let Some(latest) = latest
    {
        for arg in &mut update.args {
            if let Some(package) = arg.strip_suffix("@latest") {
                let pinned = format!("{package}@{latest}");
                update.display = update.display.replace(arg.as_str(), &pinned);
                *arg = pinned;
            }
        }
    }
    Some(update)
}

pub fn check_one(host: &dyn Host, install: &Install) -> Checked {
    let (installed, evidence) = probe_version(host, &install.provider_id, &install.executable);
    let plan = resolve::resolve(host, install, installed.as_deref());
    let (latest, latest_source) = resolve::latest_version(host, &plan.latest);
    let (status, reason) = status(
        &install.provider_id,
        installed.as_deref(),
        latest.as_deref(),
        &plan,
    );
    let update = final_action(&plan, latest.as_deref());
    let report = ProviderUpdateReport {
        provider_id: install.provider_id.clone(),
        name: install.name.clone(),
        executable: install.executable.to_string_lossy().into_owned(),
        real_executable: plan.real_executable.to_string_lossy().into_owned(),
        installed_version: installed,
        version_evidence: Some(evidence),
        latest_version: latest,
        latest_source,
        status,
        support: plan.support,
        installer: plan.installer.clone(),
        ownership_proof: plan.proof.clone(),
        update_command: update.as_ref().map(|update| update.display.clone()),
        manual_command: plan.manual_command.clone(),
        lock_key: update.as_ref().map(|update| update.lock_key.clone()),
        reason,
    };
    Checked {
        install: install.clone(),
        plan,
        report,
    }
}

/// Installs are independent probes, so they run concurrently; order is kept.
pub fn check(host: &dyn Host, installs: &[Install]) -> Vec<Checked> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = installs
            .iter()
            .map(|install| scope.spawn(move || check_one(host, install)))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// The last non-empty line an installer printed, stderr first.
fn last_error_line(stderr: &str, stdout: &str) -> Option<String> {
    let last = |text: &str| {
        text.lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .map(|line| line.chars().take(300).collect::<String>())
    };
    last(stderr).or_else(|| last(stdout))
}

fn lock_file(lock_dir: &Path, lock_key: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(lock_key.as_bytes());
    let name: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
    lock_dir.join(format!("{name}.lock"))
}

fn outcome(checked: &Checked, result: ProviderUpdateResult) -> ProviderUpdateOutcome {
    let report = &checked.report;
    ProviderUpdateOutcome {
        provider_id: report.provider_id.clone(),
        executable: report.executable.clone(),
        result,
        command: report.update_command.clone(),
        ownership_proof: report.ownership_proof.clone(),
        previous_version: report.installed_version.clone(),
        installed_version: report.installed_version.clone(),
        latest_version: report.latest_version.clone(),
        manual_command: report.manual_command.clone(),
        reason: report.reason.clone(),
        rediscovery: None,
        notes: vec![],
    }
}

fn session_notes(provider_id: &str) -> Vec<String> {
    let mut notes = vec![
        "Running sessions keep the previous CLI until they end; the new version applies to the next session."
            .to_owned(),
    ];
    if provider_id == "opencode" {
        notes.push(
            "Restart the configured local OpenCode server to use the new version.".to_owned(),
        );
    }
    notes
}

/// What `update` decides for one checked install before anything runs.
pub fn preflight(checked: &Checked, dry_run: bool) -> ProviderUpdateOutcome {
    let report = &checked.report;
    let mut result = outcome(checked, ProviderUpdateResult::Skipped);
    match report.status {
        ProviderUpdateStatus::Current => {
            result.result = ProviderUpdateResult::AlreadyCurrent;
            result.command = None;
        }
        ProviderUpdateStatus::ManualOnly => {
            result.command = None;
        }
        ProviderUpdateStatus::Unknown => {
            result.command = None;
            result.reason = Some(format!(
                "not updated: {} (an update cannot be proven without both versions)",
                report
                    .reason
                    .as_deref()
                    .unwrap_or("the version standing is unknown")
            ));
        }
        ProviderUpdateStatus::BehindLatest if dry_run => {
            result.result = ProviderUpdateResult::WouldRun;
            result.notes = session_notes(&report.provider_id);
        }
        ProviderUpdateStatus::BehindLatest => {
            // The caller runs it.
            result.result = ProviderUpdateResult::Failed;
            result.reason = Some("not run".into());
        }
    }
    result
}

/// Run the proven installer for one behind install.
///
/// `lock_dir` holds one lock file per install location. Ownership is resolved
/// again immediately before running; a changed owner, command or binary is
/// refused. Success requires the re-probed version to be readable and current.
pub fn run_update(host: &dyn Host, checked: &Checked, lock_dir: &Path) -> ProviderUpdateOutcome {
    let mut result = outcome(checked, ProviderUpdateResult::Failed);
    result.reason = None;
    let fail = |mut result: ProviderUpdateOutcome, reason: String| {
        result.reason = Some(reason);
        result
    };
    if checked.report.status != ProviderUpdateStatus::BehindLatest {
        return preflight(checked, false);
    }
    let Some(planned) = final_action(&checked.plan, checked.report.latest_version.as_deref())
    else {
        return fail(result, "no proven installer".into());
    };
    if std::fs::create_dir_all(lock_dir).is_err() {
        return fail(result, "the update lock directory is unavailable".into());
    }
    let Ok(lock) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_file(lock_dir, &planned.lock_key))
    else {
        return fail(result, "the update lock is unavailable".into());
    };
    if fs4::FileExt::try_lock(&lock).is_err() {
        return fail(
            result,
            "another update is already running for this install location".into(),
        );
    }
    // Re-read ownership now: the status may be stale.
    let (fresh_version, _) = probe_version(
        host,
        &checked.install.provider_id,
        &checked.install.executable,
    );
    let fresh = resolve::resolve(host, &checked.install, fresh_version.as_deref());
    let fresh_action = final_action(&fresh, checked.report.latest_version.as_deref());
    if fresh.real_executable != checked.plan.real_executable
        || fresh_action
            .as_ref()
            .map(|a| (&a.lock_key, &a.executable, &a.args, &a.env))
            != Some((
                &planned.lock_key,
                &planned.executable,
                &planned.args,
                &planned.env,
            ))
    {
        return fail(
            result,
            "refused: the install's owner or update command changed since the status was computed; run `originlm providers check` again".into(),
        );
    }
    if fresh_version != checked.report.installed_version {
        return fail(
            result,
            "refused: the installed version changed since the status was computed; run `originlm providers check` again".into(),
        );
    }
    let output = host.run(
        &planned.executable,
        &planned.args,
        &planned.env,
        UPDATE_TIMEOUT,
    );
    let (version_after, _) = probe_version(
        host,
        &checked.install.provider_id,
        &checked.install.executable,
    );
    result.installed_version = version_after.clone();
    let _ = fs4::FileExt::unlock(&lock);
    let Some(output) = output else {
        return fail(result, format!("could not start {}", planned.display));
    };
    if output.timed_out {
        return fail(
            result,
            format!(
                "timed out after {} minutes; the install may be incomplete",
                UPDATE_TIMEOUT.as_secs() / 60
            ),
        );
    }
    if !output.success {
        return fail(
            result,
            last_error_line(&output.stderr, &output.stdout)
                .unwrap_or_else(|| "the installer failed without output".into()),
        );
    }
    let latest = checked.report.latest_version.as_deref().unwrap_or_default();
    let Some(after) = version_after else {
        return fail(
            result,
            "the installer exited 0, but the CLI's version can no longer be read".into(),
        );
    };
    match version::compare(&checked.report.provider_id, &after, latest) {
        Some(Ordering::Equal | Ordering::Greater) => {
            result.result = ProviderUpdateResult::Updated;
            result.notes = session_notes(&checked.report.provider_id);
            result
        }
        _ => fail(
            result,
            format!(
                "the installer exited 0, but {} still reports {after} (latest {latest})",
                checked.report.executable
            ),
        ),
    }
}

/// Group key: installs sharing one update command and lock run it once.
pub fn shared_key(checked: &Checked) -> Option<(String, String)> {
    Some((
        checked.report.lock_key.clone()?,
        checked.report.update_command.clone()?,
    ))
}
