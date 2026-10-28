//! Human wording for engine codes. The broker speaks in stable identifiers and
//! diagnostic codes; a person should not have to read them.

pub fn diagnostic(code: &str) -> Option<&'static str> {
    Some(match code {
        "CONFIGURATION_DIRECTORY_PRESENT" => {
            "Its configuration is present; metadata has not been read yet."
        }
        "RESET_CREDITS_NOT_ACTIONABLE_BY_TOOLKIT" => {
            "Credits can only be reset from the provider's own client."
        }
        "UNVERIFIED_LOCAL_CACHE_ONLY" => {
            "Read from Cursor's local cache; the session is not verified."
        }
        "IMMUTABLE_DATABASE_EXCLUDES_LIVE_WAL_UPDATES" => {
            "The last on-disk snapshot is shown; newer Cursor writes may not appear."
        }
        "NO_SUBSCRIPTION_TOKEN_OR_PRIVATE_ENDPOINT_USED" => {
            "No subscription token or private endpoint was used."
        }
        "OPENCODE_2_METADATA_FROM_LOCAL_SERVICE" => "Read from OpenCode's running local service.",
        "OPENCODE_2_EXECUTION_NOT_WIRED" => {
            "OpenCode 2 is catalogued here; text execution is not available yet."
        }
        "OPENCODE_CREDENTIALS_CONFIGURED_NOT_VERIFIED" => {
            "An OpenCode connection is configured, but the service did not confirm it."
        }
        "OPENCODE_NO_CONNECTION" => "OpenCode has no configured connection.",
        "NATIVE_VERSION_NOT_PROBED" => "The installed client version was not probed.",
        "CACHED_PLAN_DOES_NOT_VERIFY_BILLING" => {
            "The plan is from a local cache and is not billing verification."
        }
        "CACHED_ACCOUNT_DOES_NOT_VERIFY_AUTHENTICATION" => {
            "A cached account exists; it does not prove the session is valid."
        }
        "CURSOR_QUOTA_NOT_READ" => "Cursor does not expose quota locally; check its own dashboard.",
        "GATEWAY_PLAN_AND_USAGE_UNAVAILABLE" => {
            "The gateway does not report a plan or usage for this connection."
        }
        "UPSTREAM_ACCESS_NOT_INDEPENDENTLY_VALIDATED" => {
            "Upstream access was not independently validated."
        }
        "PLAN_NOT_REPORTED" => "This provider does not report a plan.",
        "NOT_REPORTED" => "This provider did not report the value.",
        "METADATA_STALE" => "The last metadata check is stale.",
        _ => return None,
    })
}

/// A short sentence for a code, or a readable fallback for an unknown one.
pub fn describe_diagnostic(code: &str) -> String {
    diagnostic(code)
        .map(str::to_owned)
        .unwrap_or_else(|| readable_code(code))
}

fn readable_code(code: &str) -> String {
    let words = code
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if matches!(lower.as_str(), "id" | "api" | "cli" | "url" | "ui") {
                lower.to_ascii_uppercase()
            } else {
                lower
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if words.is_empty() {
        "No further detail was reported.".into()
    } else {
        let mut chars = words.chars();
        match chars.next() {
            Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
            None => words,
        }
    }
}

/// Provider-prefixed names often repeat the provider ("Claude Code · Default
/// profile"). Keep the distinguishing part for rows that already show the
/// provider.
pub fn without_provider_prefix(name: &str, provider: &str) -> String {
    let trimmed = name.trim();
    for separator in [" · ", " - ", " — ", ": "] {
        if let Some(rest) = trimmed.strip_prefix(&format!("{provider}{separator}"))
            && !rest.is_empty()
        {
            return rest.to_string();
        }
    }
    trimmed.to_string()
}

pub fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}
