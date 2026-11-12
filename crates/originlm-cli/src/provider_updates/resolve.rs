//! Which installer owns a provider CLI binary, and what (if anything) OriginLM
//! may run to update it. Ported from T3 Code's provider maintenance resolver:
//! an installer runs only when the resolved real path proves it owns the
//! install; everything else is manual-only with the reason shown.
use super::{
    host::{Host, Platform},
    layout,
};
use originlm_contracts::ProviderUpdateSupport;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// A provider CLI binary discovery found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub provider_id: String,
    pub name: String,
    pub executable: PathBuf,
}

/// A command OriginLM may run because its installer is proven to own the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAction {
    pub executable: PathBuf,
    pub args: Vec<String>,
    /// Added to the inherited environment.
    pub env: BTreeMap<String, String>,
    pub display: String,
    pub lock_key: String,
}

/// Where the latest version comes from. Each is the owning installer's own channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatestSource {
    None,
    Npm {
        package: String,
        prefix: Option<String>,
    },
    Mise {
        mise: PathBuf,
        tool: String,
    },
    Homebrew {
        brew: PathBuf,
        name: String,
        cask: bool,
    },
    Chocolatey {
        package: String,
    },
    ClaudeChannel,
    CodexChannel,
    MuseChannel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub real_executable: PathBuf,
    pub installer: Option<String>,
    pub proof: Option<String>,
    pub update: Option<UpdateAction>,
    pub manual_command: Option<String>,
    pub latest: LatestSource,
    pub support: ProviderUpdateSupport,
    pub reason: Option<String>,
    /// pnpm resolves `@latest` through `minimumReleaseAge`; pin the exact version instead.
    pub pin_latest: bool,
}

impl Plan {
    fn manual(real: &Path, latest: LatestSource, reason: impl Into<String>) -> Self {
        Self {
            real_executable: real.to_path_buf(),
            installer: None,
            proof: None,
            update: None,
            manual_command: None,
            latest,
            support: ProviderUpdateSupport::Manual,
            reason: Some(reason.into()),
            pin_latest: false,
        }
    }

    fn unsupported(real: &Path, reason: impl Into<String>) -> Self {
        Self {
            support: ProviderUpdateSupport::Unsupported,
            ..Self::manual(real, LatestSource::None, reason)
        }
    }

    fn installer(
        real: &Path,
        installer: &str,
        proof: String,
        update: UpdateAction,
        latest: LatestSource,
    ) -> Self {
        Self {
            real_executable: real.to_path_buf(),
            installer: Some(installer.to_owned()),
            proof: Some(proof),
            update: Some(update),
            manual_command: None,
            latest,
            support: ProviderUpdateSupport::Installer,
            reason: None,
            pin_latest: false,
        }
    }

    fn with_manual_command(mut self, command: impl Into<String>) -> Self {
        self.manual_command = Some(command.into());
        self
    }

    fn with_installer(mut self, installer: &str, proof: impl Into<String>) -> Self {
        self.installer = Some(installer.to_owned());
        self.proof = Some(proof.into());
        self
    }
}

fn action(
    host: &dyn Host,
    executable: PathBuf,
    args: &[&str],
    env: BTreeMap<String, String>,
    lock_key: String,
    display_program: Option<&str>,
) -> UpdateAction {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let windows = host.platform() == Platform::Windows;
    let program = display_program
        .map(str::to_owned)
        .unwrap_or_else(|| executable.to_string_lossy().into_owned());
    let mut display = layout::display_command(&program, &args, windows);
    if !env.is_empty() && !windows {
        let prefix: Vec<String> = env
            .iter()
            .map(|(key, value)| format!("{key}={}", layout::quote(value, false)))
            .collect();
        display = format!("{} {display}", prefix.join(" "));
    } else if !env.is_empty() {
        let prefix: Vec<String> = env
            .iter()
            .map(|(key, value)| format!("$env:{key}={};", layout::quote(value, true)))
            .collect();
        display = format!("{} {display}", prefix.join(" "));
    }
    UpdateAction {
        executable,
        args,
        env,
        display,
        lock_key,
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let index = trimmed.rfind(['/', '\\'])?;
    Some(trimmed[..index].to_owned())
}

fn is_under(path: &str, directory: &str) -> bool {
    let directory = layout::normalize(directory);
    layout::normalize(path).starts_with(&format!("{}/", directory.trim_end_matches('/')))
}

fn command_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    for extension in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(stem) = name.strip_suffix(extension) {
            return stem.to_owned();
        }
    }
    name
}

// ---------------------------------------------------------------- mise

struct MiseInstall {
    install_path: String,
    requested_version: Option<String>,
    active: bool,
}

