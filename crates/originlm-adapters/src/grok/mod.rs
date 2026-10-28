//! Grok native status and ACP catalog; no credential-file access or token export.
use crate::{
    acp::metadata::{self, AccountEvidence},
    process::{LaunchSpec, capture},
};
use originlm_contracts::*;
use originlm_discovery::LocatedClient;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};
pub fn parse_status(output: &str) -> (Observation<AuthState>, Observation<BillingMode>) {
    let source = EvidenceSource::new(
        EvidenceKind::OfficialProtocol,
        "grok",
        "grok models login status",
    );
    let lower = output.to_ascii_lowercase();
    let auth = if lower.contains("not authenticated") || lower.contains("not logged in") {
        Observation::reported(AuthState::Unauthenticated, source)
    } else if lower
        .lines()
        .any(|line| line.trim() == "you are logged in with grok.com.")
    {
        Observation::reported(AuthState::Authenticated, source)
    } else {
        Observation::unknown("NATIVE_AUTH_STATUS_NOT_REPORTED")
    };
    (
        auth,
        Observation::unknown("NATIVE_MODEL_ROUTE_BILLING_NOT_REPORTED"),
    )
}
/// An expired access token prints the logged-out line, then the CLI refreshes
/// it in that process. One more `grok models` after that process exits tells a
/// refresh from a real logout. Reimplemented from T3 Code
/// `packages/provider-grok/src/server/status.ts` at
/// [a2157eced7](https://github.com/pingdotgg/t3code/commit/a2157eced7).
/// An unknown recheck keeps the logged-out result.
pub fn login_recheck_replaces(
    first: &Observation<AuthState>,
    recheck: &Observation<AuthState>,
) -> bool {
    first.fresh_value() == Some(&AuthState::Unauthenticated) && recheck.fresh_value().is_some()
}
fn invalid() -> DomainError {
    DomainError::new(
        ErrorCode::IntegrationRestricted,
        "Grok requires its supported native executable and default local profile",
    )
}
/// Environment for `grok models` and the ACP initialize that follows it.
///
/// Both are health probes. They set `GROK_DISABLE_AUTOUPDATER=1` so the check
/// does not start Grok's updater. Text sessions do not use this environment.
pub fn probe_environment() -> BTreeMap<OsString, OsString> {
    let mut env = BTreeMap::new();
    crate::native_clients::launch::apply_grok_probe_environment(&mut env);
    env
}
/// The supported Grok executable for the default local profile, or an error.
pub fn supported_executable(located: &LocatedClient) -> Result<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(invalid)?;
    if located.profile_home != home.join(".grok") {
        return Err(invalid());
    }
    let path = located
        .executable
        .as_ref()
        .filter(|p| p.is_absolute() && p.is_file())
        .ok_or_else(invalid)?;
    if cfg!(windows) && path.extension().and_then(|s| s.to_str()) != Some("exe") {
        return Err(invalid());
    }
    Ok(path.clone())
}
pub async fn probe(
    located: LocatedClient,
    connection: String,
    route: String,
) -> Result<ConnectionMetadata> {
    let executable = supported_executable(&located)?;
    let temp = tempfile::tempdir().map_err(|_| invalid())?;
    let env = probe_environment();
    let spec = LaunchSpec {
        executable: executable.clone(),
        args: vec!["models".into()],
        cwd: temp.path().into(),
        env: env.clone(),
    };
    let output = capture(spec, Duration::from_secs(8)).await?;
    let (mut auth, mut billing) = parse_status(&output);
    if auth.fresh_value() == Some(&AuthState::Unauthenticated) {
        let again = capture(
            LaunchSpec {
                executable: executable.clone(),
                args: vec!["models".into()],
                cwd: temp.path().into(),
                env: env.clone(),
            },
            Duration::from_secs(8),
        )
        .await?;
        let (recheck_auth, recheck_billing) = parse_status(&again);
        if login_recheck_replaces(&auth, &recheck_auth) {
            auth = recheck_auth;
            billing = recheck_billing;
        }
    }
    if auth.fresh_value() != Some(&AuthState::Authenticated) {
        return Ok(ConnectionMetadata {
            connection_id: connection,
            route_id: route,
            version: Observation::unknown("NOT_PROBED"),
            auth,
            billing,
            plan: Observation::unknown("PLAN_NOT_REPORTED"),
            account: None,
            models: vec![],
            additional_routes: vec![],
            providers: vec![],
            quota_pools: vec![],
            included_usage_allowed: Observation::unknown("NOT_REPORTED"),
            diagnostics: vec!["NATIVE_AUTH_REQUIRED_OR_UNKNOWN".into()],
        });
    }
    let spec = LaunchSpec {
        executable,
        args: crate::native_clients::launch::GROK_TEXT_ONLY_ARGS
            .iter()
            .map(std::ffi::OsString::from)
            .chain([
                "--leader-socket".into(),
                temp.path().join("leader.sock").into(),
                "agent".into(),
                "stdio".into(),
            ])
            .collect(),
        cwd: temp.path().into(),
        env,
    };
    metadata::probe(
        spec,
        "grok",
        &connection,
        &route,
        AccountEvidence {
            auth,
            billing,
            plan: Observation::unknown("PLAN_NOT_REPORTED_BY_NATIVE_PROTOCOL"),
        },
    )
    .await
}
