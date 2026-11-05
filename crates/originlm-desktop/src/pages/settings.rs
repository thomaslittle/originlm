//! Settings: appearance, data source, refresh, layout, local prices, about.
//! Everyday settings first, advanced tools last.

use crate::app::{Desktop, Page};
use crate::ds::{self, sp, ty};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, Theme,
    button::{Button, ButtonVariants},
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder as _;
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

        let intelligence = self
            .bundle
            .as_ref()
            .map(|bundle| bundle.intelligence.clone())
            .unwrap_or_default();
        let updated = intelligence.fetched_at.as_ref().and_then(|stamp| {
            let at = chrono::DateTime::parse_from_rfc3339(stamp.as_str()).ok()?;
            let minutes = (chrono::Utc::now() - at.with_timezone(&chrono::Utc))
                .num_minutes()
                .max(0);
            Some(if minutes < 60 {
                format!("{minutes}m ago")
            } else {
                format!("{}h ago", minutes / 60)
            })
        });
        let index_status = if !intelligence.configured {
            "No API key in this runtime. Cached scores still display; set ARTIFICIAL_ANALYSIS_API_KEY to sync the free list."
                .to_string()
        } else if intelligence.models.is_empty() {
            "No scores cached yet. The free list needs about two requests; a quota-limited sync retries later."
                .to_string()
        } else {
            format!(
                "{} models · index v{} · updated {}{}",
                intelligence.models.len(),
                intelligence
                    .index_version
                    .map(|version| version.to_string())
                    .unwrap_or_else(|| "?".into()),
                updated.unwrap_or_else(|| "unknown".into()),
                if intelligence.stale { " · stale" } else { "" },
            )
        };
        let intelligence_panel = ds::panel(&theme)
            .child(ds::panel_head(
                "Artificial Analysis Intelligence Index",
                Some(ds::caption("Free data API · cached", &theme).into_any_element()),
                &theme,
            ))
            .child(setting(
                "Model order in OriginLM",
                "Index puts the highest Intelligence Index first. Models without a published score sort last.",
                ds::tabs(
                    "models-sort-setting",
                    ["Index", "Catalog"],
                    usize::from(!self.models_sort_by_intelligence),
                    &theme,
                    {
                        let entity = entity.clone();
                        move |index, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.set_models_sort_by_intelligence(index == 0, cx);
                            })
                        }
                    },
                ),
                &theme,
            ))
            .child(setting(
                "Default order for your apps",
                "Off by default. When enabled, clients that call models.list or models.page without a sort parameter get Index order. A client can always pass sort=intelligence or sort=catalog explicitly.",
                Switch::new("apps-aa-default")
                    .checked(self.apps_default_intelligence_order)
                    .on_change({
                        let entity = entity.clone();
                        move |enabled, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.set_apps_default_intelligence_order(*enabled, cx);
                            })
                        }
                    }),
                &theme,
            ))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap(px(sp::MD))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().flex_1().min_w(px(240.)).child(ds::caption(index_status, &theme)))
                    .child(
                        Button::new("refresh-intelligence")
                            .secondary()
                            .label("Refresh now")
                            .on_click({
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.refresh_intelligence(cx);
                                    })
                                }
                            }),
                    ),
            )
            .when_some(intelligence.last_error.clone(), |this, error| {
                this.child(div().px(px(sp::LG)).pb(px(sp::MD)).child(ds::banner(
                    crate::ui::Tone::Amber,
                    "Intelligence Index sync",
                    Some(error.into()),
                    &theme,
                )))
            })
            .child(
                div()
                    .px(px(sp::LG))
                    .pb(px(sp::MD))
                    .child(ds::link(
                        "settings-aa-attribution",
                        "Scores: Artificial Analysis (artificialanalysis.ai)",
                        &theme,
                        |_, _, cx| cx.open_url("https://artificialanalysis.ai/"),
                    )),
            )
            .child(
                div().px(px(sp::LG)).pb(px(sp::MD)).child(ds::caption(
                    "The runtime syncs the free language-model list at most every six hours (about two requests per sync) and never calls Artificial Analysis from a window read. The key stays in the runtime environment and is never logged or exported.",
                    &theme,
                )),
            );

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
            .child(intelligence_panel)
            .child(layout)
            .child(self.price_settings(cx))
            .child(about)
            .into_any_element()
    }
}
