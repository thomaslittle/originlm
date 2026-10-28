use crate::app::Desktop;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_apps(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(bundle) = self.bundle.as_ref() else {
            return div()
                .px(px(ui::PAGE_PAD))
                .child(ui::empty_state(
                    IconName::SquareTerminal,
                    "Waiting for the broker",
                    "Connected applications are listed from the authenticated admin surface.",
                    &theme,
                ))
                .into_any_element();
        };
        let apps = bundle.apps.clone();
        let apps_error = bundle.apps_error.clone();

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
                            .child("This lists clients currently connected to the broker. Revocation is app-scoped: it deletes the broker grant and sweeps that app's live connections. The broker's app listing does not report execution scope."),
                    ),
            )
            .when_some(apps_error.clone(), |this, error| {
                this.child(
                    div()
                        .v_flex()
                        .gap_2()
                        .p_4()
                        .rounded(px(ui::CARD_RADIUS))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.colors.list)
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("App management needs an admin credential"),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "The broker refused apps.list: {error}. Launch the desktop app with an admin token file (control-admin-token.json) to inspect and revoke connected apps."
                                )),
                        ),
                )
            })
            .when(apps_error.is_none() && apps.is_empty(), |this| {
                this.child(ui::empty_state(
                    IconName::SquareTerminal,
                    "No apps connected",
                    "No consumer is currently attached to this broker.",
                    &theme,
                ))
            })
            .children(apps.into_iter().map(|app| {
                let id = app.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let name = app
                    .get("app")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_string();
                let passive = app
                    .get("allowPassiveDiscovery")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let admin = app.get("admin").and_then(|v| v.as_bool()).unwrap_or(false);
                let reveal = app
                    .get("revealIdentity")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let connections = app
                    .get("connectionIds")
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                    .unwrap_or(0);
                let confirming = self.revoke_confirm.as_deref() == Some(id);
                let entity = cx.entity();
                let entity_cancel = cx.entity();
                let entity_confirm = cx.entity();
                let target = id.to_string();
                let target_cancel = id.to_string();

                div()
                    .v_flex()
                    .gap_2()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(if confirming {
                        theme.danger.opacity(0.4)
                    } else {
                        theme.border
                    })
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(30.))
                                    .rounded(px(8.))
                                    .bg(theme.muted)
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .child(ui::icon(
                                        IconName::SquareTerminal,
                                        14.,
                                        theme.primary,
                                    )),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(1.))
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(name),
                                    )
                                    .child(ui::mono(
                                        format!("client {}", id),
                                        &theme,
                                    )),
                            )
                            .child(ui::chip(
                                if admin { "admin" } else { "app" },
                                if admin { Tone::Amber } else { Tone::Neutral },
                                &theme,
                            )),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .flex_wrap()
                            .child(ui::chip(
                                if passive {
                                    "passive discovery"
                                } else {
                                    "no discovery"
                                },
                                if passive { Tone::Green } else { Tone::Neutral },
                                &theme,
                            ))
                            .child(ui::chip(
                                if reveal {
                                    "identity visible"
                                } else {
                                    "identity hidden"
                                },
                                Tone::Neutral,
                                &theme,
                            ))
                            .child(ui::chip(
                                if connections == 0 {
                                    "all connections".to_string()
                                } else {
                                    format!("{connections} connections")
                                },
                                Tone::Neutral,
                                &theme,
                            )),
                    )
                    .when(confirming, |this| {
                        this.child(
                            div()
                                .h_flex()
                                .gap_2()
                                .p_2()
                                .rounded(px(7.))
                                .bg(theme.danger.opacity(0.08))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.danger)
                                        .child("Revoke this app's grant and end its live sessions now?"),
                                )
                                .child(div().flex_1())
                                .child(
                                    Button::new("revoke-cancel")
                                        .secondary()
                                        .label("Cancel")
                                        .on_click(move |_, _, cx| {
                                            entity_cancel.update(cx, |this, cx| {
                                                this.revoke_confirm = None;
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("revoke-confirm")
                                        .danger()
                                        .label("Revoke")
                                        .on_click({
                                            let target = target.clone();
                                            move |_, _, cx| {
                                                entity_confirm.update(cx, |this, cx| {
                                                    let _ = this
                                                        .broker
                                                        .commands
                                                        .send(crate::broker::Command::RevokeApp(
                                                            target.clone(),
                                                        ));
                                                    this.revoke_confirm = None;
                                                    this.toast("Revocation requested");
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        )
                    })
                    .when(!confirming && !admin, |this| {
                        this.child(
                            div()
                                .h_flex()
                                .justify_end()
                                .child(
                                    Button::new(SharedString::from(format!("revoke-{id}")))
                                        .secondary()
                                        .label("Revoke…")
                                        .on_click({
                                            let target = target_cancel.clone();
                                            move |_, _, cx| {
                                                entity.update(cx, |this, cx| {
                                                    this.revoke_confirm = Some(target.clone());
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        )
                    })
                    .into_any_element()
            }))
            .into_any_element()
    }
}
