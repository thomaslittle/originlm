use super::{invalid, text};
use originlm_contracts::*;
use serde_json::Value;
pub(super) fn parse(
    connection: &str,
    route: &str,
    account: Option<String>,
    usage: &Value,
    source: &EvidenceSource,
) -> Result<Vec<QuotaPool>> {
    if usage["rate_limits_available"] != true {
        return Ok(vec![]);
    }
    let rates = &usage["rate_limits"];
    if !rates.is_object() {
        return Err(invalid());
    }
    // (id, label, duration in seconds when the key states one, reading)
    let mut rows: Vec<(String, String, Option<u32>, Value)> = vec![];
    if let Some(limits) = rates["limits"].as_array() {
        if limits.len() > 128 {
            return Err(invalid());
        }
        for (index, limit) in limits.iter().enumerate() {
            let kind = text(&limit["kind"]).ok_or_else(invalid)?;
            let scope = text(&limit["scope"]["model"]["display_name"]);
            // The id keeps the provider's kind, so a consumer can still tell
            // a scoped window from an account-wide one without reading the
            // label.
            rows.push((
                format!("{kind}-{index}"),
                limit_label(kind, scope),
                limit_duration(kind),
                serde_json::json!({"percent":limit["percent"],"resetsAt":limit["resets_at"]}),
            ));
        }
    } else {
        for (key, value) in rates.as_object().ok_or_else(invalid)? {
            if key == "spend" || key == "extra_usage" {
                continue;
            }
            if value.is_object() && !value["utilization"].is_null() {
                rows.push((
                    key.clone(),
                    window_label(key),
                    window_duration(key),
                    serde_json::json!({"percent":value["utilization"],"resetsAt":value["resets_at"]}),
                ));
            }
        }
    }
    let mut windows = vec![];
    for (id, label, duration_seconds, row) in rows {
        let used = row["percent"]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1000.0)
            .ok_or_else(invalid)?;
        let reading = UsageReading::try_from(UsageInput {
            unit: "percent".into(),
            used: DecimalString::try_from(used.to_string()).map_err(|_| invalid())?,
            limit: Some(DecimalString::try_from("100".to_owned()).map_err(|_| invalid())?),
        })
        .map_err(|_| invalid())?;
        let reset = text(&row["resetsAt"])
            .map(|s| Timestamp::try_from(s.to_owned()).map_err(|_| invalid()))
            .transpose()?;
        windows.push(UsageWindow {
            id,
            label,
            reading: Observation::reported(reading, source.clone()),
            resets_at: reset,
            duration_seconds,
        });
    }
    // Monthly account spend is separate from the rolling session windows.
    // Prefer the enabled spend budget; otherwise use an enabled extra-usage
    // allowance. Reimplemented from T3 Code `readSpendPercent` at
    // [fd8dd51654](https://github.com/pingdotgg/t3code/commit/fd8dd51654).
    if let Some(used) = monthly_spend_percent(rates) {
        let reading = UsageReading::try_from(UsageInput {
            unit: "percent".into(),
            used: DecimalString::try_from(used).map_err(|_| invalid())?,
            limit: Some(DecimalString::try_from("100".to_owned()).map_err(|_| invalid())?),
        })
        .map_err(|_| invalid())?;
        windows.push(UsageWindow {
            id: "monthly_spend".into(),
            label: "Monthly spend".into(),
            reading: Observation::reported(reading, source.clone()),
            resets_at: None,
            duration_seconds: None,
        });
    }
    if windows.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![QuotaPool {
        id: stable_id("quota", &[connection, "claude-native-usage"]),
        account_id: account,
        connection_ids: vec![connection.into()],
        route_ids: vec![route.into()],
        sharing: "unknown".into(),
        windows,
    }])
}

