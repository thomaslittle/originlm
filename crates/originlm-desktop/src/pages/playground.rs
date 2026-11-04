//! Playground: one explicit test turn, with the route and billing shown
//! before anything runs.

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

impl Desktop {
    pub fn page_playground(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let choices = self.model_choices();
        let selected = self.selected_choice();
        let running = self.running;
        let paid_consent = self
            .paid_consent
            .is_armed_for(self.selected_model.as_deref());

        // Model choice.
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
                    .children(choices.iter().take(6).map(|choice| {
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
            );

        // Prompt and run.
        let summary = match selected.as_ref() {
            Some(choice) => div()
                .v_flex()
                .child(ds::kv(
                    "Route",
                    div()
                        .text_size(px(ty::SMALL))
                        .truncate()
                        .child(format!("{} · {}", choice.provider, choice.connection)),
                    &theme,
                ))
                .child(ds::kv(
                    "Billing",
                    ds::chip(choice.billing.clone(), Tone::Neutral, &theme),
                    &theme,
                ))
                .child(ds::kv(
                    "Permissions",
                    div()
                        .text_size(px(ty::SMALL))
                        .child("Read-only workspace, no retries"),
                    &theme,
                ))
                .into_any_element(),
            None => ds::caption(
                "Choose a model so the route and billing are explicit.",
                &theme,
            )
            .into_any_element(),
        };
        let prompt = ds::panel(&theme)
            .child(ds::panel_head("Prompt", None, &theme))
            .child(
                div()
                    .v_flex()
                    .gap(px(sp::MD))
                    .p(px(sp::LG))
                    .child(Textarea::new(&self.prompt).h(px(108.)))
                    .child(summary)
                    .child(
                        div()
                            .h_flex()
                            .items_start()
                            .gap(px(sp::MD))
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
                                    .child(
                                        div()
                                            .text_size(px(ty::SMALL))
                                            .child("Allow paid or unknown-billing routes"),
                                    )
                                    .child(ds::caption(
                                        "For the next run of this model only, then it turns off.",
                                        &theme,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::MD))
                            .child(if running {
                                let entity = entity.clone();
                                Button::new("cancel-turn")
                                    .secondary()
                                    .label("Cancel")
                                    .icon(IconName::Square)
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.cancel_turn(cx))
                                    })
                                    .into_any_element()
                            } else {
                                let entity = entity.clone();
                                Button::new("run-turn")
                                    .primary()
                                    .label("Run one turn")
                                    .icon(IconName::Play)
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.run_prompt(cx))
                                    })
                                    .into_any_element()
                            })
                            .child(ds::caption(
                                if running {
                                    "Streaming…"
                                } else {
                                    "Runs once. Nothing runs in the background."
                                },
                                &theme,
                            )),
                    )
                    .when_some(self.playground_error.clone(), |this, error| {
                        this.child(ds::banner(
                            Tone::Red,
                            "The turn did not run",
                            Some(error.into()),
                            &theme,
                        ))
                    }),
            );

        // Transcript.
        let transcript = ds::panel(&theme)
            .flex_1()
            .min_w(px(0.))
            .child(ds::panel_head(
                "Transcript",
                self.usage_note.clone().map(|usage| {
                    ds::mono(format!("Last turn: {usage}"), ty::CAPTION, theme.muted_foreground, &theme)
                        .into_any_element()
                }),
                &theme,
            ))
            .when(self.transcript.is_empty(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "Nothing has run",
                    "Choose a model, write a prompt and press Run one turn. The route and billing are shown before anything starts.",
                    &theme,
                ))
            })
            .child(div().v_flex().gap(px(sp::MD)).p(px(sp::LG)).children(
                self.transcript.iter().map(|line| {
                    let you = line.role == TranscriptRole::You;
                    div()
                        .v_flex()
                        .gap(px(sp::XS))
                        .child(ds::caption(if you { "You" } else { "Assistant" }, &theme))
                        .child(
                            div()
                                .px(px(sp::MD))
                                .py(px(sp::SM))
                                .rounded(px(ds::radius::PANEL))
                                .border_1()
                                .border_color(theme.border)
                                .when(you, |this| this.bg(theme.muted))
                                .text_size(px(ty::BODY))
                                .line_height(px(20.))
                                .child(if line.text.is_empty() {
                                    "…".to_string()
                                } else {
                                    line.text.clone()
                                }),
                        )
                }),
            ));

        let wide = self.content_width() >= 1000.;
        let mut page = ds::page();
        if self.example {
            page = page.child(ds::banner(
                Tone::Amber,
                "Example mode cannot run turns",
                Some(
                    "Nothing is sent to any provider. The controls work so the flow can be reviewed; Run reports that execution is disabled."
                        .into(),
                ),
                &theme,
            ));
        }
        let left = div().v_flex().gap(px(sp::MD)).child(prompt).child(picker);
        page.child(if wide {
            div()
                .h_flex()
                .items_start()
                .gap(px(sp::MD))
                .child(div().w(px(420.)).flex_shrink_0().child(left))
                .child(transcript)
                .into_any_element()
        } else {
            div()
                .v_flex()
                .gap(px(sp::MD))
                .child(left)
                .child(transcript)
                .into_any_element()
        })
        .into_any_element()
    }
}
