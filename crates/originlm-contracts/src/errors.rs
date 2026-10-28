use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidRequest,
    LocalAccessDenied,
    AuthRequired,
    KeyringLocked,
    RuntimeMissing,
    IntegrationRestricted,
    ProtocolIncompatible,
    CapabilityUnknown,
    CapabilityUnsupported,
    OptionUnsupported,
    SelectionStale,
    RateLimited,
    BillingConsentRequired,
    ApprovalRequired,
    SessionRouteMismatch,
    EventGap,
    ProviderUnavailable,
    StorageUnavailable,
    Timeout,
    Cancelled,
    Conflict,
    FrameTooLarge,
    NotFound,
    NotImplemented,
    /// A suspended conversation cannot be continued (identity, route, model,
    /// workspace or native history changed or is gone). Never replaced by a
    /// fresh conversation; the host decides whether to start over.
    ResumeUnavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[error("{code:?}: {message}")]
pub struct DomainError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub retry_after_ms: Option<u64>,
}
impl DomainError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            retry_after_ms: None,
        }
    }
    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }
    pub fn with_retry_after_ms(mut self, milliseconds: u64) -> Self {
        self.retryable = true;
        self.retry_after_ms = Some(milliseconds.min(86_400_000));
        self
    }
}
pub type Result<T> = std::result::Result<T, DomainError>;

/// Keep native error detail useful for local debugging while hiding common credential fields.
pub fn redact_diagnostic_detail(detail: &str) -> String {
    const LIMIT: usize = 2048;
    const MARKERS: &[&str] = &[
        "authorization",
        "access_token",
        "access-token",
        "refresh_token",
        "refresh-token",
        "id_token",
        "id-token",
        "client_secret",
        "client-secret",
        "api_key",
        "api-key",
        "apikey",
        "api key",
        "password",
        "session_id",
        "session-id",
        "session id",
        "bearer",
        "basic",
        "token",
        "secret",
    ];
    fn append_clean(output: &mut String, text: &str) {
        for character in text.chars().filter(|character| !character.is_control()) {
            if output.len() + character.len_utf8() > LIMIT {
                break;
            }
            output.push(character);
        }
    }

    let lowered = detail.to_ascii_lowercase();
    let mut output = String::new();
    let mut cursor = 0;
    while cursor < detail.len() && output.len() < LIMIT {
        let next = MARKERS
            .iter()
            .filter_map(|marker| {
                lowered[cursor..]
                    .find(marker)
                    .map(|offset| (cursor + offset, *marker))
            })
            .min_by_key(|(index, _)| *index);
        let Some((start, marker)) = next else {
            append_clean(&mut output, &detail[cursor..]);
            break;
        };
        append_clean(&mut output, &detail[cursor..start]);
        let after_marker = start + marker.len();
        let mut value_start = if matches!(marker, "bearer" | "basic") {
            after_marker
        } else {
            let scan_end = (after_marker + 32).min(detail.len());
            let delimiter = detail[after_marker..scan_end]
                .find([':', '='])
                .map(|offset| after_marker + offset);
            let Some(delimiter) = delimiter else {
                append_clean(&mut output, &detail[start..after_marker]);
                cursor = after_marker;
                continue;
            };
            delimiter + 1
        };
        while value_start < detail.len()
            && detail[value_start..]
                .chars()
                .next()
                .is_some_and(|character| {
                    character.is_whitespace() || matches!(character, '\'' | '"')
                })
        {
            value_start += detail[value_start..].chars().next().unwrap().len_utf8();
        }
        if marker == "authorization" {
            let remainder = &lowered[value_start..];
            for scheme in ["bearer", "basic"] {
                if remainder.starts_with(scheme) {
                    value_start += scheme.len();
                    while value_start < detail.len()
                        && detail[value_start..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                    {
                        value_start += detail[value_start..].chars().next().unwrap().len_utf8();
                    }
                    break;
                }
            }
        }
        let value_end = detail[value_start..]
            .char_indices()
            .find(|(_, character)| {
                character.is_whitespace() || matches!(character, '\'' | '"' | ',' | ';' | '}' | ']')
            })
            .map(|(offset, _)| value_start + offset)
            .unwrap_or(detail.len());
        if value_end == value_start {
            append_clean(&mut output, &detail[start..after_marker]);
            cursor = after_marker;
            continue;
        }
        append_clean(&mut output, &detail[start..value_start]);
        append_clean(&mut output, "[REDACTED]");
        cursor = value_end;
    }
    output
}