/// Claude Code's usage payload uses stable bucket keys and a few undocumented
/// codenames. Keep the provider key visible for undocumented buckets instead
/// of inventing a friendly name that could be mistaken for provider evidence.
fn window_label(key: &str) -> String {
    match key {
        "five_hour" => "5-hour session".into(),
        "seven_day" => "7-day".into(),
        "seven_day_sonnet" => "7-day Sonnet".into(),
        "seven_day_opus" => "7-day Opus".into(),
        // `omelette` is an undocumented codename; do not present it as a
        // documented Opus bucket without keeping the raw key visible.
        "seven_day_omelette" => "7-day Opus (seven_day_omelette)".into(),
        "seven_day_oauth_apps" => "7-day OAuth apps".into(),
        "seven_day_cowork" => "7-day Cowork".into(),
        // Anthropic does not document this bucket. Community usage readers
        // consistently treat it as a cloud-credit bucket; retain the raw key.
        "iguana_necktie" => "Cloud credit (iguana_necktie)".into(),
        "tangelo" => "Tangelo credit (tangelo)".into(),
        "omelette_promotional" => "Promotional credit (omelette_promotional)".into(),
        other => format!("{other} (undocumented)"),
    }
}

const FIVE_HOURS: u32 = 5 * 60 * 60;
const SEVEN_DAYS: u32 = 7 * 24 * 60 * 60;

/// A window's length, only where the provider's own key states it. Credit
/// and codename buckets say nothing about a length and stay unknown.
fn window_duration(key: &str) -> Option<u32> {
    match key {
        "five_hour" => Some(FIVE_HOURS),
        key if key == "seven_day" || key.starts_with("seven_day_") => Some(SEVEN_DAYS),
        _ => None,
    }
}

/// Labels for the normalized `limits` list. `kind` is the provider's own
/// word; a kind this adapter does not know keeps that word visible, like an
/// undocumented bucket key.
fn limit_label(kind: &str, scope: Option<&str>) -> String {
    let name = match kind {
        "session" => "Session".to_owned(),
        "weekly_all" => "Weekly".to_owned(),
        // Limits one model family; the scope names it.
        "weekly_scoped" => match scope {
            Some(scope) => return format!("Weekly · {scope}"),
            None => return "Weekly (scoped)".to_owned(),
        },
        other => format!("{other} (undocumented)"),
    };
    match scope {
        Some(scope) => format!("{name} · {scope}"),
        None => name,
    }
}

/// `weekly_*` states a week. `session` states no length, so it stays
/// unknown rather than assumed to be five hours.
fn limit_duration(kind: &str) -> Option<u32> {
    (kind == "weekly_all" || kind == "weekly_scoped").then_some(SEVEN_DAYS)
}

fn amount_percent(used: &Value, limit: &Value) -> Option<f64> {
    let used = used
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)?;
    let limit = limit
        .as_f64()
        .filter(|value| value.is_finite() && *value > 0.0)?;
    let percent = (used / limit) * 100.0;
    percent.is_finite().then_some(percent.min(100.0))
}

fn monthly_spend_percent(rates: &Value) -> Option<String> {
    let spend = &rates["spend"];
    if spend["enabled"] == true {
        let used = &spend["used"];
        let limit = &spend["limit"];
        let currency = text(&used["currency"]).map(str::trim);
        let same_currency = currency.is_some_and(|value| !value.is_empty())
            && currency == text(&limit["currency"]).map(str::trim);
        let same_scale =
            used["exponent"].as_i64().is_some() && used["exponent"] == limit["exponent"];
        if same_currency
            && same_scale
            && let Some(percent) = amount_percent(&used["amount_minor"], &limit["amount_minor"])
        {
            return Some(percent_decimal(percent));
        }
    }
    let extra = &rates["extra_usage"];
    if extra["is_enabled"] != true {
        return None;
    }
    let percent =
        amount_percent(&extra["used_credits"], &extra["monthly_limit"]).or_else(|| {
            extra["utilization"]
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| value.min(100.0))
        })?;
    Some(percent_decimal(percent))
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