fn mise_installs(listing: &str, tool: &str) -> Vec<MiseInstall> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(listing) else {
        return vec![];
    };
    // `mise ls --json <tool>` prints an array; without a tool, an object keyed by tool.
    let entries = match &value {
        serde_json::Value::Array(entries) => entries.clone(),
        serde_json::Value::Object(map) => map
            .get(tool)
            .and_then(|entries| entries.as_array().cloned())
            .unwrap_or_default(),
        _ => vec![],
    };
    entries
        .iter()
        .filter_map(|entry| {
            Some(MiseInstall {
                install_path: entry.get("install_path")?.as_str()?.to_owned(),
                requested_version: entry
                    .get("requested_version")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                active: entry
                    .get("active")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
            })
        })
        .collect()
}

/// `None` when mise has nothing to do with this binary. Otherwise a plan:
/// `mise upgrade <tool>` only for an active install requested as `latest`
/// reached through a shim or the tool's `latest` link; every other mise
/// install stays manual so OriginLM never edits mise config.
fn resolve_mise(
    host: &dyn Host,
    tool: Option<(&str, bool)>,
    configured: &Path,
    real: &Path,
    npm_package: Option<&str>,
) -> Option<Plan> {
    let configured_text = text(configured);
    let real_text = text(real);
    let is_shim = layout::is_mise_executable(&real_text);
    let looks_mise = is_shim
        || layout::is_mise_path(&configured_text)
        || layout::is_mise_path(&real_text)
        || host
            .read_small(real, 16 * 1024)
            .is_some_and(|script| layout::is_mise_wrapper_script(&script));
    let latest = npm_package
        .map(|package| LatestSource::Npm {
            package: package.to_owned(),
            prefix: None,
        })
        .unwrap_or(LatestSource::None);
    let manual = |path: &str, reason: &str| {
        Some(
            Plan::manual(Path::new(path), latest.clone(), reason)
                .with_installer("mise", format!("{path} is a mise-managed path")),
        )
    };
    let not_owned = |path: &str| {
        if looks_mise {
            manual(
                path,
                "mise manages this path, but the install could not be proven; update it with mise by hand",
            )
        } else {
            None
        }
    };
    let Some((tool, may_upgrade)) = tool else {
        return not_owned(&real_text);
    };
    if is_shim && command_name(configured) != tool {
        return manual(&real_text, "this mise shim resolves a different tool");
    }
    let mise = host
        .which("mise")
        .or_else(|| is_shim.then(|| real.to_path_buf()));
    let Some(mise) = mise else {
        return not_owned(&real_text);
    };
    let which = host
        .run(
            &mise,
            &["which".into(), tool.into()],
            &BTreeMap::new(),
            PROBE_TIMEOUT,
        )
        .filter(|output| output.success)
        .map(|output| output.stdout.trim().to_owned())
        .filter(|line| !line.is_empty());
    let resolved = which.map(|path| text(&host.real_path(Path::new(&path))));
    let binary = if is_shim {
        resolved.clone()
    } else {
        Some(real_text.clone())
    };
    let Some(binary) = binary else {
        return manual(&real_text, "mise could not resolve this tool");
    };
    let runs_binary = resolved
        .as_deref()
        .is_some_and(|resolved| layout::normalize(resolved) == layout::normalize(&binary));
    let listing = host
        .run(
            &mise,
            &[
                "ls".into(),
                "--json".into(),
                "--installed".into(),
                tool.into(),
            ],
            &BTreeMap::new(),
            PROBE_TIMEOUT,
        )
        .filter(|output| output.success)
        .map(|output| output.stdout);
    let Some(listing) = listing else {
        return if runs_binary {
            manual(&binary, "mise did not list its installs")
        } else {
            not_owned(&binary)
        };
    };
    let owning: Vec<MiseInstall> = mise_installs(&listing, tool)
        .into_iter()
        .filter(|install| {
            let directory = text(&host.real_path(Path::new(&install.install_path)));
            is_under(&binary, &directory)
        })
        .collect();
    if owning.is_empty() {
        // `mise which` also resolves other tools' binaries, such as npm globals under mise's Node.
        return not_owned(&binary);
    }
    if !runs_binary {
        return manual(&binary, "mise resolves a different binary for this tool");
    }
    let Some(install) = owning
        .iter()
        .find(|install| install.active && install.requested_version.as_deref() == Some("latest"))
    else {
        return manual(
            &binary,
            "mise pins this tool to a version; change the request yourself (OriginLM never edits mise config)",
        );
    };
    if !may_upgrade {
        return manual(
            &binary,
            "mise's tool for this provider crosses a major generation; upgrade it by hand",
        );
    }
    if !is_shim {
        // A version directory keeps running the old binary; only the `latest` link moves.
        let install_dir = install.install_path.trim_end_matches(['/', '\\']);
        let Some(tool_dir) = parent(install_dir) else {
            return manual(&binary, "mise install layout is not recognized");
        };
        let real_tool_dir = text(&host.real_path(Path::new(&tool_dir)));
        let under_latest = [tool_dir, real_tool_dir]
            .iter()
            .any(|dir| is_under(&configured_text, &format!("{dir}/latest")));
        if !under_latest {
            return manual(
                &binary,
                "this path is a mise version directory, which an upgrade would not move",
            );
        }
    }
    let update = action(
        host,
        mise.clone(),
        &["upgrade", tool],
        BTreeMap::new(),
        format!("mise:{}:{tool}", layout::normalize(&text(&mise))),
        Some("mise"),
    );
    Some(Plan::installer(
        Path::new(&binary),
        "mise",
        format!(
            "`mise which {tool}` resolves {binary}, inside an active `{tool}@latest` install at {}",
            install.install_path
        ),
        update,
        LatestSource::Mise {
            mise,
            tool: tool.to_owned(),
        },
    ))
}

