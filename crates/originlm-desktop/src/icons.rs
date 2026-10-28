//! Provider glyphs. GPUI Kit ships Lucide, not vendor marks; these are the
//! official Simple Icons glyphs (CC0) embedded as SVG bytes.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{Icon, Sizable, Size};
use gpui_kit::*;

fn bytes(provider: &str) -> Option<&'static [u8]> {
    Some(match provider {
        "codex" => include_bytes!("../assets/providers/codex.svg"),
        "openai" | "openai_api" | "chatgpt" => include_bytes!("../assets/providers/openai.svg"),
        "claude_code" | "claude-code" => include_bytes!("../assets/providers/claude-code.svg"),
        "claude" | "anthropic_api" => include_bytes!("../assets/providers/claude.svg"),
        "anthropic" => include_bytes!("../assets/providers/anthropic.svg"),
        "opencode" | "opencode-go" => include_bytes!("../assets/providers/opencode.svg"),
        "cursor" => include_bytes!("../assets/providers/cursor.svg"),
        "gemini_cli" | "gemini" | "gemini_api" => {
            include_bytes!("../assets/providers/gemini.svg")
        }
        "ollama" => include_bytes!("../assets/providers/ollama.svg"),
        "lmstudio" | "lm_studio" => include_bytes!("../assets/providers/lmstudio.svg"),
        "grok" => include_bytes!("../assets/providers/grok.svg"),
        "xai" => include_bytes!("../assets/providers/xai.svg"),
        "local" => include_bytes!("../assets/providers/local.svg"),
        "antigravity" => include_bytes!("../assets/providers/antigravity.svg"),
        _ => return None,
    })
}

/// A provider glyph at `size`, tinted with `color`.
pub fn provider_mark(provider: &str, size: f32, color: Hsla) -> Option<AnyElement> {
    let data = bytes(provider)?;
    Some(
        Icon::default()
            .data(data)
            .with_size(Size::Size(px(size)))
            .text_color(color)
            .into_any_element(),
    )
}

/// A large, softly tinted provider tile for overview and provider lists.
pub fn provider_tile(
    provider: &str,
    name: &str,
    size: f32,
    theme: &gpui_kit::component::Theme,
) -> AnyElement {
    let inner = provider_mark(provider, size * 0.58, theme.primary).unwrap_or_else(|| {
        div()
            .text_size(px(size * 0.42))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.primary)
            .child(name.chars().next().unwrap_or('•').to_string())
            .into_any_element()
    });
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.26))
        .bg(theme.primary.opacity(0.10))
        .border_1()
        .border_color(theme.primary.opacity(0.22))
        .h_flex()
        .items_center()
        .justify_center()
        .child(inner)
        .into_any_element()
}
