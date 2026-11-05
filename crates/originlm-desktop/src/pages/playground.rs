//! Playground: a warm, chat-first surface for one explicit test turn, with the
//! route and billing shown before anything runs. The layout and motion borrow
//! the useful parts of the Twinhaus Studio chat surface without its 3D shell:
//! a compact model rail, right-aligned user bubbles, full-width assistant
//! prose, a live run card, and a composer that becomes an animated Stop.
use crate::app::{Desktop, Page, TranscriptRole};
use crate::ds::{self, sp, ty};
use crate::ui::Tone;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    input::Textarea,
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::f32::consts::PI;
use std::time::Duration;

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
        let selected_provider = selected
            .as_ref()
            .map(|choice| choice.provider.clone())
            .unwrap_or_else(|| "No route selected".into());
        let selected_icon = selected
            .as_ref()
            .map(|choice| choice.provider_id.clone())
            .unwrap_or_default();

        // Model rail. The chat stays the main surface; this is the compact
        // "app + model" switch a conversation needs.
        let picker = ds::panel(&theme)
            .child(ds::panel_head(
                "Model",
                Some(
                    ds::link("pick-elsewhere", "All models", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| {
                            entity.update(cx, |this, cx| this.set_page(Page::Models, cx))
                        }
                    })
                    .into_any_element(),
                ),
                &theme,
            ))
            .when(choices.is_empty(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "No runnable model",
                    if self.bundle.is_none() {
                        "Waiting for the first snapshot from the runtime."
                    } else {
                        "No signed-in account reports a model with a text driver. Check Accounts."
                    },
                    &theme,
                ))
            })
            .child(
                div()
                    .v_flex()
                    .py(px(sp::XS))
                    .children(choices.iter().take(8).map(|choice| {
                        let id = choice.model_id.clone();
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
                                cx.notify();
                            });
                        })
                    })),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.border)
                    .px(px(sp::LG))
                    .py(px(sp::MD))
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
                            .gap(px(2.))
                            .child(div().text_size(px(ty::SMALL)).child("Allow paid routes"))
                            .child(ds::caption("For the next run of this model only.", &theme)),
                    ),
            );

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
                            .rounded(px(9.))
                            .bg(theme.muted)
                            .pl(px(6.))
                            .pr(px(8.))
                            .h(px(30.))
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
                                    .max_w(px(190.))
                                    .truncate()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.muted_foreground)
                                    .child(selected_label),
                            )
                            .child(ds::caption(selected_provider, &theme).truncate()),
                    )
                    .child(send),
            );

        let transcript = ds::panel(&theme)
            .flex_1()
            .min_w(px(0.))
            .child(header)
            .child(
                div()
                    .id("playground-transcript")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(lines),
            )
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

        let wide = self.content_width() >= 1000.;
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
        page.child(if wide {
            div()
                .h_flex()
                .items_start()
                .gap(px(sp::MD))
                .child(div().w(px(340.)).flex_shrink_0().child(picker))
                .child(transcript)
                .into_any_element()
        } else {
            div()
                .v_flex()
                .gap(px(sp::MD))
                .child(picker)
                .child(transcript)
                .into_any_element()
        })
        .into_any_element()
    }
}
