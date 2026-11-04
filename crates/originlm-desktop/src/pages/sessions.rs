//! Sessions: the sessions this window started, newest first.

use crate::app::{Desktop, Page};
use crate::ds::{self, sp, ty};
use crate::ui::Tone;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

fn status_tone(status: &str) -> Tone {
    match status {
        "completed" => Tone::Green,
        "failed" | "error" => Tone::Red,
        "streaming" | "running" => Tone::Amber,
        _ => Tone::Neutral,
    }
}

impl Desktop {
    pub fn page_sessions(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let sessions = self.sessions.clone();
        let entity = cx.entity();

        let body: AnyElement = if sessions.is_empty() {
            ds::state(
                ds::StateKind::Empty,
                "No sessions yet",
                if self.example {
                    "Example mode runs nothing, so no session is ever started here. With a runtime, every turn you run in the Playground opens a session that is listed here."
                } else {
                    "Run one explicit turn from the Playground and its session appears here. Nothing starts in the background."
                },
                &theme,
            )
            .child(
                div().pt(px(sp::SM)).child(
                    Button::new("go-playground")
                        .secondary()
                        .label("Open Playground")
                        .icon(IconName::Play)
                        .on_click({
                            let entity = entity.clone();
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| this.set_page(Page::Playground, cx))
                            }
                        }),
                ),
            )
            .into_any_element()
        } else {
            div()
                .v_flex()
                .child(
                    ds::table_head(&theme)
                        .child(ds::th("Session", None, &theme))
                        .child(ds::th("Status", Some(110.), &theme))
                        .child(ds::th("Started", Some(90.), &theme))
                        .child(ds::th("Usage", Some(220.), &theme))
                        .child(ds::th("", Some(90.), &theme)),
                )
                .children(sessions.iter().enumerate().map(|(index, session)| {
                    let running = session.status == "streaming" || session.status == "running";
                    let id = session.id.clone();
                    let entity = entity.clone();
                    let seconds = session.started.elapsed().as_secs();
                    let age = if seconds < 60 {
                        format!("{seconds}s ago")
                    } else if seconds < 3600 {
                        format!("{}m ago", seconds / 60)
                    } else {
                        format!("{}h ago", seconds / 3600)
                    };
                    ds::panel_row(index == 0, &theme)
                        .py(px(sp::SM))
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .text_size(px(ty::BODY))
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(session.label.clone()),
                                )
                                .child(ds::copy_value(
                                    format!("session-{index}"),
                                    session.id.clone(),
                                    36,
                                    &theme,
                                )),
                        )
                        .child(div().h_flex().w(px(110.)).flex_shrink_0().child(ds::chip(
                            session.status.clone(),
                            status_tone(&session.status),
                            &theme,
                        )))
                        .child(
                            div()
                                .w(px(90.))
                                .flex_shrink_0()
                                .child(ds::caption(age, &theme)),
                        )
                        .child(
                            div().w(px(220.)).flex_shrink_0().child(
                                ds::caption(
                                    session
                                        .usage
                                        .clone()
                                        .unwrap_or_else(|| "Not reported".into()),
                                    &theme,
                                )
                                .truncate(),
                            ),
                        )
                        .child(div().w(px(90.)).flex_shrink_0().when(running, |this| {
                            this.child(
                                Button::new(SharedString::from(format!("cancel-session-{id}")))
                                    .secondary()
                                    .label("Cancel")
                                    .on_click(move |_, _, cx| {
                                        let id = id.clone();
                                        entity.update(cx, |this, cx| {
                                            let _ = this
                                                .executor
                                                .commands
                                                .send(crate::broker::ExecCommand::Cancel);
                                            let _ = this
                                                .executor
                                                .commands
                                                .send(crate::broker::ExecCommand::Close(id));
                                            this.running = false;
                                            cx.notify();
                                        });
                                    }),
                            )
                        }))
                }))
                .into_any_element()
        };

        ds::page()
            .child(
                ds::panel(&theme)
                    .child(ds::panel_head(
                        "Started from this window",
                        Some(
                            ds::caption(crate::ui::plural(sessions.len(), "session"), &theme)
                                .into_any_element(),
                        ),
                        &theme,
                    ))
                    .child(body),
            )
            .child(ds::banner(
                Tone::Neutral,
                "Sessions belong to the client that created them",
                Some(
                    "This list shows only sessions started from this window. Sessions opened by other apps, terminals or editors are not visible here, by design."
                        .into(),
                ),
                &theme,
            ))
            .into_any_element()
    }
}
