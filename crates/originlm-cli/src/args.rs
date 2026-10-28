use originlm_contracts::{DomainError, ErrorCode, Result};
use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};
#[derive(Debug)]
pub enum Action {
    Help,
    Version,
    Scan,
    Status,
    Serve,
    Connect,
}
pub struct Args {
    pub action: Action,
    pub json: bool,
    pub home: Option<PathBuf>,
    pub endpoints: Option<PathBuf>,
    pub native_profiles: Option<PathBuf>,
    pub chatgpt_app_name: Option<String>,
    pub billing_declarations: Option<PathBuf>,
    pub allow_passive: bool,
    pub allow_codex_metadata: bool,
    pub warm_metadata: bool,
    pub no_metadata_cache: bool,
    pub http: bool,
    pub ipc: Option<String>,
    pub independent: bool,
    pub token_file: Option<PathBuf>,
    pub admin_token_file: Option<PathBuf>,
    pub grants_file: Option<PathBuf>,
    pub idle_timeout_seconds: Option<u64>,
    pub os_notifications: bool,
    pub port: u16,
    pub assets: Option<PathBuf>,
    pub allow_execution: bool,
    pub workspace: Option<PathBuf>,
    pub allow_writes: bool,
    pub review_requests: bool,
    pub allow_native_policy: bool,
    pub allow_paid: bool,
    pub metadata_providers: BTreeSet<String>,
    pub usage_providers: BTreeSet<String>,
    pub execution_providers: BTreeSet<String>,
    pub executable_only_providers: BTreeSet<String>,
}
fn absolute(value: OsString) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    let text = path.to_string_lossy();
    if !path.is_absolute()
        || text.starts_with(r"\\")
        || text.starts_with("//")
        || text.chars().any(char::is_control)
    {
        Err(invalid())
    } else {
        Ok(path)
    }
}
impl Args {
    pub fn parse(input: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut input = input.into_iter();
        let command = input.next().unwrap_or_else(|| "--help".into());
        if command == "connect" {
            let rest: Vec<_> = input.collect();
            if rest.len() != 2 || rest[0] != "--ipc" {
                return Err(invalid());
            }
            let mut args = Self::parse([OsString::from("serve"), OsString::from("--stdio")])?;
            args.action = Action::Connect;
            args.ipc = Some(rest[1].clone().into_string().map_err(|_| invalid())?);
            return Ok(args);
        }
        let action = match command.to_str() {
            Some("--help" | "-h") => Action::Help,
            Some("--version" | "-V") => Action::Version,
            Some("scan") => Action::Scan,
            Some("status") => Action::Status,
            Some("serve") => Action::Serve,
            _ => return Err(invalid()),
        };
        let mut args = Self {
            action,
            json: false,
            home: None,
            endpoints: None,
            native_profiles: None,
            chatgpt_app_name: None,
            billing_declarations: None,
            allow_passive: false,
            allow_codex_metadata: false,
            warm_metadata: false,
            no_metadata_cache: false,
            http: false,
            ipc: None,
            independent: false,
            token_file: None,
            admin_token_file: None,
            grants_file: None,
            idle_timeout_seconds: None,
            os_notifications: false,
            port: 4499,
            assets: None,
            allow_execution: false,
            workspace: None,
            allow_writes: false,
            review_requests: false,
            allow_native_policy: false,
            allow_paid: false,
            metadata_providers: BTreeSet::new(),
            usage_providers: BTreeSet::new(),
            execution_providers: BTreeSet::new(),
            executable_only_providers: BTreeSet::new(),
        };
        let mut seen = BTreeSet::new();
        let (mut json, mut stdio) = (false, false);
        while let Some(flag) = input.next() {
            let flag = flag.to_str().ok_or_else(invalid)?;
            if !seen.insert(flag.to_owned())
                && ![
                    "--allow-metadata",
                    "--allow-usage",
                    "--allow-execution",
                    "--executable-only-provider",
                ]
                .contains(&flag)
            {
                return Err(invalid());
            }
            match flag {
                "--json" if matches!(args.action, Action::Scan | Action::Status) => json = true,
                "--home" if matches!(args.action, Action::Scan | Action::Serve) => {
                    args.home = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--endpoints" if matches!(args.action, Action::Serve | Action::Scan) => {
                    args.endpoints = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--native-profiles" if matches!(args.action, Action::Serve | Action::Scan) => {
                    args.native_profiles = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--chatgpt-app-name" if matches!(args.action, Action::Serve) => {
                    let name = input
                        .next()
                        .and_then(|v| v.into_string().ok())
                        .ok_or_else(invalid)?;
                    if name.is_empty()
                        || name.len() > 64
                        || !name.bytes().all(|b| {
                            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'
                        })
                    {
                        return Err(invalid());
                    }
                    args.chatgpt_app_name = Some(name);
                }
                "--billing-declarations" if matches!(args.action, Action::Serve | Action::Scan) => {
                    args.billing_declarations = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--stdio" if matches!(args.action, Action::Serve) => stdio = true,
                "--http" if matches!(args.action, Action::Serve) => args.http = true,
                "--ipc" if matches!(args.action, Action::Serve | Action::Status) => {
                    args.ipc = Some(
                        input
                            .next()
                            .and_then(|s| s.into_string().ok())
                            .ok_or_else(invalid)?,
                    )
                }
                "--independent" if matches!(args.action, Action::Serve) => args.independent = true,
                "--token-file" if matches!(args.action, Action::Serve | Action::Status) => {
                    args.token_file = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--admin-token-file" if matches!(args.action, Action::Serve) => {
                    args.admin_token_file = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--grants-file" if matches!(args.action, Action::Serve) => {
                    args.grants_file = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--os-notifications" if matches!(args.action, Action::Serve) => {
                    args.os_notifications = true
                }
                "--idle-timeout" if matches!(args.action, Action::Serve) => {
                    let seconds = input
                        .next()
                        .and_then(|value| {
                            value.to_str().and_then(|value| value.parse::<u64>().ok())
                        })
                        .ok_or_else(invalid)?;
                    if !(5..=86_400).contains(&seconds) {
                        return Err(invalid());
                    }
                    args.idle_timeout_seconds = Some(seconds);
                }
                "--allow-passive-discovery" if matches!(args.action, Action::Serve) => {
                    args.allow_passive = true
                }
                "--executable-only-provider"
                    if matches!(args.action, Action::Serve | Action::Scan) =>
                {
                    let provider = input
                        .next()
                        .and_then(|v| v.into_string().ok())
                        .ok_or_else(invalid)?;
                    if !originlm_discovery::definitions()
                        .iter()
                        .any(|definition| definition.id == provider)
                    {
                        return Err(invalid());
                    }
                    args.executable_only_providers.insert(provider);
                }
                "--allow-usage" if matches!(args.action, Action::Serve) => {
                    let provider = input
                        .next()
                        .and_then(|v| v.into_string().ok())
                        .filter(|v| v == "cursor")
                        .ok_or_else(invalid)?;
                    args.usage_providers.insert(provider);
                }
                "--allow-metadata" | "--allow-execution"
                    if matches!(args.action, Action::Serve) =>
                {
                    let provider = input
                        .next()
                        .and_then(|v| v.into_string().ok())
                        .filter(|v| {
                            [
                                "codex",
                                "codex_managed",
                                "claude_code",
                                "ollama",
                                "lmstudio",
                                "openai_api",
                                "anthropic_api",
                                "gemini_api",
                                "xai_api",
                                "compatible_api",
                                "opencode",
                                "muse",
                                "gemini_cli",
                                "antigravity",
                                "grok",
                            ]
                            .contains(&v.as_str())
                                || (flag == "--allow-metadata" && v == "cursor")
                        })
                        .ok_or_else(invalid)?;
                    if flag == "--allow-metadata" {
                        args.metadata_providers.insert(provider);
                    } else {
                        args.execution_providers.insert(provider);
                        args.allow_execution = true;
                    }
                }
                "--allow-codex-metadata" if matches!(args.action, Action::Serve) => {
                    args.allow_codex_metadata = true
                }
                "--allow-codex-execution" if matches!(args.action, Action::Serve) => {
                    args.allow_execution = true
                }
                "--warm-metadata" if matches!(args.action, Action::Serve) => {
                    args.warm_metadata = true
                }
                "--no-metadata-cache" if matches!(args.action, Action::Serve) => {
                    args.no_metadata_cache = true
                }
                "--workspace" if matches!(args.action, Action::Serve) => {
                    args.workspace = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--assets" if matches!(args.action, Action::Serve) => {
                    args.assets = Some(absolute(input.next().ok_or_else(invalid)?)?)
                }
                "--port" if matches!(args.action, Action::Serve) => {
                    args.port = input
                        .next()
                        .and_then(|s| s.to_str().and_then(|s| s.parse().ok()))
                        .ok_or_else(invalid)?
                }
                "--allow-writes" if matches!(args.action, Action::Serve) => {
                    args.allow_writes = true
                }
                "--review-native-requests" if matches!(args.action, Action::Serve) => {
                    args.review_requests = true
                }
                "--allow-native-policy" if matches!(args.action, Action::Serve) => {
                    args.allow_native_policy = true
                }
                "--allow-paid-options" if matches!(args.action, Action::Serve) => {
                    args.allow_paid = true
                }
                _ => return Err(invalid()),
            }
        }
        if matches!(args.action, Action::Scan) && !json
            || matches!(args.action, Action::Serve)
                && (u8::from(stdio) + u8::from(args.http) + u8::from(args.ipc.is_some()) != 1)
        {
            return Err(invalid());
        }
        args.json = json;
        if args.allow_codex_metadata {
            args.metadata_providers.insert("codex".into());
        }
        if seen.contains("--allow-codex-execution") {
            args.execution_providers.insert("codex".into());
        }
        if !args.usage_providers.is_empty()
            && args.usage_providers.is_disjoint(&args.metadata_providers)
        {
            // Usage is part of a metadata probe, so it cannot be granted alone.
            return Err(invalid());
        }
        if !args
            .executable_only_providers
            .is_disjoint(&args.metadata_providers)
            || !args
                .executable_only_providers
                .is_disjoint(&args.execution_providers)
        {
            return Err(invalid());
        }
        let native_execution = args
            .execution_providers
            .iter()
            .any(|p| ["gemini_cli", "antigravity", "grok"].contains(&p.as_str()));
        // Warm-up only makes sense where the host has authorized passive discovery and some metadata provider.
        if args.warm_metadata
            && (!args.http || !args.allow_passive || args.metadata_providers.is_empty())
        {
            return Err(invalid());
        }
        if matches!(args.action, Action::Status) {
            // Status only reads an existing broker; it never configures one.
            if stdio
                || args.http
                || args.independent
                || args.allow_execution
                || args.workspace.is_some()
                || args.grants_file.is_some()
                || args.admin_token_file.is_some()
                || args.warm_metadata
                || args.os_notifications
            {
                return Err(invalid());
            }
            return Ok(args);
        }
        if (!args.http && (args.assets.is_some() || seen.contains("--port")))
            || (native_execution
                && args.native_profiles.is_none()
                && args.billing_declarations.is_none())
            || (args.allow_execution
                && (args.workspace.is_none()
                    || !args.execution_providers.is_subset(&args.metadata_providers)))
            || (!args.allow_execution
                && (args.allow_writes
                    || args.review_requests
                    || args.allow_native_policy
                    || args.allow_paid
                    || args.workspace.is_some()))
            || (args.independent
                && (args.ipc.is_none()
                    || (args.token_file.is_none() && args.grants_file.is_none())))
            || (!args.independent && args.admin_token_file.is_some())
            || (!args.independent && args.grants_file.is_some())
            || (!args.independent
                && (args.token_file.is_some() || args.idle_timeout_seconds.is_some()))
            || (args.independent && (args.http || stdio))
            || (args.os_notifications && !cfg!(feature = "os-notifications"))
        {
            return Err(invalid());
        }
        Ok(args)
    }
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::InvalidRequest,
        "Invalid command arguments. Run originlm --help.",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Args> {
        Args::parse(args.iter().map(OsString::from))
    }
    #[test]
    fn explicit_http_serving_and_assets_are_supported() {
        let directory = std::env::temp_dir();
        let path = directory.to_str().unwrap();
        assert!(
            parse(&[
                "serve",
                "--http",
                "--port",
                "0",
                "--assets",
                path,
                "--allow-passive-discovery"
            ])
            .is_ok()
        );
    }
    #[test]
    fn warm_up_is_opt_in_and_needs_http_discovery_and_a_metadata_provider() {
        assert!(
            parse(&[
                "serve",
                "--http",
                "--allow-passive-discovery",
                "--allow-metadata",
                "codex",
                "--warm-metadata"
            ])
            .is_ok()
        );
        assert!(
            parse(&[
                "serve",
                "--http",
                "--allow-passive-discovery",
                "--warm-metadata"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "serve",
                "--http",
                "--allow-metadata",
                "codex",
                "--warm-metadata"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "serve",
                "--stdio",
                "--allow-passive-discovery",
                "--allow-metadata",
                "codex",
                "--warm-metadata"
            ])
            .is_err()
        );
        assert!(!parse(&["serve", "--http"]).unwrap().warm_metadata);
    }
    #[test]
    fn execution_needs_explicit_workspace_and_metadata_permission() {
        let directory = std::env::temp_dir();
        let path = directory.to_str().unwrap();
        assert!(
            parse(&[
                "serve",
                "--stdio",
                "--allow-codex-metadata",
                "--allow-codex-execution",
                "--workspace",
                path
            ])
            .is_ok()
        );
        assert!(parse(&["serve", "--stdio", "--allow-codex-execution"]).is_err());
    }
    #[test]
    fn cursor_usage_is_a_separate_grant_that_requires_the_metadata_grant() {
        let args = parse(&[
            "serve",
            "--stdio",
            "--allow-metadata",
            "cursor",
            "--allow-usage",
            "cursor",
        ])
        .unwrap();
        assert!(args.metadata_providers.contains("cursor"));
        assert!(args.usage_providers.contains("cursor"));
        assert!(parse(&["serve", "--stdio", "--allow-usage", "cursor"]).is_err());
        assert!(
            parse(&[
                "serve",
                "--stdio",
                "--allow-metadata",
                "cursor",
                "--allow-usage",
                "codex"
            ])
            .is_err()
        );
    }

    #[test]
    fn unsafe_or_conflicting_flags_are_rejected() {
        for args in [
            vec!["serve", "--http", "--stdio"],
            vec!["serve", "--http", "--port", "99999"],
            vec!["serve", "--stdio", "--allow-writes"],
            vec!["serve", "--stdio", "--allow-native-policy"],
            vec!["serve", "--http", "--host", "0.0.0.0"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
}

#[cfg(test)]
mod additional_provider_tests {
    use super::*;
    #[test]
    fn grok_metadata_is_explicit_and_does_not_imply_execution() {
        let args = Args::parse(
            ["serve", "--stdio", "--allow-metadata", "grok"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert!(args.metadata_providers.contains("grok"));
        assert!(!args.allow_execution);
    }
    #[test]
    fn native_subscription_execution_requires_an_explicit_profile_file() {
        let workspace = std::env::temp_dir();
        let value = workspace.to_str().unwrap();
        assert!(
            Args::parse(
                [
                    "serve",
                    "--stdio",
                    "--allow-metadata",
                    "grok",
                    "--allow-execution",
                    "grok",
                    "--workspace",
                    value
                ]
                .into_iter()
                .map(OsString::from)
            )
            .is_err()
        );
        let config = workspace.join("native-profiles.json");
        let config = config.to_str().unwrap();
        let args = Args::parse(
            [
                "serve",
                "--stdio",
                "--native-profiles",
                config,
                "--allow-metadata",
                "grok",
                "--allow-execution",
                "grok",
                "--workspace",
                value,
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();
        assert!(args.execution_providers.contains("grok"));
        assert_eq!(
            args.native_profiles.as_deref(),
            Some(std::path::Path::new(config))
        );
    }
    #[test]
    fn billing_declarations_allow_declared_grok_execution_without_native_profiles() {
        let workspace = std::env::temp_dir();
        let value = workspace.to_str().unwrap();
        let config = workspace.join("billing-declarations.json");
        let parse = |declarations: bool| {
            let mut list = vec!["serve", "--stdio"];
            if declarations {
                list.extend(["--billing-declarations", config.to_str().unwrap()]);
            }
            list.extend([
                "--allow-metadata",
                "grok",
                "--allow-execution",
                "grok",
                "--workspace",
                value,
            ]);
            Args::parse(list.into_iter().map(OsString::from))
        };
        assert!(parse(false).is_err());
        let args = parse(true).unwrap();
        assert_eq!(args.billing_declarations.as_deref(), Some(config.as_path()));
        assert!(
            Args::parse(
                [
                    "serve",
                    "--stdio",
                    "--billing-declarations",
                    "relative.json"
                ]
                .into_iter()
                .map(OsString::from)
            )
            .is_err()
        );
    }
    #[test]
    fn managed_chatgpt_registration_is_opt_in_and_needs_no_native_profile_file() {
        let parse = |args: &[&str]| Args::parse(args.iter().map(OsString::from));
        let args = parse(&[
            "serve",
            "--stdio",
            "--allow-passive-discovery",
            "--chatgpt-app-name",
            "example-app",
            "--allow-metadata",
            "codex_managed",
        ])
        .unwrap();
        assert_eq!(args.chatgpt_app_name.as_deref(), Some("example-app"));
        assert!(args.native_profiles.is_none());
        assert!(!args.allow_execution);
        assert!(parse(&["serve", "--stdio", "--chatgpt-app-name", "bad name"]).is_err());
    }
    #[test]
    fn independent_grants_replace_the_shared_owner_token_and_need_independent_mode() {
        let parse = |args: &[&str]| Args::parse(args.iter().map(OsString::from));
        let grants = std::env::temp_dir().join("originlm-independent-grants.json");
        let grants = grants.to_str().unwrap();
        let args = parse(&[
            "serve",
            "--ipc",
            "originlm-grants-test",
            "--independent",
            "--grants-file",
            grants,
            "--admin-token-file",
            std::env::temp_dir()
                .join("originlm-admin-token.json")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        assert_eq!(
            args.grants_file.as_deref(),
            Some(std::path::Path::new(grants))
        );
        assert!(args.token_file.is_none());
        assert!(
            parse(&[
                "serve",
                "--ipc",
                "originlm-grants-test",
                "--independent",
                "--allow-passive-discovery",
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "serve",
                "--ipc",
                "originlm-grants-test",
                "--independent",
                "--token-file",
                std::env::temp_dir()
                    .join("originlm-owner-token.json")
                    .to_str()
                    .unwrap(),
            ])
            .is_ok()
        );
        assert!(
            parse(&[
                "serve",
                "--ipc",
                "originlm-grants-test",
                "--grants-file",
                grants,
            ])
            .is_err()
        );
    }
    #[test]
    fn executable_only_provider_is_repeatable_and_rejects_unknown_or_conflicting_grants() {
        let parse = |args: &[&str]| Args::parse(args.iter().map(OsString::from));
        let selected = parse(&[
            "serve",
            "--stdio",
            "--executable-only-provider",
            "codex",
            "--executable-only-provider",
            "claude_code",
            "--executable-only-provider",
            "codex",
            "--allow-metadata",
            "codex_managed",
        ])
        .unwrap();
        assert_eq!(
            selected.executable_only_providers,
            BTreeSet::from(["claude_code".into(), "codex".into()])
        );
        assert!(parse(&["scan", "--json", "--executable-only-provider", "unknown"]).is_err());
        assert!(
            parse(&[
                "serve",
                "--stdio",
                "--executable-only-provider",
                "codex",
                "--allow-metadata",
                "codex"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "serve",
                "--stdio",
                "--executable-only-provider",
                "codex",
                "--allow-codex-metadata"
            ])
            .is_err()
        );
        let workspace = std::env::temp_dir();
        assert!(
            parse(&[
                "serve",
                "--stdio",
                "--executable-only-provider",
                "codex",
                "--allow-metadata",
                "codex",
                "--allow-execution",
                "codex",
                "--workspace",
                workspace.to_str().unwrap()
            ])
            .is_err()
        );
    }
}
