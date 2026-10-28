//! Normalized discovery rules. Tokscale and other upstream projects can confirm
//! a path, but they do not outrank an official client or a OriginLM
//! qualification. Nothing in this module reads the network or executes a client.
use crate::{HostPlatform, ScanEnvironment, join_relative};
use originlm_contracts::{EvidenceKind, EvidenceSource};
use std::path::{Path, PathBuf};

/// Audited Tokscale commit. Released builds do not fetch this revision.
pub const TOKSCALE_REPOSITORY: &str = "junhoyeo/tokscale";
pub const TOKSCALE_COMMIT: &str = "1d9a9395418efc6952944b794097935d7d6fa1e8";
/// Audited T3 Code commit. Released builds do not fetch this revision.
pub const T3CODE_REPOSITORY: &str = "pingdotgg/t3code";
pub const T3CODE_COMMIT: &str = "e9236274a3a6e0c731a7df4092db1e686f039068";

/// Why a discovery rule is trusted. Higher ranks win when sources disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleBasis {
    OfficialClient,
    LocalQualification,
    UpstreamOss,
    Convention,
}
impl RuleBasis {
    pub fn rank(self) -> u8 {
        match self {
            Self::OfficialClient => 4,
            Self::LocalQualification => 3,
            Self::UpstreamOss => 2,
            Self::Convention => 1,
        }
    }
    pub fn preferred(bases: &[Self]) -> Option<Self> {
        bases.iter().copied().max_by_key(|basis| basis.rank())
    }
}

/// What a rule locates. A legacy fallback can still carry upstream evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleRole {
    EnvOverride,
    ProfileHome,
    LegacyFallback,
    SessionStore,
    ExcludedUpstream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRule {
    pub id: &'static str,
    pub provider_id: &'static str,
    pub role: RuleRole,
    pub basis: RuleBasis,
    pub summary: &'static str,
    pub evidence: EvidenceSource,
}

fn tokscale(provider: &str, detail: &str) -> EvidenceSource {
    EvidenceSource::new(
        EvidenceKind::Derived,
        "upstream.tokscale",
        &format!("{TOKSCALE_REPOSITORY}@{TOKSCALE_COMMIT} {detail} ({provider})"),
    )
}
fn t3code(detail: &str) -> EvidenceSource {
    EvidenceSource::new(
        EvidenceKind::Derived,
        "upstream.t3code",
        &format!("{T3CODE_REPOSITORY}@{T3CODE_COMMIT} {detail}"),
    )
}
fn local(provider: &str, detail: &str) -> EvidenceSource {
    EvidenceSource::new(EvidenceKind::LocalConfig, provider, detail)
}
fn official(provider: &str, detail: &str) -> EvidenceSource {
    EvidenceSource::new(EvidenceKind::OfficialProtocol, provider, detail)
}