// ---------------------------------------------------------------- Homebrew

fn resolve_homebrew(host: &dyn Host, keg: &layout::Homebrew, real: &Path) -> Option<Plan> {
    let keg_brew = Path::new(&keg.prefix).join("bin").join("brew");
    let brew = if host.is_file(&keg_brew) {
        keg_brew
    } else {
        host.which("brew")?
    };
    let prefix = host
        .run(&brew, &["--prefix".into()], &BTreeMap::new(), PROBE_TIMEOUT)
        .filter(|output| output.success)
        .map(|output| output.stdout.trim().to_owned())
        .filter(|prefix| !prefix.is_empty())?;
    let real_prefix = text(&host.real_path(Path::new(&prefix)));
    if layout::normalize(&real_prefix) != layout::normalize(&keg.prefix) {
        return None;
    }
    let args: Vec<&str> = if keg.cask {
        vec!["upgrade", "--cask", &keg.name]
    } else {
        vec!["upgrade", &keg.name]
    };
    let update = action(
        host,
        brew.clone(),
        &args,
        BTreeMap::new(),
        format!("homebrew:{}", layout::normalize(&keg.prefix)),
        Some("brew"),
    );
    Some(Plan::installer(
        real,
        "homebrew",
        format!(
            "{} is inside the {} `{}` under `brew --prefix` {}",
            text(real),
            if keg.cask { "cask" } else { "formula" },
            keg.name,
            keg.prefix
        ),
        update,
        LatestSource::Homebrew {
            brew,
            name: keg.name.clone(),
            cask: keg.cask,
        },
    ))
}

// ---------------------------------------------------------------- package-managed providers

#[derive(Clone, Copy)]
enum NativeLatest {
    Npm,
    ClaudeChannel,
    CodexChannel,
}

#[derive(Clone, Copy)]
struct Native {
    label: &'static str,
    args: &'static [&'static str],
    is_path: Option<fn(&str) -> bool>,
    /// The updater cannot find installs outside `is_path`, so those stay manual.
    requires_path: bool,
    env: fn(configured: &str, real: &str) -> BTreeMap<String, String>,
    latest: NativeLatest,
}

