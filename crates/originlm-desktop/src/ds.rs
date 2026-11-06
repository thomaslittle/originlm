//! Design system: the scales and shared components every page composes from.
//!
//! Rules
//! - Spacing comes from `sp`, type sizes from `ty`, corner radii from `radius`.
//!   Pages should not pass literal pixel values for these.
//! - Surfaces step up from the page: `Surface::Page` < `Surface::Panel`.
//! - Meaning comes from state, not decoration: amber is the brand and the one
//!   primary action, green is "verified/fresh", red is "blocked/low",
//!   neutral is everything else. Unknown is drawn as its own state, never as
//!   an empty or zero meter.

use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{IconName, Theme, tooltip::Tooltip};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// 4px spacing scale.
pub mod sp {
    pub const XS: f32 = 4.;
    pub const SM: f32 = 8.;
    pub const MD: f32 = 12.;
    pub const LG: f32 = 16.;
    pub const XL: f32 = 24.;
    pub const XXL: f32 = 32.;
}

/// Type scale. 11px is the floor and is reserved for captions and meta.
pub mod ty {
    pub const CAPTION: f32 = 11.;
    pub const SMALL: f32 = 12.;
    pub const BODY: f32 = 13.;
    pub const TITLE: f32 = 15.;
    pub const HEADING: f32 = 22.;
}

pub mod radius {
    pub const CONTROL: f32 = 4.;
    pub const PANEL: f32 = 6.;
}

/// Page gutter.
pub const GUTTER: f32 = 28.;
/// Height of a selectable list row: a two-line label and a comfortable target.
pub const ROW: f32 = 44.;
/// Minimum height of any clickable control.
pub const TARGET: f32 = 28.;
/// Cells in a segmented meter. Shared with the landing page.
pub const METER_CELLS: usize = 20;

#[derive(Clone, Copy)]
pub enum Surface {
    Page,
    Panel,
}

pub fn surface(theme: &Theme, level: Surface) -> Hsla {
    match level {
        Surface::Page => theme.background,
        Surface::Panel => theme.popover,
    }
}

/// A bordered panel. Children set their own padding so rows can run edge to edge.
pub fn panel(theme: &Theme) -> Div {
    div()
        .v_flex()
        .min_w(px(0.))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme.border)
        .bg(surface(theme, Surface::Panel))
        .overflow_hidden()
}

/// Panel header: title on the left, optional meta or actions on the right.
pub fn panel_head(
    title: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    theme: &Theme,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(sp::MD))
        .min_h(px(40.))
        .px(px(sp::LG))
        .py(px(sp::SM))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(px(ty::SMALL))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(title.into()),
        )
        .child(div().flex_1())
        .children(trailing)
}

pub fn caption(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(px(ty::CAPTION))
        .text_color(theme.muted_foreground)
        .child(text.into())
}

pub fn body(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(px(ty::BODY))
        .text_color(theme.muted_foreground)
        .child(text.into())
}

pub fn mono(text: impl Into<SharedString>, size: f32, color: Hsla, theme: &Theme) -> Div {
    div()
        .font_family(theme.mono_font_family.clone())
        .text_size(px(size))
        .text_color(color)
        .child(text.into())
}

/// A small state label with a border. Text always carries the meaning; colour
/// only reinforces it.
pub fn chip(label: impl Into<SharedString>, tone: Tone, theme: &Theme) -> Div {
    let color = ui::tone_color(tone, theme);
    let neutral = matches!(tone, Tone::Neutral);
    // Content width. Put chips in an `h_flex` parent: a block parent stretches
    // them to its width.
    div()
        .h_flex()
        .flex_none()
        .max_w_full()
        .px(px(6.))
        .py(px(1.))
        .rounded(px(radius::CONTROL))
        .border_1()
        .border_color(if neutral {
            theme.border
        } else {
            color.opacity(0.55)
        })
        // A tint strong enough to read as a filled state in both themes. At
        // 8% the light theme lost the fill entirely.
        .when(!neutral, |this| this.bg(color.opacity(0.14)))
        .text_size(px(ty::CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if neutral {
            theme.muted_foreground
        } else {
            color
        })
        .child(label.into())
}

/// Status dot plus label, so state is never colour-only.
pub fn status(label: impl Into<SharedString>, tone: Tone, theme: &Theme) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(6.))
        .child(ui::dot(tone, theme))
        .child(
            div()
                .text_size(px(ty::SMALL))
                .text_color(theme.foreground)
                .child(label.into()),
        )
}

