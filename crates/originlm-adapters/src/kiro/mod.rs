//! Kiro CLI metadata from the installed client.
//!
//! Only the client's own public commands are used: `--version`, `whoami
//! --format json`, and `chat --list-models --format json`. The model list is
//! account-scoped, so it is requested only after `whoami` confirms a session;
//! Kiro starts a browser login when a signed-out session requests models.
//!
//! Billing and token usage are not invented. Kiro reports a sign-in state, not
//! a plan, and this adapter does not read credential files.
use crate::process::{LaunchSpec, capture, capture_status};
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
use serde_json::Value;
use std::{collections::BTreeSet, time::Duration};

const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const CATALOG_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_MODELS: usize = 512;

fn unavailable() -> DomainError {
    DomainError::new(
        ErrorCode::ProviderUnavailable,
        "Kiro CLI metadata request failed",
    )
}

fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::ProtocolIncompatible,
        "Kiro CLI returned an unexpected response",
    )
}

fn text(value: &Value, max: usize) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= max)
        .filter(|value| !value.chars().any(char::is_control))
        .map(str::to_owned)
}

fn evidence(client_version: Option<&str>) -> EvidenceSource {
    let mut source = EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        "kiro",
        "kiro-cli whoami --format json / chat --list-models --format json",
    );
    source.client_version = client_version.map(str::to_owned);
    source
}

