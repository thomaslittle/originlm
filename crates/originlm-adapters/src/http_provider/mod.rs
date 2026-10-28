//! Explicit local-runtime and direct-API adapters. No implicit requests or credentials.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpKind {
    Ollama,
    Lmstudio,
    Openai,
    Anthropic,
    Gemini,
    Xai,
    Compatible,
    Opencode,
}
impl HttpKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Lmstudio => "lmstudio",
            Self::Openai => "openai_api",
            Self::Anthropic => "anthropic_api",
            Self::Gemini => "gemini_api",
            Self::Xai => "xai_api",
            Self::Compatible => "compatible_api",
            Self::Opencode => "opencode",
        }
    }
    pub fn local(self) -> bool {
        matches!(self, Self::Ollama | Self::Lmstudio)
    }
}
mod catalog;
pub mod config;
pub mod frames;
pub use catalog::parse_catalog;
mod probe;
pub use probe::probe;
mod chat;
pub use chat::executor;
