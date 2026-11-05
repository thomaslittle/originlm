//! Provider glyphs. GPUI Kit ships Lucide, not vendor marks; these are the
//! official Simple Icons glyphs (CC0) embedded as SVG bytes.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{Icon, Sizable, Size};
use gpui_kit::*;

fn bytes(provider: &str) -> Option<&'static [u8]> {
    Some(match provider {
        "codex" => include_bytes!("../assets/providers/openai.svg"),
        "openai" | "openai_api" | "chatgpt" => include_bytes!("../assets/providers/openai.svg"),
        "claude_code" | "claude-code" => include_bytes!("../assets/providers/claude.svg"),
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
        "muse" | "muse_code" | "muse-code" => include_bytes!("../assets/providers/meta.svg"),
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

/// Vendor mark colours for the providers whose marks are recognisable by
/// colour. Others keep the theme foreground so a neutral mark never invents
/// a brand colour.
fn brand_hex(provider: &str) -> Option<u32> {
    Some(match provider {
        "claude_code" | "claude-code" | "claude" | "anthropic_api" => 0xD97757,
        "codex" | "openai" | "openai_api" | "chatgpt" => 0x10A37F,
        "gemini_cli" | "gemini" | "gemini_api" => 0x4285F4,
        "muse" | "muse_code" | "muse-code" => 0x0081FB,
        _ => return None,
    })
}

fn brand_color(provider: &str, theme: &gpui_kit::component::Theme) -> Hsla {
    brand_hex(provider)
        .map(|hex| gpui::rgb(hex).into())
        .unwrap_or(theme.foreground)
}

/// A large, softly tinted provider tile for overview and provider lists.
pub fn provider_tile(
    provider: &str,
    name: &str,
    size: f32,
    theme: &gpui_kit::component::Theme,
) -> AnyElement {
    let mark_color = brand_color(provider, theme);
    let inner = provider_mark(provider, size * 0.58, mark_color).unwrap_or_else(|| {
        div()
            .text_size(px(size * 0.42))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(mark_color)
            .child(name.chars().next().unwrap_or('•').to_string())
            .into_any_element()
    });
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.26))
        .bg(theme.foreground.opacity(0.06))
        .border_1()
        .border_color(theme.border)
        .h_flex()
        .items_center()
        .justify_center()
        .child(inner)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::brand_hex;

    #[test]
    fn vendor_marks_use_their_brand_colour() {
        assert_eq!(brand_hex("claude_code"), Some(0xD97757));
        assert_eq!(brand_hex("codex"), Some(0x10A37F));
        assert_eq!(brand_hex("gemini_cli"), Some(0x4285F4));
    }

    #[test]
    fn muse_uses_the_meta_mark() {
        assert!(super::bytes("muse").is_some());
        assert_eq!(brand_hex("muse"), Some(0x0081FB));
    }

    #[test]
    fn neutral_marks_do_not_invent_a_brand_colour() {
        assert_eq!(brand_hex("cursor"), None);
        assert_eq!(brand_hex("opencode"), None);
        assert_eq!(brand_hex("grok"), None);
    }
}
