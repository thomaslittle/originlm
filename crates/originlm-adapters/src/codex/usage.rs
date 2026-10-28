use super::{invalid, text};
use originlm_contracts::*;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn parse_usage(
    connection: &str,
    route: &str,
    account: Option<String>,
    root: &Value,
    source: &EvidenceSource,
) -> Result<Vec<QuotaPool>> {
    let mut buckets = Vec::new();
    if let Some(all) = root["rateLimitsByLimitId"]
        .as_object()
        .filter(|m| !m.is_empty())
    {
        if all.len() > 128 {
            return Err(invalid());
        }
        for (key, value) in all {
            buckets.push((key.as_str(), value));
        }
    } else if let Some(single) = root.get("rateLimits").filter(|v| v.is_object()) {
        buckets.push((text(&single["limitId"]).unwrap_or("codex"), single));
    } else {
        return Err(invalid());
    }
    let mut pools = Vec::new();
    let mut seen = BTreeSet::new();
    for (fallback, bucket) in buckets {
        let native_id = text(&bucket["limitId"]).unwrap_or(fallback);
        if native_id.len() > 512 || !seen.insert(native_id) {
            return Err(invalid());
        }
        let pool_id = stable_id(
            "quota",
            &[
                connection,
                account.as_deref().unwrap_or("no-account"),
                native_id,
            ],
        );
        let mut windows = Vec::new();
        for key in ["primary", "secondary"] {
            let Some(window) = bucket.get(key).filter(|v| !v.is_null()) else {
                continue;
            };
            let percent = window["usedPercent"]
                .as_u64()
                .filter(|v| *v <= 10_000)
                .ok_or_else(invalid)?;
            let reading = UsageReading::try_from(UsageInput {
                unit: "percent".into(),
                used: DecimalString::try_from(percent.to_string()).map_err(|_| invalid())?,
                limit: Some(DecimalString::try_from("100".to_owned()).map_err(|_| invalid())?),
            })
            .map_err(|_| invalid())?;
            let resets_at = window["resetsAt"]
                .as_i64()
                .and_then(|time| chrono::DateTime::from_timestamp(time, 0))
                .and_then(|dt| Timestamp::try_from(dt.to_rfc3339()).ok());
            let duration_seconds = window["windowDurationMins"]
                .as_u64()
                .and_then(|mins| mins.checked_mul(60))
                .and_then(|secs| u32::try_from(secs).ok());
            windows.push(UsageWindow {
                id: stable_id("window", &[&pool_id, key]),
                label: format!("{} · {key}", text(&bucket["limitName"]).unwrap_or("Codex")),
                reading: Observation::reported(reading, source.clone()),
                resets_at,
                duration_seconds,
            });
        }
        // Enterprise monthly credits are a separate allowance. They are reported
        // only when Codex includes `individualLimit`. Reimplemented from T3 Code
        // `codexRateLimitsToWindows` at
        // [4c18faf62c](https://github.com/pingdotgg/t3code/commit/4c18faf62c).
        if (native_id == "codex" || fallback == "codex")
            && let Some(used) = individual_credit_used_percent(bucket.get("individualLimit"))
        {
            let reading = UsageReading::try_from(UsageInput {
                unit: "percent".into(),
                used: DecimalString::try_from(used).map_err(|_| invalid())?,
                limit: Some(DecimalString::try_from("100".to_owned()).map_err(|_| invalid())?),
            })
            .map_err(|_| invalid())?;
            let resets_at = bucket["individualLimit"]["resetsAt"]
                .as_i64()
                .and_then(|time| chrono::DateTime::from_timestamp(time, 0))
                .and_then(|dt| Timestamp::try_from(dt.to_rfc3339()).ok());
            windows.push(UsageWindow {
                id: stable_id("window", &[&pool_id, "individual"]),
                label: "Monthly credit limit".into(),
                reading: Observation::reported(reading, source.clone()),
                resets_at,
                duration_seconds: None,
            });
        }
        // Percent windows are not token estimates. Credits and spend controls need their own units.
        pools.push(QuotaPool {
            id: pool_id,
            account_id: account.clone(),
            connection_ids: vec![connection.into()],
            route_ids: vec![route.into()],
            sharing: "unknown".into(),
            windows,
        });
    }
    Ok(pools)
}

/// `remainingPercent` is what is left of the monthly credit allowance.
/// A missing or non-finite value is not a zero balance.
fn individual_credit_used_percent(limit: Option<&Value>) -> Option<String> {
    let remaining = limit?.get("remainingPercent")?.as_f64()?;
    if !remaining.is_finite() || !(0.0..=100.0).contains(&remaining) {
        return None;
    }
    Some(percent_decimal(100.0 - remaining))
}

fn percent_decimal(value: f64) -> String {
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".into()
    } else {
        trimmed.to_owned()
    }
}