/// Tone for a remaining fraction.
pub fn remaining_tone(remaining: f32) -> Tone {
    if remaining < 0.20 {
        Tone::Red
    } else if remaining < 0.50 {
        Tone::Amber
    } else {
        Tone::Green
    }
}

/// Segmented meter that fills with what remains. `None` draws the unknown
/// state: hollow cells, never an empty "zero" meter.
pub fn meter(remaining: Option<f32>, theme: &Theme) -> Div {
    let filled = remaining
        .map(|value| (value.clamp(0.0, 1.0) * METER_CELLS as f32).round() as usize)
        .unwrap_or(0);
    let fill = match remaining {
        // Availability has its own meaning: green is healthy, amber is
        // tightening, red is close to exhausted. Brand amber stays for
        // actions, never for a healthy quota.
        Some(value) if value < 0.20 => theme.danger,
        Some(value) if value < 0.50 => theme.warning,
        _ => theme.success,
    };
    let track = theme.foreground.opacity(0.08);
    // Full width and a fixed height in both flex directions: `flex_1` would
    // collapse the height to zero inside a column.
    div()
        .h_flex()
        .w_full()
        .min_w(px(0.))
        .h(px(10.))
        .gap(px(2.))
        .children((0..METER_CELLS).map(move |index| {
            let cell = div().flex_1().h_full().rounded(px(1.));
            if remaining.is_none() {
                cell.border_1().border_color(track.opacity(1.6))
            } else if index < filled {
                cell.bg(fill)
            } else {
                cell.bg(track)
            }
        }))
}

/// Label and value on one line, label muted.
pub fn kv(label: impl Into<SharedString>, value: impl IntoElement, theme: &Theme) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(sp::MD))
        .min_h(px(28.))
        .child(
            div()
                .w(px(132.))
                .flex_shrink_0()
                .text_size(px(ty::SMALL))
                .text_color(theme.muted_foreground)
                .child(label.into()),
        )
        .child(div().h_flex().flex_1().min_w(px(0.)).child(value))
}

/// A compact inline figure for strips: value then label.
pub fn stat(value: impl Into<SharedString>, label: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .h_flex()
        .items_end()
        .gap(px(6.))
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(px(ty::TITLE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(value.into()),
        )
        .child(
            div()
                .pb(px(1.))
                .text_size(px(ty::SMALL))
                .text_color(theme.muted_foreground)
                .child(label.into()),
        )
}

/// Text action with a 28px hit area.
pub fn link(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let color = theme.primary;
    div()
        .id(id)
        .role(gpui::Role::Link)
        .h_flex()
        .items_center()
        .min_h(px(TARGET))
        .focus_ring(theme)
        .px(px(sp::SM))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_size(px(ty::SMALL))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .hover(move |style| style.bg(color.opacity(0.1)))
        .child(label.into())
        .on_click(on_click)
}

/// A read-only code sample with a copy action.
pub fn code(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    source: &'static str,
    theme: &Theme,
) -> Div {
    let id: SharedString = id.into();
    div()
        .v_flex()
        .min_w(px(0.))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme.border)
        .bg(surface(theme, Surface::Page))
        .overflow_hidden()
        .child(
            div()
                .h_flex()
                .items_center()
                .px(px(sp::MD))
                .h(px(32.))
                .border_b_1()
                .border_color(theme.border)
                .child(mono(title, ty::CAPTION, theme.muted_foreground, theme))
                .child(div().flex_1())
                .child(link(
                    SharedString::from(format!("{id}-copy")),
                    "Copy",
                    theme,
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(source.to_string()));
                    },
                )),
        )
        .child(
            div()
                .v_flex()
                .px(px(sp::MD))
                .py(px(sp::MD))
                .children(source.lines().map(|line| {
                    let text = if line.is_empty() {
                        " ".to_string()
                    } else {
                        line.to_string()
                    };
                    let comment =
                        line.trim_start().starts_with("//") || line.trim_start().starts_with('#');
                    mono(
                        text,
                        ty::SMALL,
                        if comment {
                            theme.muted_foreground
                        } else {
                            theme.foreground
                        },
                        theme,
                    )
                    .line_height(px(19.))
                    .whitespace_nowrap()
                })),
        )
}

