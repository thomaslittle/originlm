//! Provider-specific region rejection normalization. This is **not** a
//! preflight availability claim: it requires an actual OpenCode Go rejection.
use originlm_contracts::{DomainError, ErrorCode};

const GO_GLOBAL_REASON: &str = "This Go model requires Global regions. Select Global in your workspace's Privacy settings to use it.";
const GO_GLOBAL_GUIDANCE: &str = "OpenCode Go requires Global processing regions for this model. Review your workspace's Settings > Privacy > Regions in OpenCode Console to decide whether to enable Global.";

/// Classify only exact known OpenCode Go region-restriction wording. The
/// optional upstream wrappers are from OpenCode, not arbitrary substring
/// searches. Unknown provider errors retain their ordinary error handling.
pub(super) fn global_region_restriction(provider: &str, message: &str) -> Option<DomainError> {
    if provider != "opencode-go" {
        return None;
    }
    let message = message.trim();
    let known = [
        "",
        "Upstream request failed: ",
        "Internal error: Upstream request failed: ",
    ]
    .into_iter()
    .any(|prefix| message.strip_prefix(prefix) == Some(GO_GLOBAL_REASON));
    known.then(|| DomainError::new(ErrorCode::ProviderRegionRestricted, GO_GLOBAL_GUIDANCE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_go_region_rejections_produce_a_safe_non_retryable_error() {
        for prefix in [
            "",
            "Upstream request failed: ",
            "Internal error: Upstream request failed: ",
        ] {
            let upstream = format!("{prefix}{GO_GLOBAL_REASON}");
            let error = global_region_restriction("opencode-go", &upstream).unwrap();
            assert_eq!(error.code, ErrorCode::ProviderRegionRestricted);
            assert!(!error.retryable);
            assert_eq!(error.retry_after_ms, None);
            assert_eq!(error.message, GO_GLOBAL_GUIDANCE);
            assert!(!error.message.contains(&upstream));
        }
    }

    #[test]
    fn unrelated_routes_and_near_misses_are_not_regional_errors() {
        for provider in ["opencode", "opencode-go-free", "openai", ""] {
            assert!(global_region_restriction(provider, GO_GLOBAL_REASON).is_none());
        }
        for message in [
            "Go usage limit exceeded",
            "This Go model requires US regions.",
            "Global regions are enabled",
            "This Go model requires Global regions.",
            "Internal error: Upstream request failed: Secret: This Go model requires Global regions. Select Global in your workspace's Privacy settings to use it.",
            "This Go model requires Global regions. Select Global in your workspace's Privacy settings to use it. secret",
        ] {
            assert!(
                global_region_restriction("opencode-go", message).is_none(),
                "{message}"
            );
        }
    }
}
