use crate::{HostPlatform, ScanEnvironment};
use std::path::PathBuf;
/// Absolute path of the Muse Code binary or launcher, for installs that are
/// not on `PATH` or in the default install directory.
pub const MUSE_BINARY_OVERRIDE: &str = "LOCAL_AI_KIT_MUSE_BINARY";
#[derive(Debug, Clone, Copy)]
pub struct ProviderDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub commands: &'static [&'static str],
    pub profile_prefix: Option<&'static str>,
}
pub fn definitions() -> &'static [ProviderDefinition] {
    &[
        ProviderDefinition {
            id: "codex",
            name: "Codex",
            commands: &["codex"],
            profile_prefix: Some(".codex-"),
        },
        ProviderDefinition {
            id: "claude_code",
            name: "Claude Code",
            commands: &["claude"],
            profile_prefix: Some(".claude-"),
        },
        ProviderDefinition {
            id: "gemini_cli",
            name: "Gemini CLI",
            commands: &["gemini"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "antigravity",
            name: "Antigravity",
            commands: &["antigravity", "agy", "agy_acp_server", "agy_acp_server.par"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "grok",
            name: "Grok",
            commands: &["grok"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "opencode",
            name: "OpenCode",
            commands: &["opencode"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "muse",
            name: "Muse Code",
            commands: &["muse"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "cursor",
            name: "Cursor",
            commands: &["cursor-agent"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "github_copilot",
            name: "GitHub Copilot",
            commands: &["copilot"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "pi",
            name: "Pi",
            commands: &["pi"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "fx",
            name: "fx",
            commands: &["fx"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "ollama",
            name: "Ollama",
            commands: &["ollama"],
            profile_prefix: None,
        },
        ProviderDefinition {
            id: "lmstudio",
            name: "LM Studio",
            commands: &["lms"],
            profile_prefix: None,
        },
    ]
}
impl ProviderDefinition {
    pub(crate) fn roots(&self, env: &ScanEnvironment) -> Vec<PathBuf> {
        crate::knowledge::profile_roots(self.id, env)
    }
    /// Install directories searched in addition to `PATH` for this provider.
    pub(crate) fn extra_directories(&self, env: &ScanEnvironment) -> Vec<PathBuf> {
        match (self.id, env.platform) {
            // The official Muse Code installer's per-user location.
            ("muse", HostPlatform::Windows) => vec![env.local_app_data.join("Programs/muse")],
            // OpenCode's own installer places the binary here even when it is
            // not on PATH. T3 Code treats this path as a native install.
            ("opencode", _) => vec![crate::join_relative(&env.home, ".opencode/bin")],
            // T3 Code installs the Gemini CLI under its managed tools directory.
            // A Rust host that shares the same machine can use that install
            // without requiring a second copy on PATH.
            ("antigravity", _) => vec![
                crate::join_relative(&env.home, ".gemini/antigravity-acp"),
                crate::join_relative(&env.home, ".gemini/antigravity-cli/bin"),
            ],
            ("gemini_cli", _) => {
                let mut dirs = vec![crate::join_relative(&env.home, ".local/bin")];
                let tools = crate::join_relative(&env.home, ".t3/tools/gemini");
                if let Ok(entries) = std::fs::read_dir(&tools) {
                    for entry in entries.flatten() {
                        let dir = entry.path().join("npm/bin");
                        if dir.is_dir() {
                            dirs.push(dir);
                        }
                    }
                }
                dirs
            }
            _ => vec![],
        }
    }
    /// An explicitly configured executable that replaces the search.
    pub(crate) fn executable_override<'a>(&self, env: &'a ScanEnvironment) -> Option<&'a PathBuf> {
        env.executable_overrides
            .get(self.id)
            .or_else(|| match self.id {
                "muse" => env.overrides.get(MUSE_BINARY_OVERRIDE),
                _ => None,
            })
    }
    pub(crate) fn has_override(&self, env: &ScanEnvironment) -> bool {
        crate::knowledge::has_profile_override(self.id, env)
    }
}
