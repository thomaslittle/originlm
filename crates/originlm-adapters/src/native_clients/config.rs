use originlm_contracts::*;
use originlm_discovery::{LocatedClient, ScanResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeProvider {
    GeminiCli,
    Antigravity,
    Grok,
}
impl NativeProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::GeminiCli => "gemini_cli",
            Self::Antigravity => "antigravity",
            Self::Grok => "grok",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "gemini_cli" => Some(Self::GeminiCli),
            "antigravity" => Some(Self::Antigravity),
            "grok" => Some(Self::Grok),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeProfile {
    pub id: String,
    pub label: String,
    pub provider: NativeProvider,
    pub executable: PathBuf,
    pub profile_home: PathBuf,
    #[serde(default)]
    pub auth_method: Option<String>,
    /// Explicit host/user declaration, not a provider-verified plan or automatic fallback.
    #[serde(default)]
    pub billing: Option<BillingMode>,
}
#[derive(Debug, Clone, Default)]
pub struct NativeRegistry {
    pub profiles: BTreeMap<String, NativeProfile>,
    pub browser_blocker: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    profiles: Vec<NativeProfile>,
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Invalid native client profile; credentials and arbitrary arguments are not accepted",
    )
}
pub(crate) fn local_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    path.is_absolute()
        && !text.starts_with("//")
        && !text.starts_with(r"\")
        && !text.chars().any(char::is_control)
}

