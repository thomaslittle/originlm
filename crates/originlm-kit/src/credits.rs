//! Host-rendered, optional product credits. This module has no UI or I/O dependency.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
    pub label: &'static str,
    pub product_name: &'static str,
    pub mark_reference: &'static str,
    pub url: &'static str,
}
impl Credits {
    pub const ORIGINLM: Self = Self {
        label: "Runs on OriginLM",
        product_name: "OriginLM",
        mark_reference: "originlm-mark",
        url: "https://originlm.livestreaming.tools",
    };
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct CreditsConfig {
    pub show_attribution: bool,
}
impl Default for CreditsConfig {
    fn default() -> Self {
        Self {
            show_attribution: true,
        }
    }
}
impl CreditsConfig {
    pub fn credits(self) -> Option<Credits> {
        self.show_attribution.then_some(Credits::ORIGINLM)
    }
}
