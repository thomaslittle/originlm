//! Shared visual primitives. The component library supplies interaction and
//! controls; these helpers give every page the same composition and rhythm.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{Icon, IconName, Sizable, Size, Theme};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub const PAGE_PAD: f32 = 28.0;
pub const CARD_RADIUS: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Amber,
    Green,
    Red,
    Blue,
}

pub fn tone_color(tone: Tone, theme: &Theme) -> Hsla {
    match tone {
        Tone::Neutral => theme.muted_foreground,
        Tone::Amber => theme.primary,
        Tone::Green => theme.success,
        Tone::Red => theme.danger,
        Tone::Blue => theme.chart_3,
    }
}

pub fn section_title(title: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .child(title.into())
}

pub fn eyebrow(text: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    div()
        .text_size(px(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .child(text.into())
}

pub fn mono(value: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    div()
        .font_family(theme.mono_font_family.clone())
        .text_size(px(11.5))
        .text_color(theme.muted_foreground)
        .child(value.into())
}

pub fn numeric(value: impl Into<SharedString>, theme: &Theme) -> impl IntoElement {
    div()
        .font_family(theme.mono_font_family.clone())
        .text_size(px(13.))
        .text_color(theme.foreground)
        .child(value.into())
}

pub fn chip(label: impl Into<SharedString>, tone: Tone, theme: &Theme) -> impl IntoElement {
    let color = tone_color(tone, theme);
    div()
        .h_flex()
        .gap_1()
        .px_2()
        .py(px(2.))
        .rounded(px(5.))
        .border_1()
        .border_color(color.opacity(0.28))
        .bg(color.opacity(0.10))
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(label.into())
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
        .rounded(px(4.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted)
        .font_family(theme.mono_font_family.clone())
        .text_size(px(10.))
        .text_color(theme.muted_foreground)
        .child(label.into())
}

pub fn metric(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    hint: Option<SharedString>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .v_flex()
        .gap(px(3.))
        .min_w(px(96.))
        .child(eyebrow(label.into(), theme))
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(px(19.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(value.into()),
        )
        .when_some(hint, |this, hint| {
            this.child(
                div()
                    .text_size(px(10.5))
                    .text_color(theme.muted_foreground)
                    .child(hint),
            )
        })
}

pub fn empty_state(
    icon_name: IconName,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .v_flex()
        .items_center()
        .justify_center()
        .gap_2()
        .py_12()
        .child(icon(icon_name, 22., theme.muted_foreground.opacity(0.7)))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(title.into()),
        )
        .child(
            div()
                .max_w(px(360.))
                .text_center()
                .text_size(px(11.5))
                .text_color(theme.muted_foreground)
                .child(body.into()),
        )
}

pub fn divider(theme: &Theme) -> impl IntoElement {
    div().h(px(1.)).w_full().bg(theme.border)
}

pub fn bar(percent: f32, tone: Tone, theme: &Theme) -> impl IntoElement {
    let color = tone_color(tone, theme);
    div()
        .h(px(5.))
        .w_full()
        .rounded_full()
        .bg(theme.foreground.opacity(0.07))
        .child(
            div()
                .h_full()
                .w(relative(percent.clamp(0.0, 1.0)))
                .rounded_full()
                .bg(color),
        )
}

pub fn tab_pill(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    theme: &Theme,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let fg = if selected {
        theme.primary_foreground
    } else {
        theme.muted_foreground
    };
    div()
        .id(id)
        .px_3()
        .py(px(4.))
        .rounded(px(6.))
        .text_size(px(11.5))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(selected, |this| this.bg(theme.primary))
        .when(!selected, |this| this.hover(|s| s.bg(theme.muted)))
        .text_color(fg)
        .child(label.into())
        .on_click(on_click)
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
    let seconds = (reset.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_seconds();
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
                BillingMode::Subscription => "subscription",
                BillingMode::ApiMetered => "metered API",
                BillingMode::PrepaidCredits => "prepaid credits",
                BillingMode::Local => "local",
                BillingMode::Mixed => "mixed",
                BillingMode::Unknown => "billing unknown",
            };
            if matches!(freshness, originlm_contracts::Freshness::Stale) {
                "stale billing"
            } else {
                label
            }
        }
        Observation::Unknown { .. } => "billing unknown",
        Observation::Unavailable { .. } => "billing unavailable",
    }
}
