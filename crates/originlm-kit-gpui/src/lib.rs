//! Reusable, host-themed About/credits link. No networking or global assets required.
use gpui_kit::base::StyledExt as _;
use gpui_kit::*;
use originlm_kit::Credits;
use std::sync::{Arc, OnceLock};
fn mark() -> Arc<Image> {
    static MARK: OnceLock<Arc<Image>> = OnceLock::new();
    MARK.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/originlm-mark.png").to_vec(),
        ))
    })
    .clone()
}
/// Render the kit's metadata in a host's About/credits surface. Pass `kit.credits()`
/// or `CreditsConfig::credits()`. Hidden metadata yields no element.
pub fn credits(metadata: Option<Credits>, muted: Hsla) -> Option<Stateful<Div>> {
    let metadata = metadata?;
    Some(
        div()
            .id("originlm-credit")
            .role(gpui::Role::Link)
            .aria_label(metadata.label)
            .h_flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(12.))
            .text_color(muted)
            .tab_index(0)
            .border_1()
            .border_color(gpui::transparent_black())
            .focus_visible(move |style| style.border_color(muted))
            .cursor_pointer()
            .child(img(mark()).size(px(14.)))
            .child(metadata.label)
            .on_click(move |_, _, cx| cx.open_url(metadata.url)),
    )
}