struct PackageSpec {
    npm: &'static str,
    mise: Option<(&'static str, bool)>,
    native: Option<Native>,
}

fn no_env(_: &str, _: &str) -> BTreeMap<String, String> {
    BTreeMap::new()
}

fn codex_env(configured: &str, real: &str) -> BTreeMap<String, String> {
    // Standalone updates locate their install through CODEX_HOME.
    layout::codex_standalone_home(real)
        .or_else(|| layout::codex_standalone_home(configured))
        .map(|home| BTreeMap::from([("CODEX_HOME".to_owned(), home)]))
        .unwrap_or_default()
}

fn opencode_env(configured: &str, real: &str) -> BTreeMap<String, String> {
    // The curl installer writes to `$HOME/.opencode/bin`; point HOME at the install it replaces.
    layout::opencode_native_home(real)
        .or_else(|| layout::opencode_native_home(configured))
        .map(|home| BTreeMap::from([("HOME".to_owned(), home)]))
        .unwrap_or_default()
}

fn is_codex_standalone(path: &str) -> bool {
    layout::codex_standalone_home(path).is_some()
}

fn is_opencode_native(path: &str) -> bool {
    layout::opencode_native_home(path).is_some()
}

fn npm_latest(package: &str, prefix: Option<String>) -> LatestSource {
    LatestSource::Npm {
        package: package.to_owned(),
        prefix,
    }
}

fn resolve_package_managed(
    host: &dyn Host,
    provider_id: &str,
    spec: &PackageSpec,
    configured: &Path,
    real: &Path,
) -> Plan {
    let configured_text = text(configured);
    let real_text = text(real);
    let paths = [configured_text.as_str(), real_text.as_str()];
    let package = spec.npm;
    let windows = host.platform() == Platform::Windows;
    if provider_id == "claude_code" && windows && paths.iter().any(|p| layout::is_winget_claude(p))
    {
        return Plan::manual(
            real,
            npm_latest(package, None),
            "WinGet installed this Claude Code; `claude update` exits 0 without updating it and OriginLM does not run winget",
        )
        .with_installer("winget", format!("{real_text} is in WinGet's package layout"))
        .with_manual_command("winget upgrade Anthropic.ClaudeCode");
    }
    if provider_id == "claude_code"
        && windows
        && paths
            .iter()
            .any(|p| layout::is_chocolatey_claude(p, host.env("ChocolateyInstall").as_deref()))
    {
        return Plan::manual(
            real,
            LatestSource::Chocolatey {
                package: "claude-code".into(),
            },
            "Chocolatey installed this Claude Code: its package directory is writable only by administrators, and `claude update` through the shim would install a second copy elsewhere",
        )
        .with_installer("chocolatey", format!("{real_text} is in Chocolatey's claude-code layout"))
        .with_manual_command("choco upgrade claude-code");
    }
    if let Some(plan) = resolve_mise(host, spec.mise, configured, real, Some(package)) {
        return plan;
    }
    // After mise: a shim of a Nix-installed mise resolves into the store, yet its installs are writable.
    if paths.iter().any(|p| layout::is_nix_store(p)) {
        return Plan::manual(
            real,
            npm_latest(package, None),
            "installed in the read-only Nix store; update it through your Nix configuration",
        )
        .with_installer("nix", format!("{real_text} is under /nix/store"));
    }
    let manual_unproven = || {
        Plan::manual(
            real,
            npm_latest(package, None),
            "no installer is proven to own this binary; update it with the tool that installed it",
        )
    };
    let native_plan = |native: &Native| {
        let env = (native.env)(&configured_text, &real_text);
        let update = action(
            host,
            configured.to_path_buf(),
            native.args,
            env,
            format!("{provider_id}-native:{}", layout::normalize(&real_text)),
            None,
        );
        let latest = match native.latest {
            NativeLatest::Npm => npm_latest(package, None),
            NativeLatest::ClaudeChannel => LatestSource::ClaudeChannel,
            NativeLatest::CodexChannel => LatestSource::CodexChannel,
        };
        let proof = if native.is_path.is_some_and(|is| paths.iter().any(|p| is(p))) {
            format!(
                "{configured_text} is in {}'s native install layout",
                native.label
            )
        } else {
            format!(
                "{} detects its own installer; no package manager owns {real_text}",
                native.label
            )
        };
        Plan::installer(
            real,
            &format!("{provider_id}-native"),
            proof,
            update,
            latest,
        )
    };
    let npm_prefix_from_real = layout::npm_global_prefix(&real_text, package);
    if let Some(native) = &spec.native
        && native.is_path.is_some_and(|is| paths.iter().any(|p| is(p)))
        && npm_prefix_from_real.is_none()
    {
        return native_plan(native);
    }
    // A `node_modules` path not proven below belongs to another package or a project.
    let fallback = || match &spec.native {
        Some(native)
            if !native.requires_path
                && !paths
                    .iter()
                    .any(|p| layout::normalize(p).contains("/node_modules/")) =>
        {
            native_plan(native)
        }
        _ => manual_unproven(),
    };
    let global = |manager: &str, args: Vec<String>, lock: &str, proof: String| {
        let Some(executable) = host.which(manager) else {
            return Plan::manual(
                real,
                npm_latest(package, None),
                format!("{manager} owns this install but is not on PATH"),
            )
            .with_installer(lock, proof);
        };
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let update = action(
            host,
            executable,
            &borrowed,
            BTreeMap::new(),
            lock.to_owned(),
            Some(manager),
        );
        Plan::installer(real, lock, proof, update, npm_latest(package, None))
    };
    let latest_spec = format!("{package}@latest");
    if paths.iter().any(|p| layout::is_vite_plus_global(p)) {
        return global(
            "vp",
            vec!["i".into(), "-g".into(), package.into()],
            "vite-plus-global",
            format!("{configured_text} is in the Vite+ global bin directory"),
        );
    }
    if paths.iter().any(|p| layout::is_bun_global(p)) {
        return global(
            "bun",
            vec!["i".into(), "-g".into(), latest_spec],
            "bun-global",
            format!("{configured_text} is in Bun's global bin directory"),
        );
    }
    if paths.iter().any(|p| layout::is_pnpm_global(p)) {
        // pnpm 10 skips unapproved build scripts; the approval flag requires 10.4+.
        let allow_build = host
            .which("pnpm")
            .and_then(|pnpm| {
                host.run(
                    &pnpm,
                    &["--version".into()],
                    &BTreeMap::new(),
                    PROBE_TIMEOUT,
                )
            })
            .and_then(|output| super::version::extract(&output.stdout))
            .and_then(|version| semver::Version::parse(&version).ok())
            .is_some_and(|version| version >= semver::Version::new(10, 4, 0));
        let mut args = vec!["add".to_owned(), "-g".to_owned()];
        if allow_build {
            args.push(format!("--allow-build={package}"));
        }
        args.push(latest_spec);
        let mut plan = global(
            "pnpm",
            args,
            "pnpm-global",
            format!("{configured_text} is in pnpm's global directory"),
        );
        plan.pin_latest = plan.update.is_some();
        return plan;
    }
    if paths.iter().any(|p| layout::is_yarn_global(p)) {
        return global(
            "yarn",
            vec!["global".into(), "add".into(), latest_spec],
            "yarn-global",
            format!("{real_text} is in Yarn's global node_modules"),
        );
    }
    if layout::is_volta_shim(&real_text)
        && let Some(volta_home) = configured.parent().and_then(Path::parent)
    {
        let image = package
            .split('/')
            .fold(volta_home.join("tools/image/packages"), |dir, part| {
                dir.join(part)
            });
        if host.is_dir(&image) {
            return global(
                "volta",
                vec!["install".into(), latest_spec],
                "volta",
                format!(
                    "{configured_text} links to volta-shim and Volta holds the {package} image at {}",
                    text(&image)
                ),
            );
        }
    }
    // npm proof outranks a containing Homebrew keg; Homebrew Node globals still belong to npm.
    let npm_prefix = npm_prefix_from_real.map(|prefix| {
        (
            prefix,
            format!("{real_text} is under <prefix>/lib/node_modules/{package}/"),
        )
    });
    let npm_prefix = npm_prefix.or_else(|| {
        if !windows {
            return None;
        }
        // Windows npm shims sit beside `node_modules`.
        let shim_dir = configured.parent()?;
        let manifest = package
            .split('/')
            .fold(shim_dir.join("node_modules"), |dir, part| dir.join(part))
            .join("package.json");
        host.is_file(&manifest).then(|| {
            (
                text(shim_dir),
                format!(
                    "{configured_text} is an npm shim beside {}",
                    text(&manifest)
                ),
            )
        })
    });
    if let Some((prefix, proof)) = npm_prefix {
        let Some(npm) = host.which("npm") else {
            return Plan::manual(
                real,
                npm_latest(package, Some(prefix)),
                "npm owns this install but is not on PATH",
            )
            .with_installer("npm-global", proof);
        };
        // `--prefix` is pinned: the npm on PATH can belong to a different Node.
        let update = action(
            host,
            npm,
            &[
                "install",
                "-g",
                "--prefix",
                &prefix,
                &format!("--allow-scripts={package}"),
                &format!("{package}@latest"),
            ],
            BTreeMap::new(),
            format!("npm-global:{}", layout::normalize(&prefix)),
            Some("npm"),
        );
        return Plan::installer(
            real,
            "npm-global",
            proof,
            update,
            npm_latest(package, Some(prefix)),
        );
    }
    if let Some(keg) = layout::homebrew_keg(&real_text) {
        return resolve_homebrew(host, &keg, real).unwrap_or_else(fallback);
    }
    fallback()
}

fn claude_spec() -> PackageSpec {
    PackageSpec {
        npm: "@anthropic-ai/claude-code",
        mise: Some(("claude", true)),
        native: Some(Native {
            label: "Claude Code",
            args: &["update"],
            is_path: Some(layout::is_claude_native),
            requires_path: true,
            env: no_env,
            latest: NativeLatest::ClaudeChannel,
        }),
    }
}

fn codex_spec() -> PackageSpec {
    PackageSpec {
        npm: "@openai/codex",
        mise: Some(("codex", true)),
        native: Some(Native {
            label: "Codex standalone",
            args: &["update"],
            is_path: Some(is_codex_standalone),
            requires_path: true,
            env: codex_env,
            latest: NativeLatest::CodexChannel,
        }),
    }
}

fn opencode_spec(v2: bool) -> PackageSpec {
    PackageSpec {
        // Update only within the installed generation; 2.x converts the shared database in place.
        npm: if v2 { "@opencode/cli" } else { "opencode-ai" },
        // mise's `opencode` follows releases into 2.x, so a 1.x install is never upgraded by it.
        mise: Some(("opencode", v2)),
        native: Some(Native {
            label: "OpenCode's curl installer",
            args: &["upgrade", "--method", "curl"],
            is_path: Some(is_opencode_native),
            requires_path: true,
            env: opencode_env,
            latest: NativeLatest::Npm,
        }),
    }
}

fn pi_spec() -> PackageSpec {
    PackageSpec {
        npm: "@earendil-works/pi-coding-agent",
        mise: Some(("pi", true)),
        native: Some(Native {
            label: "Pi's own updater",
            args: &["update", "--self"],
            is_path: None,
            requires_path: false,
            env: no_env,
            latest: NativeLatest::Npm,
        }),
    }
}

fn gemini_spec() -> PackageSpec {
    PackageSpec {
        npm: "@google/gemini-cli",
        mise: None,
        native: None,
    }
}

const GROK_PACKAGE: &str = "@xai-official/grok";

fn resolve_grok(host: &dyn Host, configured: &Path, real: &Path) -> Plan {
    if let Some(plan) = resolve_mise(
        host,
        Some(("grok", true)),
        configured,
        real,
        Some(GROK_PACKAGE),
    ) {
        return plan;
    }
    let real_text = text(real);
    if layout::is_nix_store(&text(configured)) || layout::is_nix_store(&real_text) {
        return Plan::manual(
            real,
            npm_latest(GROK_PACKAGE, None),
            "installed in the read-only Nix store; update it through your Nix configuration",
        )
        .with_installer("nix", format!("{real_text} is under /nix/store"));
    }
    // Only a cask is Grok's: a formula keg such as `node` holding an npm global is not.
    if let Some(keg) = layout::homebrew_keg(&real_text).filter(|keg| keg.cask) {
        return resolve_homebrew(host, &keg, real).unwrap_or_else(
            || {
                Plan::manual(
                    real,
                    npm_latest(GROK_PACKAGE, None),
                    "a Homebrew cask whose prefix `brew --prefix` cannot verify; `grok update` would install a second copy",
                )
                .with_manual_command(format!("brew upgrade --cask {}", keg.name))
            },
        );
    }
    // npm 12 blocks the postinstall that moves the new binary in, yet exits 0.
    let env = BTreeMap::from([(
        "npm_config_allow_scripts".to_owned(),
        GROK_PACKAGE.to_owned(),
    )]);
    let update = action(
        host,
        configured.to_path_buf(),
        &["update"],
        env,
        format!("grok:{}", layout::normalize(&real_text)),
        None,
    );
    Plan::installer(
        real,
        "grok-native",
        format!("no package manager owns {real_text}; `grok update` detects its own installer"),
        update,
        npm_latest(GROK_PACKAGE, None),
    )
}

fn resolve_muse(host: &dyn Host, configured: &Path, real: &Path) -> Plan {
    if host.platform() == Platform::Windows {
        return Plan::manual(
            real,
            LatestSource::MuseChannel,
            "the Windows Muse Code install has no proven update path; update it with the Muse Code installer",
        );
    }
    if let Some(mut plan) = resolve_mise(host, Some(("muse", true)), configured, real, None) {
        if plan.latest == LatestSource::None {
            plan.latest = LatestSource::MuseChannel;
        }
        return plan;
    }
    let is_launcher = host.read_small(real, 128 * 1024).is_some_and(|source| {
        source.contains("muse-code/launcher-") && source.contains("MUSE_SYNC_UPDATE")
    });
    if !is_launcher {
        return Plan::manual(
            real,
            LatestSource::MuseChannel,
            "a standalone Muse Code binary cannot update itself; only the official launcher has an update channel",
        );
    }
    // The launcher has no `update` subcommand; its documented environment forces
    // a synchronous refresh of this executable's own installation.
    let mut env = BTreeMap::from([
        ("MUSE_NO_AUTO_UPDATE".to_owned(), "0".to_owned()),
        ("MUSE_SYNC_UPDATE".to_owned(), "1".to_owned()),
        ("MUSE_UPDATE_INTERVAL_SECONDS".to_owned(), "0".to_owned()),
    ]);
    if let Some(channel) = host.env("MUSE_CHANNEL") {
        env.insert("MUSE_CHANNEL".into(), channel);
    }
    let real_text = text(real);
    let update = action(
        host,
        configured.to_path_buf(),
        &["--version"],
        env,
        format!("muse:{}", layout::normalize(&real_text)),
        None,
    );
    Plan::installer(
        real,
        "muse-launcher",
        format!("{real_text} is the official Muse Code launcher"),
        update,
        LatestSource::MuseChannel,
    )
}

/// Resolve the owning installer for one install. `installed_version` selects
/// the OpenCode package generation.
pub fn resolve(host: &dyn Host, install: &Install, installed_version: Option<&str>) -> Plan {
    let configured = install.executable.as_path();
    let real = host.real_path(configured);
    let real_text = text(&real);
    let configured_text = text(configured);
    let t3_managed = layout::is_t3_managed(&configured_text) || layout::is_t3_managed(&real_text);
    match install.provider_id.as_str() {
        "claude_code" => resolve_package_managed(host, "claude_code", &claude_spec(), configured, &real),
        "codex" => resolve_package_managed(host, "codex", &codex_spec(), configured, &real),
        "opencode" => {
            if layout::is_opencode_desktop_bundle(&configured_text)
                || layout::is_opencode_desktop_bundle(&real_text)
            {
                return Plan::manual(
                    &real,
                    LatestSource::None,
                    "bundled with the OpenCode desktop app, which updates it; OriginLM does not modify app bundles",
                );
            }
            let Some(version) = installed_version else {
                return Plan::manual(
                    &real,
                    LatestSource::None,
                    "the OpenCode version could not be read, so its package generation (1.x or 2.x) is unknown",
                );
            };
            let v2 = version
                .split('.')
                .next()
                .and_then(|major| major.parse::<u64>().ok())
                .is_some_and(|major| major >= 2);
            resolve_package_managed(host, "opencode", &opencode_spec(v2), configured, &real)
        }
        "pi" => resolve_package_managed(host, "pi", &pi_spec(), configured, &real),
        "gemini_cli" if t3_managed => Plan::manual(
            &real,
            LatestSource::None,
            "installed and updated by T3 Code under ~/.t3/tools; OriginLM does not modify T3-managed runtimes",
        ),
        "gemini_cli" => resolve_package_managed(host, "gemini_cli", &gemini_spec(), configured, &real),
        "grok" => resolve_grok(host, configured, &real),
        "muse" => resolve_muse(host, configured, &real),
        "antigravity" if t3_managed => Plan::manual(
            &real,
            LatestSource::None,
            "installed and updated by T3 Code under ~/.t3/tools; OriginLM does not modify T3-managed runtimes",
        )
        .with_installer("t3code", format!("{real_text} is under T3 Code's managed tools directory")),
        "antigravity" => Plan::manual(
            &real,
            LatestSource::None,
            "Antigravity is updated by the Antigravity app; OriginLM has no update path or release channel for it",
        ),
        "cursor" => Plan::manual(
            &real,
            LatestSource::None,
            "Cursor's agent CLI has no installer OriginLM can prove or a release channel it can read; update it with Cursor's installer",
        ),
        "kiro" => Plan::manual(
            &real,
            LatestSource::None,
            "Kiro updates itself in the background (`kiro-cli update` only reports), so OriginLM offers no update",
        ),
        "fx" => Plan::manual(
            &real,
            LatestSource::None,
            "fx installs from its own setup script and has no package to read a latest version from",
        )
        .with_manual_command("fx upgrade"),
        "ollama" | "lmstudio" => Plan::unsupported(
            &real,
            "a local model runtime updated by its own desktop app; OriginLM does not update it",
        ),
        _ => Plan::unsupported(&real, "OriginLM has no update path for this provider"),
    }
}

// ---------------------------------------------------------------- latest versions

/// The latest version and a description of where it came from.
pub fn latest_version(host: &dyn Host, source: &LatestSource) -> (Option<String>, Option<String>) {
    let run = |program: &Path, args: &[&str]| {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        host.run(program, &args, &BTreeMap::new(), PROBE_TIMEOUT)
            .filter(|output| output.success)
            .map(|output| output.stdout)
    };
    match source {
        LatestSource::None => (None, None),
        LatestSource::Npm { package, prefix } => {
            let description =
                format!("npm view {package}@latest version (npm's configured registry)");
            let Some(npm) = host.which("npm") else {
                return (None, Some(format!("{description}: npm is not on PATH")));
            };
            let mut args = vec!["view", "--global"];
            if let Some(prefix) = prefix {
                args.extend(["--prefix", prefix.as_str()]);
            }
            let spec = format!("{package}@latest");
            args.extend(["--json", spec.as_str(), "version"]);
            let version = run(&npm, &args).and_then(|stdout| {
                // npm prints a JSON string, or an array when several versions match.
                match serde_json::from_str::<serde_json::Value>(stdout.trim()).ok()? {
                    serde_json::Value::String(version) => Some(version),
                    serde_json::Value::Array(values) => values.last()?.as_str().map(str::to_owned),
                    _ => None,
                }
            });
            (
                version.and_then(|version| super::version::extract(&version)),
                Some(description),
            )
        }
        LatestSource::Mise { mise, tool } => {
            // `minimum_release_age` can hold mise behind npm: only mise knows what it installs.
            let version = run(mise, &["latest", tool]).and_then(|stdout| {
                let line = stdout.trim();
                (!line.is_empty() && !line.contains(char::is_whitespace)).then(|| line.to_owned())
            });
            (version, Some(format!("mise latest {tool}")))
        }
        LatestSource::Homebrew { brew, name, cask } => {
            let version = run(brew, &["info", "--json=v2", name]).and_then(|stdout| {
                let info: serde_json::Value = serde_json::from_str(&stdout).ok()?;
                if *cask {
                    let version = info["casks"].as_array()?.first()?["version"].as_str()?;
                    // Cask versions can carry a `,build` suffix.
                    Some(version.split(',').next()?.to_owned())
                } else {
                    info["formulae"].as_array()?.first()?["versions"]["stable"]
                        .as_str()
                        .map(str::to_owned)
                }
            });
            (
                version,
                Some(format!(
                    "brew info --json=v2 {name} (Homebrew can trail npm by hours)"
                )),
            )
        }
        LatestSource::Chocolatey { package } => {
            let version = host.which("choco").and_then(|choco| {
                run(&choco, &["search", package, "--exact", "--limit-output"])
                    .and_then(|stdout| layout::chocolatey_latest(&stdout, package))
            });
            (
                version,
                Some(format!("choco search {package} --exact --limit-output")),
            )
        }
        LatestSource::ClaudeChannel => {
            let channel = claude_update_channel(host);
            let url = format!("https://downloads.claude.ai/claude-code-releases/{channel}");
            let version = host.http_get(&url).and_then(|body| {
                let body = body.trim();
                let parts: Vec<&str> = body.split('.').collect();
                (parts.len() == 3
                    && parts
                        .iter()
                        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())))
                .then(|| body.to_owned())
            });
            (
                version,
                Some(format!("{url} (the channel `claude update` installs)")),
            )
        }
        LatestSource::CodexChannel => {
            let url = "https://releases.openai.com/codex/channels/latest";
            let version = host.http_get(url).and_then(|body| {
                let value: serde_json::Value = serde_json::from_str(&body).ok()?;
                let tag = value["tag_name"].as_str()?;
                let version = tag.strip_prefix("rust-v")?;
                super::version::extract(version).filter(|parsed| parsed == version)
            });
            (
                version,
                Some(format!("{url} (the channel `codex update` installs)")),
            )
        }
        LatestSource::MuseChannel => {
            let channel = host
                .env("MUSE_CHANNEL")
                .unwrap_or_else(|| "muse-stable".into());
            if !matches!(channel.as_str(), "muse-stable" | "muse-canary") {
                return (
                    None,
                    Some(format!(
                        "MUSE_CHANNEL={channel} is not a known Muse Code channel"
                    )),
                );
            }
            let url = format!("https://api.meta.ai/muse-code/channels/{channel}");
            let version = host.http_get(&url).and_then(|body| {
                let value: serde_json::Value = serde_json::from_str(&body).ok()?;
                super::version::extract(value["version"].as_str()?)
            });
            (version, Some(url))
        }
    }
}