pub fn discovery_rules() -> Vec<DiscoveryRule> {
    vec![
        DiscoveryRule {
            id: "codex-home",
            provider_id: "codex",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::OfficialClient,
            summary: "CODEX_HOME, otherwise ~/.codex",
            evidence: official(
                "codex",
                "https://developers.openai.com/codex/app-server/ — CODEX_HOME",
            ),
        },
        DiscoveryRule {
            id: "codex-sessions",
            provider_id: "codex",
            role: RuleRole::SessionStore,
            basis: RuleBasis::UpstreamOss,
            summary: "sessions/ and archived_sessions/ under the Codex home",
            evidence: tokscale("codex", "clients.rs Codex plus scanner archived_sessions"),
        },
        DiscoveryRule {
            id: "claude-config",
            provider_id: "claude_code",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::LocalQualification,
            summary: "CLAUDE_SECURESTORAGE_CONFIG_DIR, then CLAUDE_CONFIG_DIR, otherwise ~/.claude",
            evidence: local(
                "claude_code",
                "CLAUDE_CONFIG_DIR is also recorded by Tokscale; the secure-storage directory is local knowledge and wins when set",
            ),
        },
        DiscoveryRule {
            id: "claude-sessions",
            provider_id: "claude_code",
            role: RuleRole::SessionStore,
            basis: RuleBasis::UpstreamOss,
            summary: "projects/ and transcripts/ under the Claude config directory",
            evidence: tokscale(
                "claude",
                "clients.rs Claude plus scanner transcripts extra root",
            ),
        },
        DiscoveryRule {
            id: "gemini-home",
            provider_id: "gemini_cli",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::UpstreamOss,
            summary: "GEMINI_CLI_HOME, otherwise ~/.gemini",
            evidence: tokscale("gemini", "clients.rs Gemini EnvVar GEMINI_CLI_HOME"),
        },
        DiscoveryRule {
            id: "antigravity-extension",
            provider_id: "antigravity",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::LocalQualification,
            summary: "~/.gemini/antigravity stays under the user home when GEMINI_CLI_HOME is set",
            evidence: local(
                "antigravity",
                "Tokscale's antigravity-extension client is Home-rooted and does not follow GEMINI_CLI_HOME",
            ),
        },
        DiscoveryRule {
            id: "antigravity-cli",
            provider_id: "antigravity",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::UpstreamOss,
            summary: "<gemini home>/antigravity-cli, honoring GEMINI_CLI_HOME",
            evidence: tokscale(
                "antigravity-cli",
                "clients.rs AntigravityCli under GEMINI_CLI_HOME",
            ),
        },
        DiscoveryRule {
            id: "antigravity-tokscale-cache",
            provider_id: "antigravity",
            role: RuleRole::ExcludedUpstream,
            summary: "Tokscale's own antigravity-cache is not an Antigravity installation",
            basis: RuleBasis::UpstreamOss,
            evidence: tokscale(
                "antigravity",
                "clients.rs Antigravity uses PathRoot::Config, which is Tokscale's config dir",
            ),
        },
        DiscoveryRule {
            id: "opencode-data",
            provider_id: "opencode",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::LocalQualification,
            summary: "XDG data home opencode/, or OPENCODE_CONFIG_DIR when set",
            evidence: local(
                "opencode",
                "Tokscale scans XDG data opencode/storage; OriginLM identifies the data directory",
            ),
        },
        DiscoveryRule {
            id: "cursor-install",
            provider_id: "cursor",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::LocalQualification,
            summary: "Platform application-data Cursor directory",
            evidence: local(
                "cursor",
                "Tokscale's cursor root is its own usage cache and is not used as an installation",
            ),
        },
        DiscoveryRule {
            id: "cursor-tokscale-cache",
            provider_id: "cursor",
            role: RuleRole::ExcludedUpstream,
            basis: RuleBasis::UpstreamOss,
            summary: "~/.config/tokscale/cursor-cache is Tokscale usage data, not Cursor",
            evidence: tokscale(
                "cursor",
                "clients.rs Cursor relative .config/tokscale/cursor-cache",
            ),
        },
        DiscoveryRule {
            id: "copilot-home",
            provider_id: "github_copilot",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::UpstreamOss,
            summary: "~/.copilot holds the CLI session store and desktop database",
            evidence: tokscale(
                "copilot",
                "clients.rs Copilot ~/.copilot/otel; sessions/copilot_session_store.rs session-store.db",
            ),
        },
        DiscoveryRule {
            id: "grok-home",
            provider_id: "grok",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::UpstreamOss,
            summary: "GROK_HOME, otherwise ~/.grok",
            evidence: tokscale("grok", "clients.rs Grok EnvVar GROK_HOME"),
        },
        DiscoveryRule {
            id: "pi-agent",
            provider_id: "pi",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::UpstreamOss,
            summary: "PI_CODING_AGENT_DIR, otherwise ~/.pi/agent. PI_CODING_AGENT_SESSION_DIR is a session directory, not an installation.",
            evidence: t3code("packages/provider-pi/src/server/usage.ts piUsageReader.directories"),
        },
        DiscoveryRule {
            id: "fx-home",
            provider_id: "fx",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::UpstreamOss,
            summary: "~/.fx holds fx sessions. There is no home override. The fx command is not launched.",
            evidence: t3code(
                "packages/provider-fx/src/server/driver.ts sessions under ~/.fx; status.ts command fx",
            ),
        },
        DiscoveryRule {
            id: "opencode-native-bin",
            provider_id: "opencode",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::UpstreamOss,
            summary: "~/.opencode/bin is searched for the opencode executable",
            evidence: t3code(
                "packages/provider-opencode/src/server/driver.ts isOpenCodeNativeCommandPath",
            ),
        },
        DiscoveryRule {
            id: "ollama-home",
            provider_id: "ollama",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::LocalQualification,
            summary: "~/.ollama; Tokscale does not discover Ollama",
            evidence: local("ollama", "Local qualification; no Tokscale client"),
        },
        DiscoveryRule {
            id: "lmstudio-home",
            provider_id: "lmstudio",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::UpstreamOss,
            summary: "LM_STUDIO_HOME, otherwise ~/.lmstudio",
            evidence: tokscale("lmstudio", "clients.rs LmStudio EnvVar LM_STUDIO_HOME"),
        },
        DiscoveryRule {
            id: "windows-home-redirect",
            provider_id: "environment",
            role: RuleRole::EnvOverride,
            basis: RuleBasis::UpstreamOss,
            summary: "On Windows, absolute native HOME redirects the scan profile",
            evidence: tokscale("paths", "paths.rs home_dir windows_native_home_override"),
        },
        DiscoveryRule {
            id: "native-separators",
            provider_id: "environment",
            role: RuleRole::ProfileHome,
            basis: RuleBasis::UpstreamOss,
            summary: "Relative discovery literals are stored with native separators",
            evidence: tokscale("paths", "clients.rs join_home component push"),
        },
    ]
}