/// Add one host-managed ChatGPT-plan Codex route from a discovered executable.
/// This is passive: no profile files, credentials, or accounts are read. The
/// managed CODEX_HOME is separate from every discovered user's Codex home.
pub fn inject_managed_codex(scan: &mut ScanResult, state_dir: &Path) {
    if !local_path(state_dir)
        || scan
            .snapshot
            .connections
            .iter()
            .any(|c| c.provider_id == "codex_managed")
    {
        return;
    }
    let executable = scan
        .locations
        .values()
        .filter(|located| located.provider_id == "codex")
        .filter_map(|located| {
            let command = located.executable.as_ref()?;
            let resolved = crate::codex::resolve_executable(command).ok()?;
            // A versioned Unix target is not itself a command named `codex`.
            let executable = if located.executable_identity.is_some() {
                command.clone()
            } else {
                resolved
            };
            Some((executable, located.executable_identity.clone()))
        })
        .min_by(|(a, _), (b, _)| a.cmp(b));
    let Some((executable, executable_identity)) = executable else {
        return;
    };
    let home = state_dir.join("chatgpt-codex-home");
    let executable_key = executable_identity
        .as_ref()
        .map(|identity| format!("{identity:?}"))
        .unwrap_or_else(|| executable.to_string_lossy().into_owned());
    let connection = stable_id(
        "managed-codex-connection",
        &[&state_dir.to_string_lossy(), &executable_key],
    );
    let installation = stable_id("managed-codex-installation", &[&connection]);
    let profile = stable_id("managed-codex-profile", &[&connection]);
    let route = stable_id("route", &[&connection]);
    scan.snapshot.providers.push(Provider {
        id: "codex_managed".into(),
        name: "ChatGPT plan (Codex)".into(),
        kind: "managed_client".into(),
    });
    scan.snapshot.installations.push(Installation {
        id: installation.clone(),
        provider_id: "codex_managed".into(),
        environment_id: "host-managed-chatgpt".into(),
        location_hint: "Host-managed Codex executable; not launched".into(),
        version: Observation::unknown("VERSION_NOT_PROBED"),
    });
    scan.snapshot.profiles.push(Profile {
        id: profile.clone(),
        installation_id: installation.clone(),
        label: "Host-managed ChatGPT plan".into(),
    });
    scan.snapshot.connections.push(Connection {
        id: connection.clone(),
        provider_id: "codex_managed".into(),
        installation_id: installation,
        profile_id: profile,
        display_name: "ChatGPT plan (Codex)".into(),
        account_id: None,
        auth: Observation::unknown("AUTH_NOT_CHECKED_PASSIVE_SCAN"),
        plan: Observation::unknown("PLAN_NOT_REPORTED"),
        status: "configured".into(),
        diagnostics: vec!["HOST_MANAGED_CHATGPT_PROFILE".into()],
    });
    scan.snapshot.routes.push(AccessRoute {
        id: route,
        connection_id: connection.clone(),
        provider_id: "codex_managed".into(),
        display_name: "ChatGPT plan (Codex)".into(),
        billing: Observation::unknown("NOT_PROBED"),
        capabilities: vec![],
        catalog_revision: "unprobed".into(),
    });
    scan.locations.insert(
        connection,
        LocatedClient {
            executable_identity,
            provider_id: "codex_managed".into(),
            profile_home: home,
            executable: Some(executable),
            scope: None,
        },
    );
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
impl NativeRegistry {
    pub fn parse(text: &str, scope: &str) -> Result<Self> {
        if text.len() > 65536 || scope.is_empty() || scope.len() > 4096 {
            return Err(invalid());
        }
        let d: Document = serde_json::from_str(text).map_err(|_| invalid())?;
        if d.profiles.len() > 32 {
            return Err(invalid());
        }
        let mut profiles = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for p in d.profiles {
            if !identifier(&p.id)
                || !ids.insert(p.id.clone())
                || p.label.is_empty()
                || p.label.len() > 160
                || p.label.chars().any(char::is_control)
                || !local_path(&p.executable)
                || !local_path(&p.profile_home)
            {
                return Err(invalid());
            }
            if p.auth_method
                .as_ref()
                .is_some_and(|s| p.provider != NativeProvider::Antigravity || !identifier(s))
            {
                return Err(invalid());
            }
            if matches!(p.billing, Some(BillingMode::Local)) {
                return Err(invalid());
            }
            let key = stable_id(
                "native-connection",
                &[
                    scope,
                    &p.id,
                    p.provider.id(),
                    &p.executable.to_string_lossy(),
                    &p.profile_home.to_string_lossy(),
                ],
            );
            profiles.insert(key, p);
        }
        Ok(Self {
            profiles,
            browser_blocker: None,
        })
    }
    pub fn inject(&self, scan: &mut ScanResult) {
        for (id, p) in &self.profiles {
            let provider = p.provider.id();
            let install = stable_id("native-installation", &[id]);
            let profile = stable_id("native-profile", &[id]);
            if !scan.snapshot.providers.iter().any(|v| v.id == provider) {
                scan.snapshot.providers.push(Provider {
                    id: provider.into(),
                    name: provider.into(),
                    kind: "native_client".into(),
                });
            }
            scan.snapshot.installations.push(Installation {
                id: install.clone(),
                provider_id: provider.into(),
                environment_id: "explicit-native-registration".into(),
                location_hint: "Host-configured native client; not launched".into(),
                version: Observation::unknown("NOT_PROBED"),
            });
            scan.snapshot.profiles.push(Profile {
                id: profile.clone(),
                installation_id: install.clone(),
                label: p.label.clone(),
            });
            scan.snapshot.connections.push(Connection {
                id: id.clone(),
                provider_id: provider.into(),
                installation_id: install,
                profile_id: profile,
                display_name: p.label.clone(),
                account_id: None,
                auth: Observation::unknown("NOT_PROBED"),
                plan: Observation::unknown("NOT_REPORTED"),
                status: "configured".into(),
                diagnostics: vec!["EXPLICIT_NATIVE_PROFILE".into()],
            });
            scan.snapshot.routes.push(AccessRoute {
                id: stable_id("route", &[id]),
                connection_id: id.clone(),
                provider_id: provider.into(),
                display_name: p.label.clone(),
                billing: Observation::unknown("NOT_PROBED"),
                capabilities: vec![],
                catalog_revision: "unprobed".into(),
            });
            scan.locations.insert(
                id.clone(),
                LocatedClient {
                    executable_identity: None,
                    provider_id: provider.into(),
                    profile_home: p.profile_home.clone(),
                    executable: Some(p.executable.clone()),
                    scope: None,
                },
            );
        }
    }
    pub fn profile(&self, connection: &str, located: &LocatedClient) -> Result<NativeProfile> {
        if let Some(profile) = self.profiles.get(connection) {
            if profile.provider.id() != located.provider_id
                || profile.profile_home != located.profile_home
                || Some(&profile.executable) != located.executable.as_ref()
            {
                return Err(invalid());
            }
            return Ok(profile.clone());
        }
        let provider = NativeProvider::from_id(&located.provider_id).ok_or_else(invalid)?;
        let executable = located.executable.clone().ok_or_else(|| {
            DomainError::new(
                ErrorCode::RuntimeMissing,
                "Compatible native executable is unavailable; configure a verified client path",
            )
        })?;
        Ok(NativeProfile {
            id: connection.into(),
            label: provider.id().into(),
            provider,
            executable,
            profile_home: located.profile_home.clone(),
            auth_method: None,
            billing: None,
        })
    }
}
