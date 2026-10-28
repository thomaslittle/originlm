use crate::app::{Desktop, Page, TranscriptRole};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    input::Textarea,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_playground(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let choices = self.model_choices();
        let selected = self.selected_choice();
        let transcript = self.transcript.clone();
        let transcript_empty = transcript.is_empty();
        let running = self.running;
        let error = self.playground_error.clone();
        let usage = self.usage_note.clone();

        let picker = div()
            .v_flex()
            .gap_2()
            .p_4()
            .rounded(px(ui::CARD_RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.list)
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(ui::section_title("Change model", &theme))
                    .child(
                        div()
                            .id("pick-elsewhere")
                            .text_size(px(10.5))
                            .text_color(theme.primary)
                            .cursor_pointer()
                            .child("Browse all models")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_page(Page::Models, cx);
                            })),
                    ),
            )
            .children(choices.iter().take(4).map(|choice| {
                let entity = cx.entity();
                let id = choice.model_id.clone();
                let is_selected = self
                    .selected_model
                    .as_deref()
                    .is_some_and(|selected| selected == choice.model_id);
                div()
                    .id(SharedString::from(format!("choice-{id}")))
                    .h_flex()
                    .gap_2()
                    .p_2()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if is_selected {
                        theme.primary.opacity(0.5)
                    } else {
                        theme.border
                    })
                    .bg(if is_selected {
                        theme.primary.opacity(0.08)
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.list_hover))
                    .child(ui::icon(
                        if is_selected {
                            IconName::CircleCheck
                        } else {
                            IconName::Bot
                        },
                        12.,
                        if is_selected {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        },
                    ))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(1.))
                            .flex_1()
                            .child(div().text_size(px(11.)).child(choice.label.clone()))
                            .child(ui::mono(
                                if choice.billing.starts_with("unknown") {
                                    choice.connection.clone()
                                } else {
                                    format!("{}, {}", choice.connection, choice.billing)
                                },
                                &theme,
                            )),
                    )
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.selected_model = Some(id.clone());
                            cx.notify();
                        });
                    })
                    .into_any_element()
            }));

        let selection_summary: AnyElement = match selected.as_ref() {
            Some(choice) => div()
                .v_flex()
                .gap_2()
                .p_3()
                .rounded(px(8.))
                .bg(theme.muted)
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::MEDIUM)
                        .child(choice.label.clone()),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .children(crate::icons::provider_mark(
                            &choice.provider_id,
                            15.,
                            theme.foreground,
                        ))
                        .child(ui::mono(choice.connection.clone(), &theme)),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_1()
                        .flex_wrap()
                        .child(ui::chip(choice.provider.clone(), Tone::Blue, &theme))
                        .child(ui::chip(choice.billing.clone(), Tone::Amber, &theme))
                        .child(ui::chip("read-only", Tone::Neutral, &theme))
                        .child(ui::chip("paid off", Tone::Green, &theme)),
                )
                .into_any_element(),
            None => div()
                .text_size(px(11.))
                .text_color(theme.muted_foreground)
                .child("No usable model is visible. Check Providers & accounts first.")
                .into_any_element(),
        };

        let prompt_card = div()
            .v_flex()
            .gap_2()
            .p_4()
            .rounded(px(ui::CARD_RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.list)
            .child(ui::section_title("Prompt", &theme))
            .child(Textarea::new(&self.prompt).h(px(92.)))
            .child(selection_summary)
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(if running {
                        let entity = cx.entity();
                        Button::new("cancel-turn")
                            .secondary()
                            .label("Cancel")
                            .icon(IconName::Square)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.cancel_turn(cx));
                            })
                            .into_any_element()
                    } else {
                        let entity = cx.entity();
                        Button::new("run-turn")
                            .primary()
                            .label("Run one turn")
                            .icon(IconName::Play)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.run_prompt(cx));
                            })
                            .into_any_element()
                    })
                    .child(ui::mono(
                        if running {
                            "streaming…".to_string()
                        } else {
                            "explicit, no retries".to_string()
                        },
                        &theme,
                    )),
            )
            .when_some(error, |this, error| {
                this.child(
                    div()
                        .text_size(px(10.5))
                        .text_color(theme.danger)
                        .child(error),
                )
            });

        let transcript_view: AnyElement = div()
            .v_flex()
            .gap_3()
            .children(transcript.into_iter().map(|line| {
                let (label, tone) = match line.role {
                    TranscriptRole::You => ("You", Tone::Neutral),
                    TranscriptRole::Assistant => ("Assistant", Tone::Amber),
                };
                div()
                    .v_flex()
                    .gap_1()
                    .p_3()
                    .rounded(px(9.))
                    .bg(theme.muted)
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(ui::eyebrow(label, &theme))
                            .when(line.role == TranscriptRole::Assistant, |this| {
                                this.child(ui::dot(tone, &theme))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(19.))
                            .child(if line.text.is_empty() {
                                "…".to_string()
                            } else {
                                line.text.clone()
                            }),
                    )
                    .into_any_element()
            }))
            .when(transcript_empty, |this| {
                this.child(ui::empty_state(
                    IconName::Play,
                    "Nothing has run",
                    "Choose a model, write a prompt and press Run. The route and billing are shown before anything starts.",
                    &theme,
                ))
            })
            .into_any_element();

        div()
            .h_flex()
            .items_stretch()
            .gap_4()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .child(
                div()
                    .w(px(400.))
                    .v_flex()
                    .gap_3()
                    .child(prompt_card)
                    .child(picker),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.))
                    .v_flex()
                    .gap_2()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .child(ui::section_title("Transcript", &theme))
                            .when_some(usage, |this, usage| {
                                this.child(ui::mono(format!("Last turn: {usage}"), &theme))
                            }),
                    )
                    .child(div().flex_1().min_h(px(0.)).child(transcript_view)),
            )
            .into_any_element()
    }
}
