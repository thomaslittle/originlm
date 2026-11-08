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
    /// Provider refuses a model until the operator changes regional processing settings.
    /// Neither execution nor billing settings are modified automatically.
    ProviderRegionRestricted,
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

/// Redacts credential-shaped values from conversation text before it is stored
/// in OriginLM's canonical transcript. Unlike error-detail redaction this keeps
/// long text intact; only the matched values are replaced.
pub fn redact_conversation_text(text: &str) -> String {
    let without_keys = redact_private_key_blocks(text);
    let without_assignments = redact_credential_assignments(&without_keys);
    redact_credential_prefixes(&without_assignments)
}

fn redact_private_key_blocks(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find("-----BEGIN ") {
        let start = cursor + offset;
        output.push_str(&text[cursor..start]);
        let Some(end_offset) = text[start..].find("-----END ") else {
            output.push_str("[REDACTED PRIVATE KEY]");
            return output;
        };
        let end = start + end_offset;
        let end = text[end..]
            .find("-----")
            .map(|extra| end + extra + 5)
            .unwrap_or(text.len());
        output.push_str("[REDACTED PRIVATE KEY]");
        cursor = end;
    }
    output.push_str(&text[cursor..]);
    output
}

fn delimiter(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '\'' | '"' | ',' | ';' | '}' | ']' | ')' | '<' | '>'
        )
}

fn redact_credential_assignments(text: &str) -> String {
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
        "passwd",
        "session_id",
        "session-id",
        "session id",
        "private_key",
        "private-key",
        "bearer",
        "token",
        "secret",
    ];
    let lowered = text.to_ascii_lowercase();
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < text.len() {
        let next = MARKERS
            .iter()
            .filter_map(|marker| {
                lowered[cursor..]
                    .find(marker)
                    .map(|offset| (cursor + offset, *marker))
            })
            .min_by_key(|(index, marker)| (*index, std::cmp::Reverse(marker.len())));
        let Some((start, marker)) = next else {
            output.push_str(&text[cursor..]);
            break;
        };
        let after_marker = start + marker.len();
        // Require a word boundary before the marker so "secretive" is untouched.
        if start > 0
            && text[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            output.push_str(&text[cursor..after_marker]);
            cursor = after_marker;
            continue;
        }
        let mut value_start = after_marker;
        if matches!(marker, "bearer" | "basic") {
            if !text[value_start..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
            {
                output.push_str(&text[cursor..after_marker]);
                cursor = after_marker;
                continue;
            }
        } else {
            let scan_end = (after_marker + 32).min(text.len());
            let Some(delimiter_offset) = text[after_marker..scan_end].find([':', '=']) else {
                output.push_str(&text[cursor..after_marker]);
                cursor = after_marker;
                continue;
            };
            value_start = after_marker + delimiter_offset + 1;
        }
        while value_start < text.len()
            && text[value_start..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || matches!(c, '\'' | '"'))
        {
            value_start += text[value_start..].chars().next().unwrap().len_utf8();
        }
        if marker == "authorization" {
            let remainder = &lowered[value_start..];
            for scheme in ["bearer", "basic"] {
                if remainder.starts_with(scheme) {
                    value_start += scheme.len();
                    while value_start < text.len()
                        && text[value_start..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                    {
                        value_start += text[value_start..].chars().next().unwrap().len_utf8();
                    }
                    break;
                }
            }
        }
        let value_end = text[value_start..]
            .char_indices()
            .find(|(_, character)| delimiter(*character))
            .map(|(offset, _)| value_start + offset)
            .unwrap_or(text.len());
        if value_end == value_start {
            output.push_str(&text[cursor..after_marker]);
            cursor = after_marker;
            continue;
        }
        output.push_str(&text[cursor..value_start]);
        output.push_str("[REDACTED]");
        cursor = value_end;
    }
    output
}

fn token_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '.' | '_' | '-' | '+' | '/' | '=' | ':')
}

fn redact_credential_prefixes(text: &str) -> String {
    const PREFIXES: &[&str] = &[
        "sk-ant-",
        "sk-proj-",
        "sk-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        "AKIA",
        "ASIA",
        "AIza",
        "ya29.",
        "eyJ",
    ];
    let lowered = text.to_ascii_lowercase();
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < text.len() {
        let next = PREFIXES
            .iter()
            .filter_map(|prefix| {
                let needle = prefix.to_ascii_lowercase();
                lowered[cursor..]
                    .find(&needle)
                    .map(|offset| (cursor + offset, prefix.len()))
            })
            .min_by_key(|(index, _)| *index);
        let Some((start, prefix_len)) = next else {
            break;
        };
        let mut end = start + prefix_len;
        let mut length = 0;
        while end < text.len() {
            let character = text[end..].chars().next().unwrap();
            if !token_character(character) || length >= 4096 {
                break;
            }
            end += character.len_utf8();
            length += 1;
        }
        // A bare marker such as "sk-" with no payload is left as prose.
        if length < 4 {
            output.push_str(&text[cursor..start + prefix_len]);
            cursor = start + prefix_len;
            continue;
        }
        output.push_str(&text[cursor..start]);
        output.push_str("[REDACTED]");
        cursor = end;
    }
    output.push_str(&text[cursor..]);
    output
}

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
