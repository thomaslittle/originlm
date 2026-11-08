//! Small primitives and formatting helpers. Composed components (panels,
//! meters, chips, states) live in `ds.rs`.

use gpui_kit::component::{Icon, IconName, Sizable, Size, Theme};
use gpui_kit::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Amber,
    Green,
    Red,
}

pub fn tone_color(tone: Tone, theme: &Theme) -> Hsla {
    match tone {
        Tone::Neutral => theme.muted_foreground,
        Tone::Amber => theme.primary,
        Tone::Green => theme.success,
        Tone::Red => theme.danger,
    }
}

pub fn dot(tone: Tone, theme: &Theme) -> impl IntoElement {
    let color = tone_color(tone, theme);
    div().size(px(7.)).rounded_full().bg(color).flex_shrink_0()
}

pub fn icon(name: IconName, size: f32, color: Hsla) -> impl IntoElement {
    Icon::new(name)
        .with_size(Size::Size(px(size)))
        .text_color(color)
}

pub fn keycap(label: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    div()
        .px(px(5.))
        .py(px(1.))
        .rounded(px(crate::ds::radius::CONTROL))
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted)
        .font_family(theme.mono_font_family.clone())
        .text_size(px(crate::ds::ty::CAPTION))
        .text_color(theme.muted_foreground)
        .child(label.into())
}

pub fn plural(count: usize, singular: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {singular}s")
    }
}

pub fn short(value: &str, max: usize) -> String {
    if value.chars().count() > max {
        let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        value.to_string()
    }
}

pub fn format_age(seconds: i64) -> String {
    match seconds {
        i64::MIN..=0 => "now".into(),
        1..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86_399 => format!("{}h", seconds / 3600),
        _ => format!("{}d", seconds / 86_400),
    }
}

pub fn format_countdown(resets_at: Option<&str>) -> Option<String> {
    let value = resets_at?;
    let reset = chrono::DateTime::parse_from_rfc3339(value).ok()?;
    let seconds = (reset.with_timezone(&chrono::Utc) - crate::clock::utc_now()).num_seconds();
    if seconds <= 0 {
        return Some("reset due".into());
    }
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3600;
    let minutes = (seconds % 3600) / 60;
    Some(if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    })
}

pub fn billing_label(route: &originlm_contracts::AccessRoute) -> &'static str {
    use originlm_contracts::{BillingMode, Observation};
    match &route.billing {
        Observation::Known {
            value, freshness, ..
        } => {
            let label = match value {
                BillingMode::Subscription => "Subscription",
                BillingMode::ApiMetered => "Metered API",
                BillingMode::PrepaidCredits => "Prepaid credits",
                BillingMode::Local => "Local",
                BillingMode::Mixed => "Mixed",
                BillingMode::Unknown => "Billing unknown",
            };
            if matches!(freshness, originlm_contracts::Freshness::Stale) {
                "Stale billing"
            } else {
                label
            }
        }
        Observation::Unknown { .. } => "billing unknown",
        Observation::Unavailable { .. } => "billing unavailable",
    }
}