/// Keyboard focus. Every interactive element calls this: it joins the tab
/// order, and a 2px ring appears only for keyboard focus, like CSS
/// `:focus-visible`. Enter and Space activate the element's `on_click`.
pub trait Focusable: Sized {
    fn focus_ring(self, theme: &Theme) -> Self;
}

impl Focusable for Stateful<Div> {
    fn focus_ring(self, theme: &Theme) -> Self {
        let ring = theme.ring;
        self.tab_index(0)
            .border_2()
            .border_color(gpui::transparent_black())
            .focus_visible(move |style| style.border_color(ring))
    }
}

/// The page body: gutter, bottom padding and the vertical rhythm between
/// sections. Every page starts from this.
pub fn page() -> Div {
    div()
        .v_flex()
        .w_full()
        .px(px(GUTTER))
        .pb(px(sp::XXL))
        .gap(px(sp::XL))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StateKind {
    /// Nothing to show yet, and that is a normal answer.
    Empty,
    /// Waiting for the first answer.
    Loading,
    /// The request failed and nothing usable is cached. (A failure with a
    /// cached snapshot is shown as the shell's stale banner instead.)
    Error,
}

/// A state block: icon, title, body. Callers append an action row. Copy says
/// what happened and what to do next; it never implies a zero reading.
pub fn state(
    kind: StateKind,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    theme: &Theme,
) -> Div {
    let (icon, color) = match kind {
        StateKind::Empty => (IconName::Inbox, theme.muted_foreground),
        StateKind::Loading => (IconName::LoaderCircle, theme.muted_foreground),
        StateKind::Error => (IconName::CircleX, theme.danger),
    };
    div()
        .v_flex()
        .items_start()
        .gap(px(sp::SM))
        .px(px(sp::XL))
        .py(px(sp::XL))
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(sp::SM))
                .child(ui::icon(icon, 16., color))
                .child(
                    div()
                        .text_size(px(ty::TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(title.into()),
                ),
        )
        .child(
            div()
                .max_w(px(560.))
                .text_size(px(ty::BODY))
                .line_height(px(20.))
                .text_color(theme.muted_foreground)
                .child(body.into()),
        )
}

/// An inline notice. Tone carries the severity; the text carries the meaning.
pub fn banner(
    tone: Tone,
    title: impl Into<SharedString>,
    body: Option<SharedString>,
    theme: &Theme,
) -> Div {
    let color = ui::tone_color(tone, theme);
    let neutral = matches!(tone, Tone::Neutral);
    let icon = match tone {
        Tone::Red => IconName::CircleX,
        Tone::Amber => IconName::TriangleAlert,
        Tone::Green => IconName::CircleCheck,
        _ => IconName::Info,
    };
    div()
        .h_flex()
        .items_start()
        .gap(px(sp::SM))
        .px(px(sp::LG))
        .py(px(sp::MD))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(if neutral {
            theme.border
        } else {
            color.opacity(0.45)
        })
        .bg(if neutral {
            surface(theme, Surface::Panel)
        } else {
            color.opacity(0.07)
        })
        .child(div().pt(px(2.)).child(ui::icon(
            icon,
            14.,
            if neutral {
                theme.muted_foreground
            } else {
                color
            },
        )))
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(ty::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(title.into()),
                )
                .when_some(body, |this, body| {
                    this.child(
                        div()
                            .text_size(px(ty::SMALL))
                            .line_height(px(18.))
                            .text_color(theme.muted_foreground)
                            .child(body),
                    )
                }),
        )
}

/// Segmented tabs. Each segment is focusable and announced as a tab.
pub fn tabs<I: Into<SharedString>>(
    id: &'static str,
    labels: impl IntoIterator<Item = I>,
    selected: usize,
    theme: &Theme,
    on_select: impl Fn(usize, &mut Window, &mut App) + Clone + 'static,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme.border)
        .bg(surface(theme, Surface::Page))
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let active = index == selected;
            let on_select = on_select.clone();
            let label: SharedString = label.into();
            div()
                .id((id, index))
                .role(gpui::Role::Tab)
                .aria_label(label.clone())
                .h_flex()
                .items_center()
                .h(px(TARGET))
                .px(px(sp::MD))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(ty::SMALL))
                .font_weight(FontWeight::MEDIUM)
                .focus_ring(theme)
                .when(active, |this| {
                    this.bg(theme.secondary_active).text_color(theme.foreground)
                })
                .when(!active, |this| {
                    this.text_color(theme.muted_foreground)
                        .hover(|style| style.bg(theme.secondary_hover))
                })
                .child(label)
                .on_click(move |_, window, cx| on_select(index, window, cx))
        }))
}

