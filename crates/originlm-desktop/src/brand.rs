//! User-supplied OriginLM offset-O artwork, rendered in full color.
use gpui_kit::*;
use std::sync::{Arc, OnceLock};

pub fn app_tile(size: f32) -> AnyElement {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    let icon = ICON.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/brand/originlm-app-256.png").to_vec(),
        ))
    });
    img(icon.clone()).size(px(size)).into_any_element()
}
