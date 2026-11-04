//! Settings: appearance, data source, refresh, layout, local prices, about.
//! Everyday settings first, advanced tools last.

use crate::app::{Desktop, Page};
use crate::ds::{self, sp, ty};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::*;

const REFRESH: [u64; 3] = [15, 30, 60];

fn setting(
    title: &'static str,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
    theme: &Theme,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .flex_wrap()
        .gap(px(sp::LG))
        .px(px(sp::LG))
        .py(px(sp::MD))
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w(px(240.))
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(ty::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(ds::caption(detail, theme)),
        )
        .child(control)
}

impl Desktop {
    pub fn page_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let state_dir = crate::history::state_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Unavailable; preferences are not saved".into());
        let value = |text: String| {
            div()
                .text_size(px(ty::SMALL))
                .text_color(theme.foreground)
                .child(text)
        };

        let appearance = ds::panel(&theme)
            .child(ds::panel_head("Appearance", None, &theme))
            .child(setting(
                "Theme",
                "Graphite dark or paper light. Ctrl+T switches.",
                ds::tabs(
                    "theme",
                    ["Dark", "Light"],
                    usize::from(!self.dark),
                    &theme,
                    {
                        let entity = entity.clone();
                        move |index, _, cx| {
                            entity.update(cx, |this, cx| {
                                if this.dark != (index == 0) {
                                    this.toggle_theme(cx);
                                }
                            })
                        }
                    },
                ),
                &theme,
            ));

        let source = ds::panel(&theme)
            .child(ds::panel_head("Data source", None, &theme))
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::kv(
                        "Runtime",
                        value(if self.example {
                            "Example mode: no runtime, nothing on this machine is read".into()
                        } else {
                            self.credential.label()
                        }),
                        &theme,
                    ))
                    .child(ds::kv(
                        "Endpoint",
                        if self.example {
                            value("None in example mode".into()).into_any_element()
                        } else {
                            ds::copy_value("endpoint", self.credential.address(), 44, &theme)
                                .into_any_element()
                        },
                        &theme,
                    ))
                    .child(ds::kv(
                        "Credential",
                        value(self.credential.descriptor_label()),
                        &theme,
                    ))
                    .child(ds::kv(
                        "App administration",
                        value(if self.example {
                            "Not available in example mode".into()
                        } else if self.bundle.as_ref().is_some_and(|bundle| {
                            bundle.error.is_none() && bundle.apps_error.is_none()
                        }) {
                            "Confirmed by the runtime".into()
                        } else {
                            "Not confirmed: this credential cannot list apps".to_string()
                        }),
                        &theme,
                    ))
                    .child(ds::kv(
                        "State folder",
                        ds::copy_value("state-dir", state_dir, 44, &theme),
                        &theme,
                    )),
            )
            .child(
                div()
                    .px(px(sp::LG))
                    .pb(px(sp::MD))
                    .child(ds::caption(
                        "Credentials are never displayed, logged or included in exports. The state folder holds preferences and the usage readings this app recorded.",
                        &theme,
                    )),
            );

        let refresh_index = REFRESH
            .iter()
            .position(|s| *s == self.refresh_seconds)
            .unwrap_or(1);
        let refresh = ds::panel(&theme)
            .child(ds::panel_head("Refresh", None, &theme))
            .child(setting(
                "Read the runtime every",
                "Only how often this window reads the runtime's cached snapshot. It never makes a provider call on its own.",
                ds::tabs(
                    "refresh",
                    REFRESH.map(|s| format!("{s}s")),
                    refresh_index,
                    &theme,
                    {
                        let entity = entity.clone();
                        move |index, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.refresh_seconds = REFRESH[index];
                                this.save_prefs();
                                this.refresh(cx);
                            })
                        }
                    },
                ),
                &theme,
            ));

        let layout = ds::panel(&theme)
            .child(ds::panel_head("Layout", None, &theme))
            .child(setting(
                "Sidebar and panels",
                format!(
                    "Sidebar {:.0}px. Drag its edge to resize; Ctrl+B collapses it to a rail.",
                    self.sidebar_width
                ),
                Button::new("reset-layout")
                    .secondary()
                    .label("Reset layout")
                    .on_click({
                        let entity = entity.clone();
                        move |_, window, cx| {
                            entity.update(cx, |this, cx| this.reset_layout(window, cx))
                        }
                    }),
                &theme,
            ));

        let about = ds::panel(&theme)
            .child(ds::panel_head("About", None, &theme))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .child(self.brand_marks[2].clone())
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(ty::BODY))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("OriginLM control center"),
                            )
                            .child(ds::mono(
                                format!(
                                    "v{} · protocol {} · alpha, unsigned installers",
                                    env!("CARGO_PKG_VERSION"),
                                    self.snapshot()
                                        .map(|s| s.protocol_version.clone())
                                        .unwrap_or_else(|| "1.0".into())
                                ),
                                ty::CAPTION,
                                theme.muted_foreground,
                                &theme,
                            )),
                    )
                    .child(ds::link("about-diagnostics", "Diagnostics", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| entity.update(cx, |this, cx| this.set_page(Page::Diagnostics, cx))
                    })),
            )
            .child(
                div().px(px(sp::LG)).pb(px(sp::MD)).child(ds::caption(
                    "Reads the runtime through its public authenticated API. With no shared broker running it hosts a private runtime for this window that runs passive discovery and read-only metadata probes. It never opens the broker database or reads provider credentials. No telemetry.",
                    &theme,
                )),
            );

        ds::page()
            .gap(px(sp::MD))
            .child(appearance)
            .child(source)
            .child(refresh)
            .child(layout)
            .child(self.price_settings(cx))
            .child(about)
            .into_any_element()
    }
}
