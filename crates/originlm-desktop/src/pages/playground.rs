//! Playground: a warm, chat-first surface for one explicit test turn, with the
//! route and billing shown before anything runs. The layout and motion borrow
//! the useful parts of the Twinhaus Studio chat surface without its 3D shell:
//! a compact model rail, right-aligned user bubbles, full-width assistant
//! prose, a live run card, and a composer that becomes an animated Stop.
use crate::app::{Desktop, TranscriptRole};
use crate::ds::{self, sp, ty};
use crate::ui::Tone;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, Size,
    button::{Button, ButtonVariants},
    input::{Input, Textarea},
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::f32::consts::PI;
use std::time::Duration;

fn asset_icon(name: gpui_kit::assets::IconName, size: f32, color: gpui::Hsla) -> impl IntoElement {
    Icon::new(name)
        .with_size(Size::Size(px(size)))
        .text_color(color)
}

impl Desktop {
    fn playground_status(&self) -> (&'static str, bool) {
        if self.running {
            ("Writing", true)
        } else if self.playground_error.is_some() {
            ("Needs attention", false)
        } else if self.transcript.is_empty() {
            ("Ready", false)
        } else {
            ("Done", false)
        }
    }

    fn playground_live_dot(&self, theme: &gpui_kit::component::Theme, reduce: bool) -> AnyElement {
        let live = self.running;
        let dot = div().w(px(7.)).h(px(7.)).rounded_full().bg(if live {
            theme.primary
        } else if self.playground_error.is_some() {
            gpui::rgb(0xe8826a).into()
        } else {
            theme.muted_foreground
        });
        if live && !reduce {
            dot.with_animation(
                "playground-live-dot",
                Animation::new(Duration::from_millis(1_400)).repeat(),
                |dot, progress| {
                    let pulse = 0.5 - 0.5 * (progress * PI * 2.0).cos();
                    dot.opacity(0.45 + 0.55 * pulse)
                },
            )
            .into_any_element()
        } else {
            dot.into_any_element()
        }
    }

