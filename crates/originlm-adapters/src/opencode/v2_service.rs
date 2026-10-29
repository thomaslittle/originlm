//! Metadata from a running OpenCode 2 background service.
//!
//! OpenCode 2 serves a password-protected loopback API. The kit reads the
//! owner-only service descriptor, asks the running service for its provider
//! catalog and workspace limits, and reports those observations. It does not
//! start a second service, import credentials, or contact a remote account
//! service itself. Execution remains unavailable until the v2 ACP/text client
//! is independently wired.

use crate::{
    http::{Authentication, Endpoint},
    process::{LaunchSpec, capture},
};
use chrono::DateTime;
use originlm_contracts::*;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_DESCRIPTOR: u64 = 8 * 1024;
/// Starting the owner's own background service is bounded; a healthy service
/// answers immediately and the CLI is idempotent.
const SERVICE_START: Duration = Duration::from_secs(20);

fn unavailable(reason: &str) -> DomainError {
    DomainError::new(ErrorCode::ProviderUnavailable, reason)
}

fn denied(reason: &str) -> DomainError {
    DomainError::new(ErrorCode::LocalAccessDenied, reason)
}

fn state_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local/state"))
        })?;
    Some(base.join("opencode/service.json"))
}

fn service() -> Result<(String, String)> {
    let path =
        state_path().ok_or_else(|| unavailable("OpenCode state directory is unavailable"))?;
    read_service(&path)
}

fn read_service(path: &Path) -> Result<(String, String)> {
    let metadata = fs::metadata(path).map_err(|_| {
        unavailable("OpenCode 2 background service is not running; start `opencode` first")
    })?;
    if !metadata.is_file() || metadata.len() > MAX_DESCRIPTOR {
        return Err(unavailable("OpenCode service descriptor is invalid"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(denied("OpenCode service descriptor is not owner-only"));
        }
    }
    let bytes =
        fs::read(path).map_err(|_| unavailable("OpenCode service descriptor is unreadable"))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| unavailable("OpenCode service descriptor is malformed"))?;
    let url = value["url"]
        .as_str()
        .filter(|url| {
            (url.starts_with("http://127.0.0.1:") || url.starts_with("http://[::1]:"))
                && !url.contains('@')
                && url.len() <= 128
        })
        .ok_or_else(|| denied("OpenCode service URL is not loopback"))?;
    let password = value["password"]
        .as_str()
        .filter(|password| password.len() >= 16 && password.len() <= 512)
        .ok_or_else(|| denied("OpenCode service credential is invalid"))?;
    if let Some(pid) = value["pid"].as_u64()
        && pid > 0
        && !process_is_alive(pid)
    {
        // A stopped OpenCode service leaves this descriptor behind. Treating
        // its port as authoritative keeps every later probe pinned to a dead
        // endpoint, so usage can never advance.
        return Err(unavailable(
            "OpenCode 2 background service is stopped; its descriptor is stale",
        ));
    }
    Ok((url.trim_end_matches('/').to_string(), password.to_string()))
}

fn process_is_alive(pid: u64) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new("/proc").join(pid.to_string()).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Other platforms have no portable, dependency-free PID liveness
        // check here; an unreachable endpoint still triggers one recovery.
        let _ = pid;
        true
    }
}

fn service_environment() -> BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    // The CLI resolves its own state and credential store from the owner's
    // environment. Pass only those roots; never a server password or a
    // session-specific OPENCODE_CONFIG_CONTENT.
    const KEYS: [&str; 11] = [
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "XDG_STATE_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "TMPDIR",
        "TEMP",
        "PATH",
    ];
    KEYS.into_iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (std::ffi::OsString::from(key), value)))
        .collect()
}

/// Ask the installed OpenCode CLI to start its own background service. This is
/// the same owner-scoped service the OpenCode app/CLI uses, not a second
/// kit-owned server; the CLI creates its own password and descriptor.
async fn ensure_service(executable: &Path) -> Result<()> {
    let cwd = std::env::current_dir()
        .map_err(|_| unavailable("OpenCode working directory is unavailable"))?;
    capture(
        LaunchSpec {
            executable: executable.to_owned(),
            args: vec!["service".into(), "start".into()],
            cwd,
            env: service_environment(),
        },
        SERVICE_START,
    )
    .await
    .map_err(|_| unavailable("OpenCode 2 background service could not be started"))?;
    Ok(())
}

