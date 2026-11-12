//! Ownership, status and runner behavior against a scripted host. No real
//! installer, registry or provider CLI is touched.
use super::{
    host::{CommandOutput, Host, Platform},
    layout::normalize,
    resolve::{Install, LatestSource},
    *,
};
use originlm_contracts::ProviderUpdateSupport;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Mutex,
};

#[derive(Default)]
struct FakeHost {
    windows: bool,
    env: BTreeMap<String, String>,
    which: BTreeMap<String, PathBuf>,
    links: BTreeMap<String, PathBuf>,
    files: BTreeMap<String, String>,
    dirs: BTreeSet<String>,
    http: BTreeMap<String, String>,
    /// Command line → outputs; the last one repeats.
    responses: Mutex<BTreeMap<String, VecDeque<CommandOutput>>>,
    log: Mutex<Vec<String>>,
    envs: Mutex<Vec<(String, BTreeMap<String, String>)>>,
}

fn ok(stdout: &str) -> CommandOutput {
    CommandOutput {
        success: true,
        stdout: stdout.into(),
        ..CommandOutput::default()
    }
}

fn failed(stderr: &str) -> CommandOutput {
    CommandOutput {
        success: false,
        stderr: stderr.into(),
        ..CommandOutput::default()
    }
}

impl FakeHost {
    fn posix() -> Self {
        Self::default()
    }
    fn windows() -> Self {
        Self {
            windows: true,
            ..Self::default()
        }
    }
    fn which(mut self, command: &str, path: &str) -> Self {
        self.which.insert(command.into(), PathBuf::from(path));
        self
    }
    fn link(mut self, from: &str, to: &str) -> Self {
        self.links.insert(normalize(from), PathBuf::from(to));
        self
    }
    fn file(mut self, path: &str, body: &str) -> Self {
        self.files.insert(normalize(path), body.into());
        self
    }
    fn http(mut self, url: &str, body: &str) -> Self {
        self.http.insert(url.into(), body.into());
        self
    }
    fn env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
    fn respond(self, line: &str, outputs: Vec<CommandOutput>) -> Self {
        self.responses
            .lock()
            .unwrap()
            .insert(normalize(line), outputs.into());
        self
    }
    fn ran(&self, line: &str) -> bool {
        self.log.lock().unwrap().contains(&normalize(line))
    }
}

