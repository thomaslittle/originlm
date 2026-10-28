use crate::app::Desktop;
use crate::ui;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let address = originlm_platform::read_private_file(&self.token_path, 16 * 1024)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("address")
                    .and_then(|address| address.as_str())
                    .map(|address| address.to_string())
            })
            .unwrap_or_else(|| "unavailable".into());
        let state_dir = crate::history::state_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Unavailable; preferences are not persisted".into());
        let entity_theme = cx.entity();
        let entity_refresh = cx.entity();
        let entity_dir = cx.entity();
        let refresh = self.refresh_seconds;

        div()
            .v_flex()
            .w_full()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .gap_4()
            .child(self.price_settings(cx))
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("Appearance", &theme))
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(1.))
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .child("Dark graphite"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(theme.muted_foreground)
                                            .child("Warm off-white text, amber accent, deliberate light theme."),
                                    ),
                            )
                            .child(
                                Switch::new("theme-switch")
                                    .checked(self.dark)
                                    .on_change(move |checked, _, cx| {
                                        let checked = *checked;
                                        entity_theme.update(cx, |this, cx| {
                                            if this.dark != checked {
                                                this.toggle_theme(cx);
                                            }
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("Appearance preferences apply to this app."),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("Data source", &theme))
                    .child(row(
                        "Broker socket",
                        &address,
                        false,
                        &theme,
                    ))
                    .child(row(
                        "Credential file",
                        &self.token_path.display().to_string(),
                        true,
                        &theme,
                    ))
                    .child(row(
                        "Application administration",
                        if self.example {
                            "Example mode; no credentials loaded"
                        } else if self.bundle.as_ref().is_some_and(|bundle| bundle.error.is_none() && bundle.apps_error.is_none()) {
                            "Confirmed by the runtime"
                        } else {
                            "Not confirmed; inspect Connections for the response"
                        },
                        false,
                        &theme,
                    ))
                    .child(ui::eyebrow("History and preferences", &theme))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("copy-state-dir")
                                    .font_family(theme.mono_font_family.clone())
                                    .text_size(px(10.))
                                    .text_color(theme.muted_foreground)
                                    .cursor_pointer()
                                    .child(state_dir.clone())
                                    .on_click(move |_, _, cx| {
                                        let dir = state_dir.clone();
                                        entity_dir.update(cx, |this, cx| {
                                            cx.write_to_clipboard(
                                                gpui::ClipboardItem::new_string(dir),
                                            );
                                            this.toast("State directory copied");
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(ui::icon(IconName::Copy, 10., theme.muted_foreground)),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("Credentials are never displayed, logged or included in exports."),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("Layout", &theme))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "Sidebar {:.0}px, Ctrl+B toggles it",
                                        self.sidebar_width
                                    )),
                            )
                            .child(div().flex_1())
                            .child({
                                let entity = cx.entity();
                                Button::new("reset-layout")
                                    .secondary()
                                    .label("Reset layout")
                                    .on_click(move |_, window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.reset_layout(window, cx)
                                        });
                                    })
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("Drag the divider beside the sidebar to resize it. The collapsed rail keeps every page one click away."),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("Refresh", &theme))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .children([15u64, 30, 60].into_iter().map(|seconds| {
                                let entity = entity_refresh.clone();
                                ui::tab_pill(
                                    SharedString::from(format!("refresh-{seconds}")),
                                    format!("{seconds}s"),
                                    refresh == seconds,
                                    &theme,
                                    move |_, _, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.refresh_seconds = seconds;
                                            this.save_prefs();
                                            this.refresh(cx);
                                        });
                                    },
                                )
                                .into_any_element()
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("The broker refreshes metadata on its own cadence. This setting only controls how often the window reads the cached snapshot, so it never triggers provider work."),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("About", &theme))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                crate::brand::app_tile(30.),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("OriginLM Desktop"),
                                    )
                                    .child(ui::mono(
                                        format!(
                                            "OriginLM · v{}, protocol {}, GPUI Fast",
                                            env!("CARGO_PKG_VERSION"),
                                            self.snapshot()
                                                .map(|s| s.protocol_version.clone())
                                                .unwrap_or_else(|| "1.0".into())
                                        ),
                                        &theme,
                                    )),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("open-diagnostics")
                                    .secondary()
                                    .label("Diagnostics")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_page(crate::app::Page::Diagnostics, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("Reads the shared broker through its public authenticated API. It never opens the broker database, reads provider credentials or starts discovery on its own."),
                    ),
            )
            .into_any_element()
    }
}

fn row(label: &str, value: &str, mono: bool, theme: &gpui_kit::component::Theme) -> AnyElement {
    div()
        .v_flex()
        .gap(px(2.))
        .child(ui::eyebrow(label, theme))
        .child(
            div()
                .when(mono, |this| {
                    this.font_family(theme.mono_font_family.clone())
                })
                .text_size(px(10.5))
                .text_color(theme.foreground)
                .child(value.to_string()),
        )
        .into_any_element()
}