    pub fn page_playground(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let reduce = cx.reduce_motion();
        let entity = cx.entity();
        let choices = self.model_choices();
        let selected = self.selected_choice();
        let running = self.running;
        let paid_consent = self
            .paid_consent
            .is_armed_for(self.selected_model.as_deref());
        let (status, live) = self.playground_status();
        let selected_label = selected
            .as_ref()
            .map(|choice| choice.label.clone())
            .unwrap_or_else(|| "Choose a model".into());
        let selected_icon = selected
            .as_ref()
            .map(|choice| choice.provider_id.clone())
            .unwrap_or_default();

        // Model rail. The chat stays the main surface; this is the compact
        // "app + model" switch a conversation needs.
        // Provider tabs and a bounded model list live in the composer popover.
        // No navigation to another page and no unbounded scroll surface.
        let mut providers: Vec<(String, String)> = Vec::new();
        for choice in &choices {
            if !providers
                .iter()
                .any(|(provider, _)| provider == &choice.provider_id)
            {
                providers.push((
                    choice.provider_id.clone(),
                    self.provider_name(&choice.provider_id),
                ));
            }
        }
        providers.sort_by(|a, b| a.1.cmp(&b.1));
        let active_provider = self.model_provider_filter.clone();
        let query = self.model_query.trim().to_ascii_lowercase();
        let visible_choices: Vec<_> = choices
            .iter()
            .filter(|choice| {
                active_provider
                    .as_ref()
                    .is_none_or(|provider| provider == &choice.provider_id)
            })
            .filter(|choice| {
                query.is_empty()
                    || choice.label.to_ascii_lowercase().contains(&query)
                    || choice.model_id.to_ascii_lowercase().contains(&query)
                    || choice.connection.to_ascii_lowercase().contains(&query)
            })
            .collect();

        let mut tabs: Vec<(Option<String>, String)> = vec![(None, "All".into())];
        tabs.extend(
            providers
                .iter()
                .map(|(id, label)| (Some(id.clone()), label.clone())),
        );
        let provider_tabs = div()
            .h_flex()
            .flex_wrap()
            .gap(px(4.))
            .px(px(10.))
            .pb(px(8.))
            .children(tabs.into_iter().map(|(provider, label)| {
                let selected = active_provider == provider;
                let entity = entity.clone();
                let id = provider
                    .as_deref()
                    .map(|provider| format!("model-provider-{provider}"))
                    .unwrap_or_else(|| "model-provider-all".into());
                div()
                    .id(SharedString::from(id))
                    .role(gpui::Role::Tab)
                    .aria_label(label.clone())
                    .h_flex()
                    .items_center()
                    .h(px(28.))
                    .px(px(9.))
                    .rounded(px(8.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(selected, |this| {
                        this.bg(theme.muted)
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.foreground)
                    })
                    .when(!selected, |this| {
                        this.text_color(theme.muted_foreground)
                            .hover(|style| style.bg(theme.muted).text_color(theme.foreground))
                    })
                    .children(provider.as_deref().and_then(|provider| {
                        crate::icons::provider_mark(
                            provider,
                            12.,
                            if selected {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            },
                        )
                    }))
                    .child(label)
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.model_provider_filter = provider.clone();
                            cx.notify();
                        });
                    })
            }));

        let visible_count = visible_choices.len();
        let model_rows = if visible_choices.is_empty() {
            ds::state(
                ds::StateKind::Empty,
                "No models match",
                if query.is_empty() {
                    "Choose another provider tab or refresh the model catalog."
                } else {
                    "Search matches model names, ids, providers and connections."
                },
                &theme,
            )
            .into_any_element()
        } else {
            div()
                .v_flex()
                .py(px(4.))
                .children(visible_choices.into_iter().map(|choice| {
                    let id = choice.model_id.clone();
                    let provider_id = choice.provider_id.clone();
                    let is_selected =
                        self.selected_model.as_deref() == Some(choice.model_id.as_str());
                    let entity = entity.clone();
                    ds::select_row(
                        SharedString::from(format!("choice-{id}")),
                        is_selected,
                        &theme,
                    )
                    .aria_label(format!("{}, {}", choice.label, choice.connection))
                    .children(crate::icons::provider_mark(
                        &choice.provider_id,
                        14.,
                        theme.muted_foreground,
                    ))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(choice.label.clone()),
                            )
                            .child(ds::caption(choice.connection.clone(), &theme).truncate()),
                    )
                    .child(ds::chip(choice.billing.clone(), Tone::Neutral, &theme))
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.selected_model = Some(id.clone());
                            this.selected_effort = None;
                            this.model_provider_filter = Some(provider_id.clone());
                            this.model_picker_open = false;
                            this.effort_picker_open = false;
                            cx.notify();
                        });
                    })
                }))
                .into_any_element()
        };

        let picker = div()
            .id("model-picker")
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .rounded(px(14.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .v_flex()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .pt(px(10.))
                    .pb(px(6.))
                    .child(
                        div()
                            .text_size(px(ty::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Choose a model"),
                    )
                    .child({
                        let entity = entity.clone();
                        div()
                            .id("close-model-picker")
                            .role(gpui::Role::Button)
                            .aria_label("Close model picker")
                            .w(px(22.))
                            .h(px(22.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.muted))
                            .child(asset_icon(
                                gpui_kit::assets::IconName::X,
                                12.,
                                theme.muted_foreground,
                            ))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.model_picker_open = false;
                                    cx.notify();
                                });
                            })
                    }),
            )
            .child(
                div()
                    .px(px(10.))
                    .pb(px(8.))
                    .child(Input::new(&self.model_search)),
            )
            .child(provider_tabs)
            .child(
                div()
                    .id("model-picker-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(model_rows),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .px(px(12.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        Switch::new("paid-consent")
                            .checked(paid_consent)
                            .on_change({
                                let entity = entity.clone();
                                move |checked, _, cx| {
                                    let checked = *checked;
                                    entity.update(cx, |this, cx| {
                                        let model = this.selected_model.clone();
                                        this.paid_consent.set(model.as_deref(), checked);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .child("Allow paid or unknown-billing routes"),
                            )
                            .child(ds::caption("For the next run of this model only.", &theme)),
                    ),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(7.))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(ds::caption(
                        format!(
                            "{} model{} · {} provider{}",
                            visible_count,
                            if visible_count == 1 { "" } else { "s" },
                            providers.len(),
                            if providers.len() == 1 { "" } else { "s" },
                        ),
                        &theme,
                    )),
            );

        let effort_choices = self.effort_choices();
        let effort_picker = div()
            .id("effort-picker")
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .rounded(px(14.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .v_flex()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .pt(px(10.))
                    .pb(px(6.))
                    .child(
                        div()
                            .text_size(px(ty::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Reasoning effort"),
                    )
                    .child({
                        let entity = entity.clone();
                        div()
                            .id("close-effort-picker")
                            .role(gpui::Role::Button)
                            .aria_label("Close effort picker")
                            .w(px(22.))
                            .h(px(22.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.muted))
                            .child(asset_icon(
                                gpui_kit::assets::IconName::X,
                                12.,
                                theme.muted_foreground,
                            ))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.effort_picker_open = false;
                                    cx.notify();
                                });
                            })
                    }),
            )
            .child(if effort_choices.is_empty() {
                div()
                    .px(px(12.))
                    .pb(px(12.))
                    .child(ds::caption(
                        "This model exposes only its default effort. Pick a model that advertises effort levels to change it.",
                        &theme,
                    ))
                    .into_any_element()
            } else {
                let current = self.effort_value();
                div()
                    .v_flex()
                    .py(px(4.))
                    .children(effort_choices.into_iter().map(|(value, label)| {
                        let selected = current.as_deref() == Some(value.as_str());
                        let entity = entity.clone();
                        ds::select_row(
                            SharedString::from(format!("effort-option-{value}")),
                            selected,
                            &theme,
                        )
                        .aria_label(label.clone())
                        .child(div().flex_1().child(label))
                        .when(selected, |this| {
                            this.child(asset_icon(
                                gpui_kit::assets::IconName::Check,
                                14.,
                                theme.primary,
                            ))
                        })
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.selected_effort = Some(value.clone());
                                this.effort_picker_open = false;
                                cx.notify();
                            });
                        })
                    }))
                    .into_any_element()
            });

        // Chat header: the selected model and the live run state are always
        // visible, even when the transcript is empty.
        let header = div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(sp::MD))
            .border_b_1()
            .border_color(theme.border)
            .px(px(sp::LG))
            .py(px(sp::MD))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .child(
                        div()
                            .w(px(30.))
                            .h(px(30.))
                            .rounded_full()
                            .bg(theme.muted)
                            .child(
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .children(if selected_icon.is_empty() {
                                        crate::icons::provider_mark("local", 15., theme.primary)
                                    } else {
                                        crate::icons::provider_mark(
                                            &selected_icon,
                                            15.,
                                            theme.primary,
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(px(ty::BODY))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Playground"),
                            )
                            .child(ds::caption(
                                if live {
                                    "One explicit turn is running."
                                } else {
                                    "One explicit turn. Nothing runs in the background."
                                },
                                &theme,
                            )),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::SM))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::XS))
                            .child(self.playground_live_dot(&theme, reduce))
                            .child(
                                div()
                                    .text_size(px(ty::CAPTION))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if live {
                                        theme.primary
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(status),
                            ),
                    )
                    .child(ds::caption(selected_label.clone(), &theme)),
            );

        // Transcript. User turns are compact right-aligned bubbles; assistant
        // prose stays on the panel, like the Twinhaus chat surface.
        let mut lines = div().v_flex().gap(px(18.)).p(px(sp::LG));
        if self.transcript.is_empty() {
            lines = lines.child(ds::state(
                ds::StateKind::Empty,
                "Start a conversation",
                "Choose a model, write a prompt and press Send. The route and billing are shown before anything starts.",
                &theme,
            ));
        }
        for (index, line) in self.transcript.iter().enumerate() {
            let you = line.role == TranscriptRole::You;
            let message = if you {
                div()
                    .w_full()
                    .flex()
                    .justify_end()
                    .child(
                        div()
                            .max_w(px(360.))
                            .rounded(px(14.))
                            .bg(theme.muted)
                            .px(px(13.))
                            .py(px(9.))
                            .text_size(px(13.5))
                            .line_height(px(20.))
                            .child(line.text.clone()),
                    )
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .v_flex()
                    .gap(px(5.))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::XS))
                            .child(div().w(px(5.)).h(px(5.)).rounded_full().bg(theme.primary))
                            .child(ds::caption(
                                if selected_label.is_empty() {
                                    "Assistant".to_owned()
                                } else {
                                    format!("Assistant · {selected_label}")
                                },
                                &theme,
                            )),
                    )
                    .child(div().text_size(px(13.5)).line_height(px(21.)).child(
                        if line.text.is_empty() {
                            "…".to_string()
                        } else {
                            line.text.clone()
                        },
                    ))
                    .into_any_element()
            };
            let message = if reduce {
                message
            } else {
                div()
                    .with_animation(
                        SharedString::from(format!("playground-msg-{index}")),
                        Animation::new(Duration::from_millis(240)),
                        |el, progress| el.opacity(progress).mt(px(6.) * (1.0 - progress)),
                    )
                    .child(message)
                    .into_any_element()
            };
            lines = lines.child(message);
        }
        if running {
            let thinking = div()
                .id("playground-thinking")
                .h_flex()
                .items_center()
                .gap(px(sp::SM))
                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(theme.primary))
                .child(
                    div()
                        .text_size(px(ty::SMALL))
                        .text_color(theme.muted_foreground)
                        .child("Thinking"),
                )
                .child(
                    div()
                        .h(px(2.))
                        .flex_1()
                        .rounded_full()
                        .bg(theme.border)
                        .child(div().h(px(2.)).w(px(42.)).rounded_full().bg(theme.primary)),
                );
            lines = lines.child(if reduce {
                thinking.into_any_element()
            } else {
                thinking
                    .with_animation(
                        "playground-thinking-pulse",
                        Animation::new(Duration::from_millis(1_900)).repeat(),
                        |el, progress| {
                            let sweep = -0.35 + progress * 1.7;
                            el.opacity(0.55 + 0.45 * (sweep * PI).sin().abs())
                        },
                    )
                    .into_any_element()
            });
        }

        // Composer: the input and the send/stop control share one warm card.
        let send = if running {
            let entity = entity.clone();
            let button = Button::new("cancel-turn")
                .secondary()
                .label("Stop")
                .icon(IconName::Square)
                .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.cancel_turn(cx)));
            if reduce {
                button.into_any_element()
            } else {
                button
                    .with_animation(
                        "playground-stop-ring",
                        Animation::new(Duration::from_millis(1_400)).repeat(),
                        |el, progress| {
                            let pulse = 0.5 - 0.5 * (progress * PI * 2.0).cos();
                            el.opacity(0.82 + 0.18 * pulse)
                        },
                    )
                    .into_any_element()
            }
        } else {
            let entity = entity.clone();
            Button::new("run-turn")
                .primary()
                .label("Send")
                .icon(IconName::Play)
                .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.run_prompt(cx)))
                .into_any_element()
        };
        let attachment_tray = if self.attachments.is_empty() {
            None
        } else {
            Some(div().h_flex().flex_wrap().gap(px(6.)).children(
                self.attachments.iter().enumerate().map(|(index, path)| {
                    let entity = entity.clone();
                    let path = path.clone();
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(7.))
                        .bg(theme.muted)
                        .pl(px(7.))
                        .pr(px(4.))
                        .py(px(3.))
                        .text_size(px(10.5))
                        .text_color(theme.muted_foreground)
                        .child(asset_icon(
                            gpui_kit::assets::IconName::Paperclip,
                            11.,
                            theme.primary,
                        ))
                        .child(
                            div().max_w(px(150.)).truncate().child(
                                path.file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or("image")
                                    .to_owned(),
                            ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("remove-attachment-{index}")))
                                .w(px(16.))
                                .h(px(16.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.popover))
                                .child(asset_icon(
                                    gpui_kit::assets::IconName::X,
                                    10.,
                                    theme.muted_foreground,
                                ))
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.attachments.retain(|current| current != &path);
                                        cx.notify();
                                    });
                                }),
                        )
                }),
            ))
        };

        let attach_entity = entity.clone();
        let attach = div()
            .id("attach-images")
            .role(gpui::Role::Button)
            .aria_label("Attach images")
            .w(px(30.))
            .h(px(30.))
            .rounded(px(9.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(theme.muted))
            .child(asset_icon(
                gpui_kit::assets::IconName::Paperclip,
                15.,
                theme.muted_foreground,
            ))
            .on_click(move |_, _, cx| {
                attach_entity.update(cx, |this, cx| this.pick_attachments(cx))
            });

        let model_entity = entity.clone();
        let model_chip = div()
            .id("model-chip")
            .role(gpui::Role::Button)
            .aria_label("Choose a model")
            .h_flex()
            .items_center()
            .gap(px(6.))
            .h(px(30.))
            .pl(px(6.))
            .pr(px(7.))
            .rounded(px(9.))
            .border_1()
            .border_color(if self.model_picker_open {
                theme.border
            } else {
                gpui::transparent_black()
            })
            .when(self.model_picker_open, |this| this.bg(theme.muted))
            .cursor_pointer()
            .hover(|style| style.bg(theme.muted))
            .children(crate::icons::provider_mark(
                if selected_icon.is_empty() {
                    "local"
                } else {
                    selected_icon.as_str()
                },
                14.,
                theme.primary,
            ))
            .child(
                div()
                    .max_w(px(170.))
                    .truncate()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if self.model_picker_open {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(selected_label),
            )
            .child(asset_icon(
                gpui_kit::assets::IconName::ChevronDown,
                12.,
                theme.muted_foreground,
            ))
            .on_click(move |_, _, cx| {
                model_entity.update(cx, |this, cx| {
                    this.model_picker_open = !this.model_picker_open;
                    this.effort_picker_open = false;
                    cx.notify();
                });
            });

        let effort = {
            let label = self
                .effort_label()
                .unwrap_or_else(|| "Default effort".to_string());
            let entity = entity.clone();
            div()
                .id("effort-chip")
                .role(gpui::Role::Button)
                .aria_label("Reasoning effort")
                .h_flex()
                .items_center()
                .gap(px(5.))
                .h(px(30.))
                .px(px(8.))
                .rounded(px(9.))
                .cursor_pointer()
                .hover(|style| style.bg(theme.muted))
                .child(asset_icon(
                    gpui_kit::assets::IconName::Gauge,
                    14.,
                    theme.muted_foreground,
                ))
                .child(
                    div()
                        .max_w(px(120.))
                        .truncate()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.effort_picker_open = !this.effort_picker_open;
                        this.model_picker_open = false;
                        cx.notify();
                    });
                })
        };

        let composer = div()
            .id("playground-composer")
            .m(px(sp::MD))
            .p(px(10.))
            .rounded(px(14.))
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .v_flex()
            .gap(px(sp::SM))
            .when_some(attachment_tray, |this, tray| this.child(tray))
            .child(Textarea::new(&self.prompt).h(px(64.)))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(sp::SM))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::XS))
                            .child(attach)
                            .child(model_chip)
                            .child(effort),
                    )
                    .child(send),
            );

        let body = if self.model_picker_open {
            picker.into_any_element()
        } else if self.effort_picker_open {
            effort_picker.into_any_element()
        } else {
            div()
                .id("playground-transcript")
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .child(lines)
                .into_any_element()
        };
        let transcript = ds::panel(&theme)
            .flex_1()
            .min_w(px(0.))
            .child(header)
            .child(body)
            .when_some(self.playground_error.clone(), |this, error| {
                this.child(div().p(px(sp::MD)).child(ds::banner(
                    Tone::Red,
                    "The turn did not run",
                    Some(error.into()),
                    &theme,
                )))
            })
            .when_some(self.usage_note.clone(), |this, usage| {
                this.child(
                    div()
                        .px(px(sp::LG))
                        .py(px(sp::XS))
                        .border_t_1()
                        .border_color(theme.border)
                        .child(ds::mono(
                            format!("Last turn: {usage}"),
                            ty::CAPTION,
                            theme.muted_foreground,
                            &theme,
                        )),
                )
            })
            .child(composer);

        let mut page = ds::page();
        if self.example {
            page = page.child(ds::banner(
                Tone::Amber,
                "Example mode cannot run turns",
                Some(
                    "Nothing is sent to any provider. The controls work so the flow can be reviewed; Send reports that execution is disabled."
                        .into(),
                ),
                &theme,
            ));
        }
        page.child(transcript).into_any_element()
    }
}