impl Host for FakeHost {
    fn platform(&self) -> Platform {
        if self.windows {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
    fn env(&self, key: &str) -> Option<String> {
        self.env.get(key).cloned()
    }
    fn which(&self, command: &str) -> Option<PathBuf> {
        self.which.get(command).cloned()
    }
    fn real_path(&self, path: &Path) -> PathBuf {
        self.links
            .get(&normalize(&path.to_string_lossy()))
            .cloned()
            .unwrap_or_else(|| path.to_path_buf())
    }
    fn is_file(&self, path: &Path) -> bool {
        self.files.contains_key(&normalize(&path.to_string_lossy()))
    }
    fn is_dir(&self, path: &Path) -> bool {
        self.dirs.contains(&normalize(&path.to_string_lossy()))
    }
    fn read_small(&self, path: &Path, _: u64) -> Option<String> {
        self.files.get(&normalize(&path.to_string_lossy())).cloned()
    }
    fn run(
        &self,
        program: &Path,
        args: &[String],
        env: &BTreeMap<String, String>,
        _: Duration,
    ) -> Option<CommandOutput> {
        let line = normalize(&format!("{} {}", program.to_string_lossy(), args.join(" ")));
        self.log.lock().unwrap().push(line.clone());
        self.envs.lock().unwrap().push((line.clone(), env.clone()));
        let mut responses = self.responses.lock().unwrap();
        let queue = responses.get_mut(&line)?;
        if queue.len() > 1 {
            queue.pop_front()
        } else {
            queue.front().cloned()
        }
    }
    fn http_get(&self, url: &str) -> Option<String> {
        self.http.get(url).cloned()
    }
}

fn install(provider_id: &str, executable: &str) -> Install {
    Install {
        provider_id: provider_id.into(),
        name: provider_id.into(),
        executable: PathBuf::from(executable),
    }
}

const NPM: &str = "/usr/bin/npm";

fn npm_latest(host: FakeHost, package: &str, prefix: Option<&str>, version: &str) -> FakeHost {
    let prefix = prefix
        .map(|prefix| format!(" --prefix {prefix}"))
        .unwrap_or_default();
    host.which("npm", NPM).respond(
        &format!("{NPM} view --global{prefix} --json {package}@latest version"),
        vec![ok(&format!("\"{version}\""))],
    )
}

#[test]
fn npm_global_install_is_owned_by_npm_with_a_pinned_prefix() {
    let codex = "/usr/local/bin/codex";
    let real = "/usr/local/lib/node_modules/@openai/codex/bin/codex.js";
    let host = npm_latest(
        FakeHost::posix().link(codex, real),
        "@openai/codex",
        Some("/usr/local"),
        "0.60.1",
    )
    .respond(
        &format!("{codex} --version"),
        vec![ok("codex-cli 0.58.0\n")],
    );
    let checked = check_one(&host, &install("codex", codex));
    let report = &checked.report;
    assert_eq!(report.status, ProviderUpdateStatus::BehindLatest);
    assert_eq!(report.installed_version.as_deref(), Some("0.58.0"));
    assert_eq!(report.latest_version.as_deref(), Some("0.60.1"));
    assert_eq!(report.installer.as_deref(), Some("npm-global"));
    assert_eq!(
        report.update_command.as_deref(),
        Some(
            "npm install -g --prefix /usr/local --allow-scripts=@openai/codex @openai/codex@latest"
        )
    );
    assert_eq!(report.lock_key.as_deref(), Some("npm-global:/usr/local"));
    assert!(
        report
            .latest_source
            .as_deref()
            .unwrap()
            .starts_with("npm view @openai/codex@latest")
    );
}

#[test]
fn windows_npm_shim_is_proven_by_the_package_beside_it() {
    let shim = r"C:\Users\u\AppData\Roaming\npm\codex.cmd";
    let host = npm_latest(
        FakeHost::windows().file(
            r"C:\Users\u\AppData\Roaming\npm\node_modules\@openai\codex\package.json",
            "{}",
        ),
        "@openai/codex",
        Some(r"C:\Users\u\AppData\Roaming\npm"),
        "0.60.1",
    )
    .respond(&format!("{shim} --version"), vec![ok("codex-cli 0.60.1")]);
    let report = check_one(&host, &install("codex", shim)).report;
    assert_eq!(report.status, ProviderUpdateStatus::Current);
    assert_eq!(report.installer.as_deref(), Some("npm-global"));
    // Without the package beside the shim nothing is proven.
    let bare = npm_latest(FakeHost::windows(), "@openai/codex", None, "0.60.1")
        .respond(&format!("{shim} --version"), vec![ok("codex-cli 0.58.0")]);
    let report = check_one(&bare, &install("codex", shim)).report;
    assert_eq!(report.status, ProviderUpdateStatus::ManualOnly);
    assert_eq!(report.update_command, None);
}

#[test]
fn claude_native_install_uses_its_own_updater_and_release_channel() {
    let claude = r"C:\Users\u\.local\bin\claude.exe";
    let host = FakeHost::windows()
        .env("USERPROFILE", r"C:\Users\u")
        .file(
            r"C:\Users\u\.claude\settings.json",
            r#"{"autoUpdatesChannel":"stable"}"#,
        )
        .http(
            "https://downloads.claude.ai/claude-code-releases/stable",
            "2.1.40\n",
        )
        .respond(
            &format!("{claude} --version"),
            vec![ok("2.1.37 (Claude Code)")],
        );
    let report = check_one(&host, &install("claude_code", claude)).report;
    assert_eq!(report.status, ProviderUpdateStatus::BehindLatest);
    assert_eq!(report.latest_version.as_deref(), Some("2.1.40"));
    assert_eq!(
        report.update_command.as_deref(),
        Some(r"C:\Users\u\.local\bin\claude.exe update")
    );
    assert!(report.latest_source.unwrap().contains("/stable"));
}

#[test]
fn claude_outside_its_native_layout_is_manual_only() {
    // A plain ~/.local/bin/claude on Unix is a wrapper `claude update` cannot see.
    let claude = "/home/u/.local/bin/claude";
    let host = npm_latest(
        FakeHost::posix(),
        "@anthropic-ai/claude-code",
        None,
        "2.1.40",
    )
    .respond(
        &format!("{claude} --version"),
        vec![ok("2.1.37 (Claude Code)")],
    );
    let report = check_one(&host, &install("claude_code", claude)).report;
    assert_eq!(report.status, ProviderUpdateStatus::ManualOnly);
    assert_eq!(report.update_command, None);
    assert!(report.reason.unwrap().contains("no installer is proven"));
}

#[test]
fn chocolatey_claude_shows_choco_upgrade_and_never_runs_it() {
    let claude = r"C:\ProgramData\chocolatey\bin\claude.exe";
    let host = FakeHost::windows()
        .which("choco", r"C:\ProgramData\chocolatey\bin\choco.exe")
        .respond(
            r"C:\ProgramData\chocolatey\bin\choco.exe search claude-code --exact --limit-output",
            vec![ok("claude-code|2.1.40\n")],
        )
        .respond(
            &format!("{claude} --version"),
            vec![ok("2.1.37 (Claude Code)")],
        );
    let checked = check_one(&host, &install("claude_code", claude));
    assert_eq!(checked.report.status, ProviderUpdateStatus::ManualOnly);
    assert_eq!(
        checked.report.manual_command.as_deref(),
        Some("choco upgrade claude-code")
    );
    assert_eq!(checked.report.latest_version.as_deref(), Some("2.1.40"));
    assert_eq!(checked.report.update_command, None);
    let outcome = run_update(&host, &checked, Path::new("/unused"));
    assert_eq!(outcome.result, ProviderUpdateResult::Skipped);
    assert!(!host.ran("choco upgrade claude-code"));
    assert!(!host.ran(&format!("{claude} update")));
}

#[test]
fn winget_and_nix_installs_are_manual_only() {
    let winget = r"C:\Users\u\AppData\Local\Microsoft\WinGet\Links\claude.exe";
    let host = FakeHost::windows();
    let plan = resolve::resolve(&host, &install("claude_code", winget), Some("2.1.0"));
    assert_eq!(plan.update, None);
    assert_eq!(plan.installer.as_deref(), Some("winget"));
    let nix = "/nix/store/abc-claude-code/bin/claude";
    let plan = resolve::resolve(
        &FakeHost::posix(),
        &install("claude_code", nix),
        Some("2.1.0"),
    );
    assert_eq!(plan.update, None);
    assert_eq!(plan.installer.as_deref(), Some("nix"));
}

#[test]
fn homebrew_runs_only_when_brew_vouches_for_the_prefix() {
    let opencode = "/opt/homebrew/bin/opencode";
    let real = "/opt/homebrew/Cellar/opencode/2.2.0/bin/opencode";
    let base = || {
        FakeHost::posix()
            .link(opencode, real)
            .file("/opt/homebrew/bin/brew", "")
    };
    let host = base().respond(
        "/opt/homebrew/bin/brew --prefix",
        vec![ok("/opt/homebrew\n")],
    );
    let plan = resolve::resolve(&host, &install("opencode", opencode), Some("2.2.0"));
    assert_eq!(plan.installer.as_deref(), Some("homebrew"));
    assert_eq!(plan.update.unwrap().display, "brew upgrade opencode");
    assert!(matches!(plan.latest, LatestSource::Homebrew { .. }));
    let host = base().respond("/opt/homebrew/bin/brew --prefix", vec![ok("/usr/local\n")]);
    let plan = resolve::resolve(&host, &install("opencode", opencode), Some("2.2.0"));
    assert_eq!(plan.update, None);
    assert_eq!(plan.support, ProviderUpdateSupport::Manual);
}

#[test]
fn grok_cask_without_a_verified_prefix_stays_manual() {
    let grok = "/opt/homebrew/bin/grok";
    let host = FakeHost::posix().link(grok, "/opt/homebrew/Caskroom/grok/1.0.0/grok");
    let plan = resolve::resolve(&host, &install("grok", grok), Some("1.0.0"));
    assert_eq!(plan.update, None);
    assert_eq!(
        plan.manual_command.as_deref(),
        Some("brew upgrade --cask grok")
    );
    // A plain install lets `grok update` detect its own installer.
    let plain = "/home/u/.grok/bin/grok";
    let plan = resolve::resolve(&FakeHost::posix(), &install("grok", plain), Some("1.0.0"));
    let update = plan.update.unwrap();
    assert_eq!(update.args, vec!["update"]);
    assert_eq!(
        update
            .env
            .get("npm_config_allow_scripts")
            .map(String::as_str),
        Some("@xai-official/grok")
    );
}

fn mise_host(requested: &str, configured: &str) -> FakeHost {
    let install_path = "/home/u/.local/share/mise/installs/claude/2.1.37";
    FakeHost::posix()
        .which("mise", "/usr/bin/mise")
        .link(configured, &format!("{install_path}/bin/claude"))
        .respond(
            "/usr/bin/mise which claude",
            vec![ok(&format!("{install_path}/bin/claude\n"))],
        )
        .link(
            &format!("{install_path}/bin/claude"),
            &format!("{install_path}/bin/claude"),
        )
        .respond(
            "/usr/bin/mise ls --json --installed claude",
            vec![ok(&format!(
                r#"[{{"version":"2.1.37","requested_version":"{requested}","install_path":"{install_path}","active":true}}]"#
            ))],
        )
}

#[test]
fn mise_upgrades_only_latest_requests_through_the_latest_link() {
    let latest_link = "/home/u/.local/share/mise/installs/claude/latest/bin/claude";
    let plan = resolve::resolve(
        &mise_host("latest", latest_link),
        &install("claude_code", latest_link),
        Some("2.1.37"),
    );
    assert_eq!(plan.installer.as_deref(), Some("mise"));
    assert_eq!(plan.update.unwrap().display, "mise upgrade claude");
    assert!(matches!(plan.latest, LatestSource::Mise { .. }));
    // Pinned: mise config is never edited.
    let plan = resolve::resolve(
        &mise_host("2.1.37", latest_link),
        &install("claude_code", latest_link),
        Some("2.1.37"),
    );
    assert_eq!(plan.update, None);
    assert!(plan.reason.unwrap().contains("never edits mise config"));
    // A version directory would keep running the old binary.
    let version_dir = "/home/u/.local/share/mise/installs/claude/2.1.37/bin/claude";
    let plan = resolve::resolve(
        &mise_host("latest", version_dir),
        &install("claude_code", version_dir),
        Some("2.1.37"),
    );
    assert_eq!(plan.update, None);
    assert!(plan.reason.unwrap().contains("version directory"));
}

#[test]
fn opencode_generation_selects_package_and_mise_policy() {
    let opencode = "/usr/local/bin/opencode";
    let v1 = "/usr/local/lib/node_modules/opencode-ai/bin/opencode";
    let host = FakeHost::posix().which("npm", NPM).link(opencode, v1);
    let plan = resolve::resolve(&host, &install("opencode", opencode), Some("1.4.0"));
    assert!(plan.update.unwrap().display.contains("opencode-ai@latest"));
    // A 2.x binary is not proven by the 1.x package path.
    let plan = resolve::resolve(&host, &install("opencode", opencode), Some("2.2.0"));
    assert_eq!(plan.update, None);
    // The desktop app's bundled CLI is the app's to update.
    let bundled =
        r"C:\Users\u\AppData\Local\Programs\@opencode-aidesktop\resources\opencode-cli.exe";
    let plan = resolve::resolve(
        &FakeHost::windows(),
        &install("opencode", bundled),
        Some("2.2.0"),
    );
    assert_eq!(plan.update, None);
    assert_eq!(plan.latest, LatestSource::None);
}

#[test]
fn pi_runs_its_own_updater_unless_inside_node_modules() {
    let pi = r"C:\Users\u\.pi\agent\bin\pi.cmd";
    let plan = resolve::resolve(&FakeHost::windows(), &install("pi", pi), Some("1.5.0"));
    assert_eq!(plan.update.unwrap().args, vec!["update", "--self"]);
    let nested = "/repo/node_modules/.bin/pi";
    let plan = resolve::resolve(&FakeHost::posix(), &install("pi", nested), Some("1.5.0"));
    assert_eq!(plan.update, None);
}

#[test]
fn managed_and_unsupported_providers_report_real_reasons() {
    let host = FakeHost::windows();
    let agy = r"C:\Users\u\.t3\tools\antigravity-acp\win32-x64\versions\h\agy_acp_server.exe";
    let plan = resolve::resolve(&host, &install("antigravity", agy), None);
    assert_eq!(plan.installer.as_deref(), Some("t3code"));
    assert!(plan.reason.unwrap().contains("T3 Code"));
    let cursor = r"C:\Users\u\AppData\Local\cursor-agent\cursor-agent.cmd";
    let plan = resolve::resolve(&host, &install("cursor", cursor), None);
    assert_eq!((plan.update, plan.latest), (None, LatestSource::None));
    let plan = resolve::resolve(&host, &install("ollama", r"C:\o\ollama.exe"), None);
    assert_eq!(plan.support, ProviderUpdateSupport::Unsupported);
}

#[test]
fn an_unreadable_latest_version_is_unknown_never_guessed() {
    let codex = "/usr/local/bin/codex";
    let host = FakeHost::posix()
        .which("npm", NPM)
        .link(
            codex,
            "/usr/local/lib/node_modules/@openai/codex/bin/codex.js",
        )
        .respond(&format!("{codex} --version"), vec![ok("codex-cli 0.58.0")]);
    let checked = check_one(&host, &install("codex", codex));
    assert_eq!(checked.report.status, ProviderUpdateStatus::Unknown);
    assert_eq!(checked.report.latest_version, None);
    let outcome = preflight(&checked, false);
    assert_eq!(outcome.result, ProviderUpdateResult::Skipped);
    // And an unreadable installed version.
    let host = npm_latest(
        FakeHost::posix(),
        "@openai/codex",
        Some("/usr/local"),
        "0.60.1",
    )
    .link(
        codex,
        "/usr/local/lib/node_modules/@openai/codex/bin/codex.js",
    )
    .respond(&format!("{codex} --version"), vec![failed("boom")]);
    assert_eq!(
        check_one(&host, &install("codex", codex)).report.status,
        ProviderUpdateStatus::Unknown
    );
}

const UPDATE_LINE: &str = "/usr/bin/npm install -g --prefix /usr/local --allow-scripts=@openai/codex @openai/codex@latest";

fn behind_codex(versions: Vec<CommandOutput>, update: Vec<CommandOutput>) -> FakeHost {
    let codex = "/usr/local/bin/codex";
    npm_latest(
        FakeHost::posix(),
        "@openai/codex",
        Some("/usr/local"),
        "0.60.1",
    )
    .link(
        codex,
        "/usr/local/lib/node_modules/@openai/codex/bin/codex.js",
    )
    .respond(&format!("{codex} --version"), versions)
    .respond(UPDATE_LINE, update)
}

#[test]
fn dry_run_prints_the_command_and_runs_nothing() {
    let host = behind_codex(vec![ok("codex-cli 0.58.0")], vec![ok("")]);
    let checked = check(&host, &[install("codex", "/usr/local/bin/codex")]);
    let lock = tempfile::tempdir().unwrap();
    let outcomes = crate::providers_cli::update(&host, &checked, Some(lock.path()), true, |_| {
        unreachable!("dry run never rediscovers")
    });
    assert_eq!(outcomes[0].result, ProviderUpdateResult::WouldRun);
    assert!(
        outcomes[0]
            .command
            .as_deref()
            .unwrap()
            .starts_with("npm install -g --prefix")
    );
    assert!(outcomes[0].ownership_proof.is_some());
    assert!(!host.ran(UPDATE_LINE));
    assert_eq!(std::fs::read_dir(lock.path()).unwrap().count(), 0);
}

#[test]
fn success_requires_the_reprobed_version_to_be_current_then_rediscovers() {
    let host = behind_codex(
        vec![
            ok("codex-cli 0.58.0"),
            ok("codex-cli 0.58.0"),
            ok("codex-cli 0.60.1"),
        ],
        vec![ok("added 1 package")],
    );
    let checked = check(&host, &[install("codex", "/usr/local/bin/codex")]);
    let lock = tempfile::tempdir().unwrap();
    let mut rediscovered = vec![];
    let outcomes =
        crate::providers_cli::update(&host, &checked, Some(lock.path()), false, |provider| {
            rediscovered.push(provider.to_owned());
            originlm_contracts::ProviderRediscovery {
                connections: vec!["codex:default".into()],
                invalidated_metadata: 1,
                note: None,
            }
        });
    assert!(host.ran(UPDATE_LINE));
    assert_eq!(outcomes[0].result, ProviderUpdateResult::Updated);
    assert_eq!(outcomes[0].installed_version.as_deref(), Some("0.60.1"));
    assert_eq!(rediscovered, vec!["codex"]);
    assert!(outcomes[0].notes[0].contains("next session"));
}

#[test]
fn an_updater_that_exits_zero_without_updating_fails() {
    let host = behind_codex(vec![ok("codex-cli 0.58.0")], vec![ok("up to date")]);
    let checked = check_one(&host, &install("codex", "/usr/local/bin/codex"));
    let lock = tempfile::tempdir().unwrap();
    let outcome = run_update(&host, &checked, lock.path());
    assert_eq!(outcome.result, ProviderUpdateResult::Failed);
    assert!(outcome.reason.unwrap().contains("still reports 0.58.0"));
}

#[test]
fn a_failed_installer_reports_its_last_error_line() {
    let host = behind_codex(
        vec![ok("codex-cli 0.58.0")],
        vec![failed("npm warn something\nnpm error code EACCES\n\n")],
    );
    let checked = check_one(&host, &install("codex", "/usr/local/bin/codex"));
    let lock = tempfile::tempdir().unwrap();
    let outcome = run_update(&host, &checked, lock.path());
    assert_eq!(outcome.result, ProviderUpdateResult::Failed);
    assert_eq!(outcome.reason.as_deref(), Some("npm error code EACCES"));
}

#[test]
fn a_changed_owner_since_the_check_is_refused_before_running() {
    let host = behind_codex(vec![ok("codex-cli 0.58.0")], vec![ok("")]);
    let mut checked = check_one(&host, &install("codex", "/usr/local/bin/codex"));
    // Simulate the status having been computed against a different install.
    checked.plan.real_executable = PathBuf::from("/opt/other/codex");
    let lock = tempfile::tempdir().unwrap();
    let outcome = run_update(&host, &checked, lock.path());
    assert_eq!(outcome.result, ProviderUpdateResult::Failed);
    assert!(outcome.reason.unwrap().contains("changed since the status"));
    assert!(!host.ran(UPDATE_LINE));
}

#[test]
fn one_update_at_a_time_per_install_location() {
    let host = behind_codex(vec![ok("codex-cli 0.58.0")], vec![ok("")]);
    let checked = check_one(&host, &install("codex", "/usr/local/bin/codex"));
    let lock = tempfile::tempdir().unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_file(lock.path(), "npm-global:/usr/local"))
        .unwrap();
    fs4::FileExt::try_lock(&held).unwrap();
    let outcome = run_update(&host, &checked, lock.path());
    assert_eq!(outcome.result, ProviderUpdateResult::Failed);
    assert!(outcome.reason.unwrap().contains("already running"));
    assert!(!host.ran(UPDATE_LINE));
}

#[test]
fn installs_sharing_one_owner_run_the_installer_once() {
    let host = behind_codex(
        vec![
            ok("codex-cli 0.58.0"),
            ok("codex-cli 0.58.0"),
            ok("codex-cli 0.58.0"),
            ok("codex-cli 0.58.0"),
            ok("codex-cli 0.60.1"),
        ],
        vec![ok("")],
    );
    let codex = install("codex", "/usr/local/bin/codex");
    let checked = vec![check_one(&host, &codex), check_one(&host, &codex)];
    let lock = tempfile::tempdir().unwrap();
    let outcomes = crate::providers_cli::update(&host, &checked, Some(lock.path()), false, |_| {
        originlm_contracts::ProviderRediscovery {
            connections: vec![],
            invalidated_metadata: 0,
            note: None,
        }
    });
    let runs = host
        .log
        .lock()
        .unwrap()
        .iter()
        .filter(|line| **line == normalize(UPDATE_LINE))
        .count();
    assert_eq!(runs, 1);
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes[1]
            .notes
            .iter()
            .any(|note| note.contains("run once"))
    );
}

