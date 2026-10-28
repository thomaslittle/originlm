//! Metadata from a running OpenCode 2 background service.
//!
//! OpenCode 2 serves a password-protected loopback API. The kit reads the
//! owner-only service descriptor, asks the running service for its provider
//! catalog and workspace limits, and reports those observations. It does not
//! start a second service, import credentials, or contact a remote account
//! service itself. Execution remains unavailable until the v2 ACP/text client
//! is independently wired.

use crate::http::{Authentication, Endpoint};
use chrono::DateTime;
use originlm_contracts::*;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const MAX_DESCRIPTOR: u64 = 8 * 1024;

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
    Ok((url.trim_end_matches('/').to_string(), password.to_string()))
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

fn quota_windows(connection: &str, route: &str, limits: &Value) -> Result<Vec<QuotaPool>> {
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
        route_ids: vec![route.into()],
        sharing: "account".into(),
        windows,
    }])
}

/// Probe the running service. `connection` and `route` are the discovered
/// OpenCode ids; `version` is the CLI's reported version string.
pub async fn probe(connection: &str, route: &str, version: &str) -> Result<ConnectionMetadata> {
    let (url, password) = service()?;
    let endpoint = Endpoint::new(
        &format!("{url}/"),
        Authentication::Basic("opencode".into(), password),
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
        metadata.quota_pools = quota_windows(connection, route, limits)?;
    }
    metadata.diagnostics = vec![
        "OPENCODE_2_METADATA_FROM_LOCAL_SERVICE".into(),
        "OPENCODE_2_EXECUTION_NOT_WIRED".into(),
    ];
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::read_service;
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
}
