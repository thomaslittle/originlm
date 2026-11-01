use originlm_contracts::{DomainError, ErrorCode, Result};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};
/// Client-home variables the passive scan will honor. Adding a key here is a
/// product decision; unknown variables are ignored so a future upstream client
/// cannot redirect discovery until OriginLM adopts the rule.
pub const DISCOVERY_OVERRIDE_KEYS: &[&str] = &[
    "CODEX_HOME",
    "CLAUDE_SECURESTORAGE_CONFIG_DIR",
    "CLAUDE_CONFIG_DIR",
    "OPENCODE_CONFIG_DIR",
    "GEMINI_CLI_HOME",
    "GROK_HOME",
    "LM_STUDIO_HOME",
    "PI_CODING_AGENT_DIR",
    crate::MUSE_BINARY_OVERRIDE,
];
const LOCATION_KEYS: &[&str] = &[
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "APPDATA",
    "LOCALAPPDATA",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    Windows,
    Macos,
    Linux,
}
#[derive(Debug, Clone)]
pub struct ScanEnvironment {
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub data_home: PathBuf,
    pub app_data: PathBuf,
    pub local_app_data: PathBuf,
    pub search_path: Vec<PathBuf>,
    pub overrides: BTreeMap<String, PathBuf>,
    /// Host-owned exact executable per provider, used instead of directory
    /// search. This preserves a logical command/symlink and its resolved
    /// identity without widening the global `PATH`.
    pub executable_overrides: BTreeMap<String, PathBuf>,
    pub platform: HostPlatform,
}
impl ScanEnvironment {
    pub fn for_home(home: PathBuf, platform: HostPlatform) -> Self {
        Self {
            config_home: join_relative(&home, ".config"),
            data_home: join_relative(&home, ".local/share"),
            app_data: join_relative(&home, "AppData/Roaming"),
            local_app_data: join_relative(&home, "AppData/Local"),
            home,
            search_path: vec![],
            overrides: BTreeMap::new(),
            executable_overrides: BTreeMap::new(),
            platform,
        }
    }
    /// Only called by an explicit scan command. This does not read provider credential variables.
    pub fn from_process() -> Result<Self> {
        let platform = if cfg!(windows) {
            HostPlatform::Windows
        } else if cfg!(target_os = "macos") {
            HostPlatform::Macos
        } else {
            HostPlatform::Linux
        };
        let mut vars = BTreeMap::new();
        for key in ["HOME", "USERPROFILE", "PATH"]
            .into_iter()
            .chain(LOCATION_KEYS.iter().copied())
            .chain(DISCOVERY_OVERRIDE_KEYS.iter().copied())
        {
            if let Some(value) = std::env::var_os(key).and_then(|value| value.into_string().ok()) {
                vars.insert(key.to_string(), value);
            }
        }
        Self::from_map(platform, &vars)
    }
    /// Build a scan environment from an explicit variable map.
    ///
    /// Blank and whitespace-only values are unset. On Windows an absolute
    /// native `HOME` redirects the profile; POSIX-shaped and drive-relative
    /// values do not, because `Path` would resolve them against ambient state.
    /// Network and relative roots are still rejected by [`Self::validate`].
    pub fn from_map(platform: HostPlatform, vars: &BTreeMap<String, String>) -> Result<Self> {
        let home = home_from_vars(platform, vars)?;
        let mut env = Self::for_home(home, platform);
        for (key, destination) in [
            ("XDG_CONFIG_HOME", &mut env.config_home),
            ("XDG_DATA_HOME", &mut env.data_home),
            ("APPDATA", &mut env.app_data),
            ("LOCALAPPDATA", &mut env.local_app_data),
        ] {
            if let Some(value) = present(vars, key) {
                *destination = expand(&env.home, PathBuf::from(value));
            }
        }
        if let Some(path) = vars.get("PATH").filter(|value| !value.is_empty()) {
            env.search_path = std::env::split_paths(path)
                .take(128)
                .filter(|p| valid_root(p))
                .collect();
        }
        for key in DISCOVERY_OVERRIDE_KEYS {
            if let Some(value) = present(vars, key) {
                env.overrides
                    .insert((*key).to_string(), expand(&env.home, PathBuf::from(value)));
            }
        }
        env.validate()?;
        Ok(env)
    }
    pub fn validate(&self) -> Result<()> {
        if [
            &self.home,
            &self.config_home,
            &self.data_home,
            &self.app_data,
            &self.local_app_data,
        ]
        .into_iter()
        .chain(self.overrides.values())
        .chain(self.executable_overrides.values())
        .any(|p| !valid_root(p))
        {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "Scan roots must be absolute local paths",
            ));
        }
        if self.overrides.len() > 32
            || self.search_path.len() > 128
            || self.executable_overrides.len() > 32
        {
            return Err(DomainError::new(
                ErrorCode::InvalidRequest,
                "Scan environment exceeds configured bounds",
            ));
        }
        Ok(())
    }
}
fn home_from_vars(platform: HostPlatform, vars: &BTreeMap<String, String>) -> Result<PathBuf> {
    if platform == HostPlatform::Windows {
        if let Some(raw) = vars.get("HOME").map(String::as_str)
            && !raw.is_empty()
        {
            let path = PathBuf::from(raw);
            // `is_absolute` rejects Git Bash `/home/user` and drive-relative
            // `C:temp` on Windows. Both would otherwise follow the process
            // cwd or the current drive's cwd.
            if path.is_absolute() {
                return Ok(path);
            }
        }
        return present(vars, "USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| {
                DomainError::new(
                    ErrorCode::LocalAccessDenied,
                    "Current user home is unavailable",
                )
            });
    }
    present(vars, "HOME").map(PathBuf::from).ok_or_else(|| {
        DomainError::new(
            ErrorCode::LocalAccessDenied,
            "Current user home is unavailable",
        )
    })
}
fn present<'a>(vars: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    vars.get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
}
/// Join a `/`-separated relative literal one component at a time.
///
/// `Path::join` only normalizes the junction, so `"AppData/Roaming"` and
/// `".codex/sessions"` keep their `/` characters on Windows. Pushing each
/// normal component stores native separators instead.
pub fn join_relative(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    if relative.is_empty() {
        return path;
    }
    for component in Path::new(relative).components() {
        if let Component::Normal(part) = component {
            path.push(part);
        }
    }
    path
}
fn expand(home: &Path, path: PathBuf) -> PathBuf {
    if path == Path::new("~") {
        home.into()
    } else if let Ok(tail) = path.strip_prefix("~") {
        home.join(tail)
    } else {
        path
    }
}
pub(crate) fn valid_root(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    path.is_absolute()
        && !text.starts_with("//")
        && !text.starts_with(r"\\")
        && !text.chars().any(char::is_control)
}