#[test]
fn pnpm_pins_the_exact_latest_version() {
    let codex = r"C:\Users\u\AppData\Local\pnpm\codex.cmd";
    let host = npm_latest(FakeHost::windows(), "@openai/codex", None, "0.60.1")
        .which("pnpm", r"C:\Users\u\AppData\Local\pnpm\pnpm.cmd")
        .respond(
            r"C:\Users\u\AppData\Local\pnpm\pnpm.cmd --version",
            vec![ok("10.5.0")],
        )
        .respond(&format!("{codex} --version"), vec![ok("codex-cli 0.58.0")]);
    let report = check_one(&host, &install("codex", codex)).report;
    assert_eq!(
        report.update_command.as_deref(),
        Some("pnpm add -g --allow-build=@openai/codex @openai/codex@0.60.1")
    );
}

#[test]
fn system_processes_see_stdin_at_eof() {
    // A command that waits for a line would hang forever if stdin were inherited.
    let (program, args): (PathBuf, Vec<String>) = if cfg!(windows) {
        (
            PathBuf::from(std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into())),
            vec!["/D".into(), "/C".into(), "set /p line= & echo done".into()],
        )
    } else {
        (
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), "read line; echo done".into()],
        )
    };
    let started = std::time::Instant::now();
    let output = host::SystemHost
        .run(&program, &args, &BTreeMap::new(), Duration::from_secs(20))
        .unwrap();
    assert!(!output.timed_out);
    assert!(output.stdout.contains("done"));
    assert!(started.elapsed() < Duration::from_secs(15));
}