/// A selectable list row: full width, `ROW` tall, one selection signal
/// (a raised background and a 2px amber edge on the left).
pub fn select_row(id: impl Into<ElementId>, selected: bool, theme: &Theme) -> Stateful<Div> {
    let edge = theme.primary;
    div()
        .id(id)
        .role(gpui::Role::ListItem)
        .relative()
        .h_flex()
        .items_center()
        .gap(px(sp::MD))
        .min_h(px(ROW))
        .px(px(sp::MD))
        .cursor_pointer()
        .focus_ring(theme)
        .when(selected, |this| {
            this.bg(theme.list_active).child(
                div()
                    .absolute()
                    // Keep the selection edge inside the panel's clipped bounds.
                    .left(px(0.))
                    .top(px(8.))
                    .bottom(px(8.))
                    .w(px(2.))
                    .bg(edge),
            )
        })
        .when(!selected, |this| {
            this.hover(|style| style.bg(theme.list_hover))
        })
}

/// A column heading for a panel table.
pub fn th(text: &'static str, width: Option<f32>, theme: &Theme) -> Div {
    let cell = div()
        .text_size(px(ty::CAPTION))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .child(text);
    match width {
        Some(width) => cell.w(px(width)).flex_shrink_0(),
        None => cell.flex_1().min_w(px(0.)),
    }
}

/// The heading row of a panel table.
pub fn table_head(theme: &Theme) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(sp::MD))
        .px(px(sp::LG))
        .h(px(32.))
        .border_b_1()
        .border_color(theme.border)
}

/// A mono value with a copy action. The value and the icon are one target.
pub fn copy_value(
    id: impl Into<SharedString>,
    value: impl Into<SharedString>,
    max_chars: usize,
    theme: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let value: SharedString = value.into();
    let copy = value.clone();
    let label: SharedString = format!("Copy {value}").into();
    let tip = label.clone();
    div()
        .id(SharedString::from(format!("copy-{id}")))
        .role(gpui::Role::Button)
        .aria_label(label)
        .h_flex()
        .items_center()
        .gap(px(6.))
        .min_w(px(0.))
        .min_h(px(TARGET))
        .px(px(6.))
        // Line the text up with plain values: cancel the padding and ring.
        .ml(px(-8.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .focus_ring(theme)
        .hover(|style| style.bg(theme.secondary_hover))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .child(
            mono(
                ui::short(&value, max_chars),
                ty::SMALL,
                theme.foreground,
                theme,
            )
            .min_w(px(0.))
            .truncate(),
        )
        .child(ui::icon(IconName::Copy, 12., theme.muted_foreground))
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()));
        })
}

/// An icon-only button. The label is the tooltip and the accessible name.
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
    color: Hsla,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let tip = label.clone();
    div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(label)
        .h_flex()
        .items_center()
        .justify_center()
        .size(px(TARGET + 4.))
        .flex_shrink_0()
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .focus_ring(theme)
        .hover(|style| style.bg(theme.secondary_hover))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .child(ui::icon(icon, 14., color))
        .on_click(on_click)
}

/// A row in a key/value or list panel, with the standard inset and a hairline
/// above every row but the first.
pub fn panel_row(first: bool, theme: &Theme) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(sp::MD))
        .px(px(sp::LG))
        .min_h(px(36.))
        .when(!first, |this| {
            this.border_t_1().border_color(theme.border.opacity(0.7))
        })
}

#[cfg(test)]
mod tests {
    use super::remaining_tone;
    use crate::ui::Tone;

    #[test]
    fn availability_is_green_amber_or_red_by_remaining() {
        assert_eq!(remaining_tone(0.75), Tone::Green);
        assert_eq!(remaining_tone(0.50), Tone::Green);
        assert_eq!(remaining_tone(0.49), Tone::Amber);
        assert_eq!(remaining_tone(0.20), Tone::Amber);
        assert_eq!(remaining_tone(0.19), Tone::Red);
        assert_eq!(remaining_tone(0.0), Tone::Red);
    }
}
