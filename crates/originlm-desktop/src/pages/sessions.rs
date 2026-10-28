use crate::app::Desktop;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, IconName, button::ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_sessions(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let sessions = self.sessions.clone();

        div()
            .v_flex()
            .w_full()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .p_3()
                    .rounded(px(9.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .child(ui::icon(IconName::CircleAlert, 12., theme.muted_foreground))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.muted_foreground)
                            .child("Sessions are scoped to the client that created them. This view shows the turns started from this app; sessions opened by other applications, terminals or editors are not observable here."),
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
                    .child(ui::section_title("This app's sessions", &theme))
                    .when(sessions.is_empty(), |this| {
                        let entity = cx.entity();
                        this.child(
                            div()
                                .v_flex()
                                .items_center()
                                .gap_2()
                                .child(ui::empty_state(
                                    IconName::Inbox,
                                    "No sessions yet",
                                    "Run one explicit turn from the Playground to see it here. Nothing starts in the background.",
                                    &theme,
                                ))
                                .child(
                                    gpui_kit::component::button::Button::new("go-playground")
                                        .primary()
                                        .label("Open Playground")
                                        .on_click(move |_, _, cx| {
                                            entity.update(cx, |this, cx| {
                                                this.set_page(crate::app::Page::Playground, cx)
                                            });
                                        }),
                                ),
                        )
                    })
                    .children(sessions.iter().enumerate().map(|(index, session)| {
                        let entity = cx.entity();
                        let id = session.id.clone();
                        let running = session.status == "streaming" || session.status == "running";
                        let tone = match session.status.as_str() {
                            "completed" => Tone::Green,
                            "failed" | "error" => Tone::Red,
                            "streaming" | "running" => Tone::Amber,
                            _ => Tone::Neutral,
                        };
                        div()
                            .id(("session", index))
                            .h_flex()
                            .gap_3()
                            .p_3()
                            .rounded(px(9.))
                            .border_1()
                            .border_color(theme.border)
                            .child(ui::dot(tone, &theme))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.))
                                    .flex_1()
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(session.label.clone()),
                                            )
                                            .child(ui::chip(
                                                session.status.clone(),
                                                tone,
                                                &theme,
                                            )),
                                    )
                                    .child(ui::mono(
                                        {
                                            let seconds = session.started.elapsed().as_secs();
                                            let age = if seconds < 60 {
                                                format!("{seconds}s")
                                            } else {
                                                format!("{}m", seconds / 60)
                                            };
                                            format!("{}, {age} ago", session.id)
                                        },
                                        &theme,
                                    )),
                            )
                            .when_some(session.usage.clone(), |this, usage| {
                                this.child(ui::mono(usage, &theme))
                            })
                            .when(running, |this| {
                                this.child(
                                    gpui_kit::component::button::Button::new(SharedString::from(format!("cancel-session-{id}")))
                                    .secondary()
                                    .label("Cancel")
                                    .on_click(move |_, _, cx| {
                                        let id = id.clone();
                                        entity.update(cx, |this, cx| {
                                            let _ = this
                                                .executor
                                                .commands
                                                .send(crate::broker::ExecCommand::Cancel);
                                            let _ = this.executor.commands.send(
                                                crate::broker::ExecCommand::Close(id),
                                            );
                                            this.running = false;
                                            cx.notify();
                                        });
                                    }),
                                )
                            })
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }
}