/// A CLI version is the first `major.minor.patch` token in the output. Kiro
/// prints `kiro-cli 2.32.0`; a bare `v2.32.0` is accepted too.
pub(crate) fn parse_version(output: &str) -> Option<String> {
    output
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '.' || character == 'v')
        })
        .find_map(|token| {
            let token = token.strip_prefix('v').unwrap_or(token);
            let mut parts = token.split('.');
            let major = parts.next()?;
            let minor = parts.next()?;
            let patch = parts.next()?;
            if parts.next().is_some()
                || major.is_empty()
                || minor.is_empty()
                || patch.is_empty()
                || ![major, minor, patch]
                    .iter()
                    .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
            {
                return None;
            }
            Some(format!("{major}.{minor}.{patch}"))
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WhoAmI {
    pub authenticated: bool,
    pub signed_out: bool,
    pub account_type: Option<String>,
    pub email: Option<String>,
}

/// Reads `kiro-cli whoami --format json`. A signed-out CLI exits non-zero with
/// `{"account":null}`; any other failure is unknown rather than signed out.
pub(crate) fn parse_whoami(success: bool, output: &str) -> WhoAmI {
    let first_line = output.lines().next().unwrap_or("").trim();
    let value: Option<Value> = serde_json::from_str(first_line).ok();
    let account_type = value
        .as_ref()
        .and_then(|value| text(&value["accountType"], 64));
    let email = value.as_ref().and_then(|value| text(&value["email"], 256));
    let signed_out = !success
        && value
            .as_ref()
            .is_some_and(|value| value["account"].is_null());
    WhoAmI {
        authenticated: success && (account_type.is_some() || email.is_some()),
        signed_out,
        account_type,
        email,
    }
}

/// Maps `chat --list-models --format json` into native models. The list is the
/// account's own catalog; unknown fields are ignored and no model is invented.
pub(crate) fn parse_models(
    output: &str,
    route: &str,
    client_version: Option<&str>,
) -> Result<Vec<Model>> {
    let document: Value = serde_json::from_str(output.trim()).map_err(|_| invalid())?;
    let entries = document["models"]
        .as_array()
        .filter(|entries| entries.len() <= MAX_MODELS)
        .ok_or_else(invalid)?;
    let default = text(&document["default_model"], 256);
    let source = evidence(client_version);
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    for entry in entries {
        let native = text(&entry["model_id"], 256).ok_or_else(invalid)?;
        if !seen.insert(native.clone()) {
            return Err(invalid());
        }
        let name = text(&entry["model_name"], 256).unwrap_or_else(|| native.clone());
        let is_default = default.as_deref() == Some(native.as_str());
        let revision = stable_id(
            "catalog",
            &[
                route,
                &native,
                &name,
                if is_default { "default" } else { "listed" },
            ],
        );
        models.push(Model {
            id: route_model_key(route, &native),
            route_id: route.into(),
            native_id: native,
            name,
            revision,
            is_default,
            capabilities: vec![Capability {
                id: "generation.text".into(),
                support: Support::Supported,
                origin: CapabilityOrigin::Model,
                evidence: Observation::reported(
                    "Listed by the Kiro CLI account model catalog".into(),
                    source.clone(),
                ),
                conditions: vec![],
            }],
            options: vec![],
            context_window: Observation::unknown("KIRO_CONTEXT_WINDOW_NOT_REPORTED"),
        });
    }
    Ok(models)
}

fn spec(
    executable: &std::path::Path,
    profile_home: &std::path::Path,
    args: &[&str],
    api_key: Option<&str>,
) -> LaunchSpec {
    let mut env = std::collections::BTreeMap::new();
    if let Some(api_key) = api_key {
        env.insert("KIRO_API_KEY".into(), api_key.into());
    }
    LaunchSpec {
        executable: executable.to_path_buf(),
        args: args.iter().map(|arg| (*arg).into()).collect(),
        cwd: profile_home.to_path_buf(),
        env,
    }
}

fn api_key() -> Option<String> {
    std::env::var("KIRO_API_KEY")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub async fn probe(
    located: LocatedClient,
    connection: &str,
    route: &str,
) -> Result<ConnectionMetadata> {
    if located.provider_id != "kiro"
        || connection.is_empty()
        || route.is_empty()
        || !located.profile_home.is_absolute()
    {
        return Err(unavailable());
    }
    let executable = located
        .executable
        .clone()
        .ok_or_else(|| DomainError::new(ErrorCode::RuntimeMissing, "Kiro CLI is unavailable"))?;
    // A binary-only candidate may not have `~/.kiro` yet. Run from an existing
    // directory instead of creating or changing the profile.
    let cwd = if located.profile_home.is_dir() {
        located.profile_home.clone()
    } else {
        std::env::current_dir().map_err(|_| {
            DomainError::new(
                ErrorCode::RuntimeMissing,
                "Kiro working directory is unavailable",
            )
        })?
    };
    let key = api_key();
    let version = capture(
        spec(&executable, &cwd, &["--version"], key.as_deref()),
        VERSION_TIMEOUT,
    )
    .await
    .ok()
    .and_then(|output| parse_version(&output));
    let source = evidence(version.as_deref());

    let (success, whoami_output) = capture_status(
        spec(
            &executable,
            &cwd,
            &["whoami", "--format", "json"],
            key.as_deref(),
        ),
        AUTH_TIMEOUT,
    )
    .await
    .unwrap_or((false, String::new()));
    let whoami = parse_whoami(success, &whoami_output);
    let authenticated = whoami.authenticated || key.is_some();

    let mut report = ConnectionMetadata {
        connection_id: connection.into(),
        route_id: route.into(),
        version: version
            .clone()
            .map(|value| Observation::reported(value, source.clone()))
            .unwrap_or_else(|| Observation::unknown("CLIENT_VERSION_NOT_REPORTED")),
        auth: if authenticated {
            Observation::reported(AuthState::Authenticated, source.clone())
        } else if whoami.signed_out {
            Observation::reported(AuthState::Unauthenticated, source.clone())
        } else {
            Observation::unknown("KIRO_AUTH_NOT_REPORTED")
        },
        plan: Observation::unknown("KIRO_PLAN_NOT_REPORTED"),
        billing: Observation::unknown("KIRO_BILLING_NOT_REPORTED"),
        account: None,
        models: vec![],
        additional_routes: vec![],
        providers: vec![],
        quota_pools: vec![],
        included_usage_allowed: Observation::unknown("KIRO_USAGE_NOT_REPORTED"),
        diagnostics: vec![
            "KIRO_BILLING_NOT_REPORTED".into(),
            "KIRO_USAGE_NOT_REPORTED".into(),
        ],
    };
    if let Some(email) = whoami.email.clone() {
        let id = stable_id("account", &[connection, "kiro", &email]);
        report.account = Some(Account {
            id,
            provider_id: "kiro".into(),
            display_label: Observation::reported(email, source.clone()),
            identity_source: Observation::reported(
                "Connection-scoped kiro-cli whoami; no cross-client linking".into(),
                source.clone(),
            ),
            scopes: vec![],
        });
    }
    if authenticated {
        match capture(
            spec(
                &executable,
                &cwd,
                &["chat", "--list-models", "--format", "json"],
                key.as_deref(),
            ),
            CATALOG_TIMEOUT,
        )
        .await
        {
            Ok(output) => report.models = parse_models(&output, route, version.as_deref())?,
            Err(_) => report
                .diagnostics
                .push("KIRO_MODEL_CATALOG_UNAVAILABLE".into()),
        }
        report
            .diagnostics
            .push("KIRO_MODEL_CATALOG_FROM_CLIENT".into());
    } else {
        report
            .diagnostics
            .push("KIRO_MODEL_CATALOG_REQUIRES_SIGN_IN".into());
    }
    report.diagnostics.sort();
    report.diagnostics.dedup();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{parse_models, parse_version, parse_whoami};

    #[test]
    fn version_parsing_keeps_the_numeric_release() {
        assert_eq!(
            parse_version("kiro-cli 2.32.0\n").as_deref(),
            Some("2.32.0")
        );
        assert_eq!(parse_version("Kiro CLI v2.27.1").as_deref(), Some("2.27.1"));
        assert_eq!(parse_version("kiro-cli 2.32").as_deref(), None);
        assert_eq!(parse_version("kiro-cli").as_deref(), None);
    }

    #[test]
    fn signed_out_whoami_is_unauthenticated_not_unknown() {
        let value = parse_whoami(false, "{\"account\":null}\n");
        assert!(!value.authenticated);
        assert_eq!(value.account_type, None);
        let other = parse_whoami(false, "error: service error");
        assert!(!other.authenticated);
    }

    #[test]
    fn identity_center_whoami_keeps_the_account_email() {
        let output = [
            "{\"accountType\":\"IamIdentityCenter\",\"email\":\"dev@example.test\",\"region\":\"us-east-1\"}",
            "",
            "Profile:",
            "KiroProfile-us-east-1",
        ]
        .join("\n");
        let value = parse_whoami(true, &output);
        assert!(value.authenticated);
        assert_eq!(value.email.as_deref(), Some("dev@example.test"));
        assert_eq!(value.account_type.as_deref(), Some("IamIdentityCenter"));
    }

    #[test]
    fn model_catalog_preserves_native_ids_and_the_reported_default() {
        let models = parse_models(
            r#"{"models":[{"model_id":"claude-sonnet-4.6","model_name":"Claude Sonnet 4.6"},{"model_id":"auto"}],"default_model":"auto"}"#,
            "route",
            Some("2.32.0"),
        )
        .unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].native_id, "claude-sonnet-4.6");
        assert_eq!(models[1].name, "auto");
        assert!(models[1].is_default);
        assert!(!models[0].is_default);
    }

    #[test]
    fn duplicate_or_malformed_models_fail_closed() {
        for output in [
            r#"{"models":[{"model_id":"a"},{"model_id":"a"}]}"#,
            r#"{"models":[{"model_name":"missing id"}]}"#,
            r#"{"models":"not-an-array"}"#,
            r#"{"models":[{"model_id":""}]}"#,
        ] {
            assert!(parse_models(output, "route", None).is_err(), "{output}");
        }
    }
}
