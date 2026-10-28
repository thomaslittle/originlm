//! Protocol generation is observed, never guessed from installed credentials.
use originlm_contracts::{DomainError, ErrorCode, Result};

/// The installed major version, if the CLI reported a plain semver string.
pub(crate) fn installed_major(output: &str) -> Option<u32> {
    let value = output.trim();
    if value.len() > 128 {
        return None;
    }
    let value = value.strip_prefix("opencode ").unwrap_or(value);
    let value = value.strip_prefix('v').unwrap_or(value);
    if value.chars().any(char::is_whitespace) {
        return None;
    }
    let release = value.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<_> = release.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    parts[0].parse().ok()
}

pub(crate) fn require_legacy(output: &str) -> Result<()> {
    let invalid = || {
        DomainError::new(
            ErrorCode::ProtocolIncompatible,
            "This OpenCode protocol version is unsupported; this adapter requires OpenCode 1.x",
        )
    };
    let value = output.trim();
    if value.len() > 128 {
        return Err(invalid());
    }
    let value = value.strip_prefix("opencode ").unwrap_or(value);
    let value = value.strip_prefix('v').unwrap_or(value);
    let release = value.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<_> = release.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()))
        || parts[0] != "1"
        || value.chars().any(char::is_whitespace)
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::require_legacy;

    #[test]
    fn reads_the_installed_major() {
        assert_eq!(super::installed_major("opencode v2.2.0\n"), Some(2));
        assert_eq!(super::installed_major("1.18.32"), Some(1));
        assert_eq!(super::installed_major("unknown"), None);
    }

    #[test]
    fn accepts_reported_legacy_versions_only() {
        for version in ["1.18.32", "opencode v1.18.32\n", "1.18.32-beta.1"] {
            assert!(require_legacy(version).is_ok(), "{version}");
        }
        for version in [
            "opencode v2.0.18",
            "3.0.0",
            "unknown",
            "1",
            "1.2",
            "1..2",
            "1.2.x",
            "1.2.3\n2.0.0",
        ] {
            assert!(require_legacy(version).is_err(), "{version}");
        }
    }
}