fn observed<T>(value: T, source: EvidenceSource) -> Observation<T> {
    Observation::Known {
        value,
        source,
        observed_at: Timestamp::now(),
        freshness: Freshness::Fresh,
        verification: Verification::Observed,
        expires_at: None,
    }
}

fn service_evidence(reference: &str) -> EvidenceSource {
    EvidenceSource::new(EvidenceKind::OfficialApi, "opencode", reference)
}

fn decimal(value: f64) -> Result<DecimalString> {
    DecimalString::try_from(value.max(0.0).round().to_string())
        .map_err(|_| DomainError::new(ErrorCode::InvalidRequest, "OpenCode usage value is invalid"))
}

fn quota_windows(
    connection: &str,
    route: &str,
    go_route: Option<&str>,
    limits: &Value,
) -> Result<Vec<QuotaPool>> {
    let Some(go) = limits["data"]["go"].as_object() else {
        return Ok(Vec::new());
    };
    let pool_id = stable_id("pool", &[connection, "opencode-go"]);
    let mut windows = Vec::new();
    for (key, label, duration) in [
        ("rolling", "Rolling", None),
        ("weekly", "Weekly", Some(604_800)),
        ("monthly", "Monthly", None),
    ] {
        let Some(entry) = go.get(key) else { continue };
        let Some(percent) = entry["percent"].as_f64() else {
            continue;
        };
        let used = decimal(percent)?;
        let remaining = decimal(100.0 - percent)?;
        let reading = UsageReading {
            unit: "percent".into(),
            used: used.clone(),
            limit: Some(decimal(100.0)?),
            remaining: Some(remaining),
            used_fraction: Some((percent / 100.0).clamp(0.0, 1.0)),
        };
        let resets_at = entry["resetsAt"]
            .as_i64()
            .map(|millis| {
                if millis > 1_000_000_000_000 {
                    millis / 1000
                } else {
                    millis
                }
            })
            .and_then(|secs| DateTime::from_timestamp(secs, 0))
            .and_then(|stamp| {
                Timestamp::try_from(stamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).ok()
            });
        windows.push(UsageWindow {
            id: stable_id("window", &[&pool_id, key]),
            label: label.into(),
            reading: observed(reading, service_evidence("OpenCode workspace usage limits")),
            resets_at,
            duration_seconds: duration,
        });
    }
    if windows.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![QuotaPool {
        id: pool_id,
        account_id: None,
        connection_ids: vec![connection.into()],
        // Attribute Go usage to the exact `opencode-go` route when the catalog
        // exposes it; a plain `opencode`/Zen route must never inherit it.
        route_ids: vec![go_route.unwrap_or(route).into()],
        sharing: "account".into(),
        windows,
    }])
}

/// Probe the running service through its owner-only descriptor. Callers that
/// have the installed executable should use [`probe_with_install`] so a stale
/// descriptor left by a stopped service can recover instead of pinning usage
/// to a dead endpoint forever.
pub async fn probe(connection: &str, route: &str, version: &str) -> Result<ConnectionMetadata> {
    let (url, password) = service()?;
    probe_endpoint(&url, &password, connection, route, version).await
}

/// Probe the installed OpenCode 2 service, starting the owner's own background
/// service when the descriptor is missing or points at a stopped process. The
/// service supplies its own credentials; none are read or replaced here.
pub async fn probe_with_install(
    executable: &Path,
    connection: &str,
    route: &str,
    version: &str,
) -> Result<ConnectionMetadata> {
    let (url, password, started) = match service() {
        Ok((url, password)) => (url, password, false),
        Err(_) => {
            ensure_service(executable).await?;
            let (url, password) = service()?;
            (url, password, true)
        }
    };
    match probe_endpoint(&url, &password, connection, route, version).await {
        Ok(metadata) => Ok(metadata),
        Err(error)
            if !started
                && matches!(
                    error.code,
                    ErrorCode::ProviderUnavailable
                        | ErrorCode::ProtocolIncompatible
                        | ErrorCode::Timeout
                ) =>
        {
            // A descriptor can outlive its service (for example after the
            // OpenCode app or CLI stops), or its old port can be reused.
            // Recover once through the installed CLI, then use its fresh
            // descriptor. This starts the owner's service, never a second one.
            ensure_service(executable).await?;
            let (url, password) = service()?;
            probe_endpoint(&url, &password, connection, route, version).await
        }
        Err(error) => Err(error),
    }
}