pub fn profile_roots(provider_id: &str, env: &ScanEnvironment) -> Vec<PathBuf> {
    match provider_id {
        "codex" => overridden(env, &["CODEX_HOME"], || {
            vec![join_relative(&env.home, ".codex")]
        }),
        "claude_code" => overridden(
            env,
            &["CLAUDE_SECURESTORAGE_CONFIG_DIR", "CLAUDE_CONFIG_DIR"],
            || vec![join_relative(&env.home, ".claude")],
        ),
        "gemini_cli" => overridden(env, &["GEMINI_CLI_HOME"], || {
            vec![join_relative(&env.home, ".gemini")]
        }),
        "antigravity" => {
            let gemini = env
                .overrides
                .get("GEMINI_CLI_HOME")
                .cloned()
                .unwrap_or_else(|| join_relative(&env.home, ".gemini"));
            vec![
                join_relative(&env.home, ".gemini/antigravity"),
                join_relative(&gemini, "antigravity-cli"),
            ]
        }
        "grok" => overridden(env, &["GROK_HOME"], || {
            vec![join_relative(&env.home, ".grok")]
        }),
        "opencode" => overridden(env, &["OPENCODE_CONFIG_DIR"], || {
            vec![join_relative(&env.data_home, "opencode")]
        }),
        // Muse keeps settings and its credential store under
        // `$XDG_CONFIG_HOME/muse`, else `~/.config/muse` (Windows too).
        "muse" => vec![join_relative(&env.config_home, "muse")],
        "cursor" => vec![match env.platform {
            HostPlatform::Windows => join_relative(&env.app_data, "Cursor"),
            HostPlatform::Macos => join_relative(&env.home, "Library/Application Support/Cursor"),
            HostPlatform::Linux => join_relative(&env.config_home, "Cursor"),
        }],
        "github_copilot" => vec![join_relative(&env.home, ".copilot")],
        "pi" => overridden(env, &["PI_CODING_AGENT_DIR"], || {
            vec![join_relative(&env.home, ".pi/agent")]
        }),
        "fx" => vec![join_relative(&env.home, ".fx")],
        "ollama" => vec![join_relative(&env.home, ".ollama")],
        "lmstudio" => overridden(env, &["LM_STUDIO_HOME"], || {
            vec![join_relative(&env.home, ".lmstudio")]
        }),
        _ => vec![],
    }
}

pub fn has_profile_override(provider_id: &str, env: &ScanEnvironment) -> bool {
    let keys: &[&str] = match provider_id {
        "codex" => &["CODEX_HOME"],
        "claude_code" => &["CLAUDE_SECURESTORAGE_CONFIG_DIR", "CLAUDE_CONFIG_DIR"],
        "gemini_cli" | "antigravity" => &["GEMINI_CLI_HOME"],
        "grok" => &["GROK_HOME"],
        "opencode" => &["OPENCODE_CONFIG_DIR"],
        "pi" => &["PI_CODING_AGENT_DIR"],
        "lmstudio" => &["LM_STUDIO_HOME"],
        _ => &[],
    };
    keys.iter().any(|key| env.overrides.contains_key(*key))
}

/// Session and database locations under a profile. These are not scanned as
/// separate installations, and passive discovery does not open them.
pub fn session_stores(provider_id: &str, profile: &Path) -> Vec<PathBuf> {
    let relatives: &[&str] = match provider_id {
        "codex" => &["sessions", "archived_sessions"],
        "claude_code" => &["projects", "transcripts"],
        "gemini_cli" => &["tmp"],
        "antigravity" => &["conversations"],
        "grok" => &["sessions", "logs/unified.jsonl"],
        "opencode" => &["storage/message"],
        "github_copilot" => &["session-store.db", "otel", "data.db"],
        "pi" => &["sessions"],
        "lmstudio" => &["server-logs"],
        _ => &[],
    };
    relatives
        .iter()
        .map(|relative| join_relative(profile, relative))
        .collect()
}

fn overridden(
    env: &ScanEnvironment,
    keys: &[&str],
    fallback: impl FnOnce() -> Vec<PathBuf>,
) -> Vec<PathBuf> {
    if let Some(root) = keys.iter().find_map(|key| env.overrides.get(*key)) {
        vec![root.clone()]
    } else {
        fallback()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_client_evidence_outranks_tokscale() {
        assert_eq!(
            RuleBasis::preferred(&[
                RuleBasis::Convention,
                RuleBasis::UpstreamOss,
                RuleBasis::LocalQualification,
                RuleBasis::OfficialClient,
            ]),
            Some(RuleBasis::OfficialClient)
        );
        let rules = discovery_rules();
        let codex = rules.iter().find(|rule| rule.id == "codex-home").unwrap();
        let sessions = rules
            .iter()
            .find(|rule| rule.id == "codex-sessions")
            .unwrap();
        assert_eq!(
            RuleBasis::preferred(&[sessions.basis, codex.basis]),
            Some(RuleBasis::OfficialClient)
        );
        assert_eq!(codex.evidence.kind, EvidenceKind::OfficialProtocol);
        assert_eq!(sessions.evidence.adapter_id, "upstream.tokscale");
        assert!(
            sessions
                .evidence
                .reference
                .as_deref()
                .is_some_and(|reference| reference.contains(TOKSCALE_COMMIT))
        );
    }
}