#[test]
fn system_processes_are_killed_at_the_timeout() {
    let (program, args): (PathBuf, Vec<String>) = if cfg!(windows) {
        (
            PathBuf::from(std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into())),
            vec!["/D".into(), "/C".into(), "ping -n 30 127.0.0.1 >NUL".into()],
        )
    } else {
        (
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), "sleep 30".into()],
        )
    };
    let started = std::time::Instant::now();
    let output = host::SystemHost
        .run(
            &program,
            &args,
            &BTreeMap::new(),
            Duration::from_millis(300),
        )
        .unwrap();
    assert!(output.timed_out && !output.success);
    // The whole tree is killed, so a grandchild cannot hold the pipes open.
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_status_probe_never_lets_muse_update_itself() {
    let muse = r"C:\Users\u\AppData\Local\Programs\muse\muse.cmd";
    let host = FakeHost::windows()
        .http(
            "https://api.meta.ai/muse-code/channels/muse-stable",
            r#"{"version":"1.4.18-R8587.1"}"#,
        )
        .respond(&format!("{muse} --version"), vec![ok("1.4.18")]);
    let report = check_one(&host, &install("muse", muse)).report;
    // `--version` omits the revision; the base versions are equal.
    assert_eq!(report.status, ProviderUpdateStatus::Current);
    let envs = host.envs.lock().unwrap();
    let (_, env) = envs
        .iter()
        .find(|(line, _)| line.ends_with("muse.cmd --version"))
        .unwrap();
    assert_eq!(
        env.get("MUSE_NO_AUTO_UPDATE").map(String::as_str),
        Some("1")
    );
}