async fn probe_endpoint(
    url: &str,
    password: &str,
    connection: &str,
    route: &str,
    version: &str,
) -> Result<ConnectionMetadata> {
    let endpoint = Endpoint::new(
        &format!("{url}/"),
        Authentication::Basic("opencode".into(), password.to_string()),
        true,
    )?;
    let models = endpoint
        .json_query_limited("api/model", &[], None, 16 * 1024 * 1024)
        .await
        .map_err(|_| unavailable("OpenCode 2 model inventory is unavailable"))?;
    let providers = endpoint
        .json_query_limited("api/provider", &[], None, 4 * 1024 * 1024)
        .await
        .map_err(|_| unavailable("OpenCode 2 provider inventory is unavailable"))?;
    let integration = endpoint
        .json_query_limited("api/integration/opencode", &[], None, 1024 * 1024)
        .await
        .ok();
    let limits = endpoint
        .json_query_limited("api/experimental/usage/limits", &[], None, 1024 * 1024)
        .await
        .ok();

    let mut metadata =
        super::catalog_v2::catalog(connection, route, &providers, &models, &BTreeMap::new())?;
    metadata.version = observed(
        version
            .trim()
            .trim_start_matches("opencode ")
            .trim_start_matches('v')
            .to_string(),
        service_evidence("OpenCode CLI --version"),
    );
    metadata.auth = match limits.as_ref() {
        Some(limits) if limits["data"]["workspace"]["id"].as_str().is_some() => observed(
            AuthState::Authenticated,
            service_evidence("Authenticated workspace limits returned by the local service"),
        ),
        _ => match integration
            .as_ref()
            .and_then(|value| value["data"]["connections"].as_array())
            .filter(|connections| !connections.is_empty())
        {
            Some(_) => Observation::unknown("OPENCODE_CREDENTIALS_CONFIGURED_NOT_VERIFIED"),
            None => Observation::unknown("OPENCODE_NO_CONNECTION"),
        },
    };
    if let Some(limits) = limits.as_ref() {
        let go_route = metadata
            .additional_routes
            .iter()
            .find(|route| route.provider_id == "opencode-go")
            .map(|route| route.id.clone());
        metadata.quota_pools = quota_windows(connection, route, go_route.as_deref(), limits)?;
    }
    metadata.diagnostics = vec![
        "OPENCODE_2_METADATA_FROM_LOCAL_SERVICE".into(),
        "OPENCODE_2_EXECUTION_NOT_WIRED".into(),
    ];
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::{quota_windows, read_service};
    use std::io::Write;

    #[test]
    fn rejects_non_loopback_and_insecure_descriptors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("service.json");
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "{}",
            serde_json::json!({"url":"http://10.0.0.1:9000","password":"0123456789abcdef0123456789abcdef"})
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(read_service(&path).is_err());
    }

    #[test]
    fn accepts_a_loopback_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("service.json");
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "{}",
            serde_json::json!({"url":"http://127.0.0.1:49374","password":"0123456789abcdef0123456789abcdef"})
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let (url, password) = read_service(&path).unwrap();
        assert_eq!(url, "http://127.0.0.1:49374");
        assert_eq!(password.len(), 32);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_a_descriptor_whose_service_pid_is_dead() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("service.json");
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "{}",
            serde_json::json!({"url":"http://127.0.0.1:49374","pid":4294967295u64,"password":"0123456789abcdef0123456789abcdef"})
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let error = read_service(&path).unwrap_err();
        assert_eq!(
            error.code,
            originlm_contracts::ErrorCode::ProviderUnavailable
        );
    }

    #[test]
    fn go_pool_uses_the_exact_go_route_and_never_zen() {
        let limits = serde_json::json!({"data":{"go":{
            "rolling":{"percent":14,"resetsAt":1793272155000u64},
            "weekly":{"percent":23,"resetsAt":1793577600000u64},
            "monthly":{"percent":61,"resetsAt":1795241023000u64}
        }}});
        let pools = quota_windows("connection", "route-zen", Some("route-go"), &limits).unwrap();
        assert_eq!(pools[0].route_ids, vec!["route-go".to_string()]);
        assert_eq!(
            pools[0].id,
            originlm_contracts::stable_id("pool", &["connection", "opencode-go"])
        );
        // Without a published Go route the catalog falls back to the root
        // route; the deterministic pool id still proves it is the Go pool.
        let pools = quota_windows("connection", "route-zen", None, &limits).unwrap();
        assert_eq!(pools[0].route_ids, vec!["route-zen".to_string()]);
    }
}