/// Claude Code's `autoUpdatesChannel` (`latest` or `stable`); managed settings win.
fn claude_update_channel(host: &dyn Host) -> String {
    let platform = host.platform();
    let managed = match platform {
        Platform::Macos => PathBuf::from("/Library/Application Support/ClaudeCode"),
        Platform::Windows => PathBuf::from(
            host.env("ProgramFiles")
                .unwrap_or_else(|| r"C:\Program Files".into()),
        )
        .join("ClaudeCode"),
        Platform::Linux => PathBuf::from("/etc/claude-code"),
    };
    let config = host
        .env("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            host.env(if platform == Platform::Windows {
                "USERPROFILE"
            } else {
                "HOME"
            })
            .map(|home| PathBuf::from(home).join(".claude"))
        });
    let mut files: Vec<PathBuf> = config
        .map(|config| config.join("settings.json"))
        .into_iter()
        .collect();
    files.push(managed.join("managed-settings.json"));
    let mut channel = "latest".to_owned();
    for file in files {
        if let Some(value) = host
            .read_small(&file, 1024 * 1024)
            .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
            .and_then(|settings| settings["autoUpdatesChannel"].as_str().map(str::to_owned))
            .filter(|value| matches!(value.as_str(), "latest" | "stable"))
        {
            channel = value;
        }
    }
    channel
}
