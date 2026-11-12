//! Provider CLI update status and outcomes.
//!
//! These describe the provider CLI binaries installed on the host (`claude`,
//! `codex`, `opencode`, ...), never OriginLM itself and never an account or
//! subscription. They are produced only by an explicit local operator command
//! (`originlm providers check` / `originlm providers update`); importing this
//! module performs no I/O.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where an installed provider CLI stands against its owning installer's latest release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUpdateStatus {
    /// The installed version is the latest the owning installer offers.
    Current,
    /// A newer version exists and OriginLM can run the owning installer to get it.
    BehindLatest,
    /// A newer version exists, but no installer is proven to own this binary, or
    /// its installer must not be run by OriginLM. `manualCommand` and `reason`
    /// say what to do instead.
    ManualOnly,
    /// The installed or latest version could not be determined. Never guessed.
    Unknown,
}

/// How OriginLM can update this install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUpdateSupport {
    /// An owning installer is proven; `originlm providers update` can run it.
    Installer,
    /// The owner is known or suspected but must be updated by hand or by its own app.
    Manual,
    /// OriginLM has no update path for this provider.
    Unsupported,
}

/// Evidence for a probed version: the exact command and the line it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionEvidence {
    pub command: String,
    pub exit_ok: bool,
    /// First non-empty output line, bounded to 200 characters.
    pub output: String,
}

/// One installed provider CLI binary and its update standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderUpdateReport {
    pub provider_id: String,
    pub name: String,
    /// The executable discovery found. Private host path: local operator output only.
    pub executable: String,
    /// The executable after following symlinks.
    pub real_executable: String,
    pub installed_version: Option<String>,
    pub version_evidence: Option<VersionEvidence>,
    pub latest_version: Option<String>,
    /// Where `latestVersion` came from, e.g. `npm view @openai/codex@latest version`.
    pub latest_source: Option<String>,
    pub status: ProviderUpdateStatus,
    pub support: ProviderUpdateSupport,
    /// The owning installer, e.g. `npm-global`, `homebrew`, `mise`, `claude-native`.
    pub installer: Option<String>,
    /// Why that installer is believed to own the binary.
    pub ownership_proof: Option<String>,
    /// The command `originlm providers update` would run. Present only when proven.
    pub update_command: Option<String>,
    /// A command to run by hand. Displayed only; OriginLM never runs it.
    pub manual_command: Option<String>,
    /// One update at a time per install location shares this key.
    pub lock_key: Option<String>,
    /// Human explanation for `manual_only` / `unknown` / `unsupported`.
    pub reason: Option<String>,
}

/// The result of `originlm providers check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderUpdateCheck {
    pub checked_at: String,
    pub providers: Vec<ProviderUpdateReport>,
}

/// What happened (or, for a dry run, would happen) for one install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUpdateResult {
    /// Dry run: the command was printed, nothing changed.
    WouldRun,
    /// The installer ran and the re-probed version is installed, readable and current.
    Updated,
    /// The install is already current.
    AlreadyCurrent,
    /// Not runnable by OriginLM; see `reason` and `manualCommand`.
    Skipped,
    /// The installer ran but the update is not proven, or OriginLM refused to run it.
    Failed,
}

/// Model discovery refresh after a successful update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRediscovery {
    /// Connections of this provider found by the post-update passive scan.
    pub connections: Vec<String>,
    /// Cached metadata (model lists) dropped so the next probe asks the new CLI.
    pub invalidated_metadata: u32,
    pub note: Option<String>,
}

/// One entry of `originlm providers update` output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderUpdateOutcome {
    pub provider_id: String,
    pub executable: String,
    pub result: ProviderUpdateResult,
    pub command: Option<String>,
    pub ownership_proof: Option<String>,
    pub previous_version: Option<String>,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub manual_command: Option<String>,
    /// Refusal reason, or the last error line the installer printed.
    pub reason: Option<String>,
    pub rediscovery: Option<ProviderRediscovery>,
    pub notes: Vec<String>,
}
