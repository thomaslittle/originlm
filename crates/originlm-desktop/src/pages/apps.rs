//! Apps & grants: how applications get access to this runtime, which ones
//! are connected now, and app-scoped revocation.

use crate::app::Desktop;
use crate::ds::{self, sp, ty};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

const SHARED_BROKER_DOC: &str =
    "https://github.com/thomaslittle/originlm/blob/main/docs/integration/shared-broker.md";

/// The three real ways an app is granted access. None of them is a prompt in
/// this window: approval belongs to the host or to the person running the
/// broker.
const ACCESS: [(&str, &str, &str); 3] = [
    (
        "Grants file",
        "originlm serve --independent --grants-file PATH",
        "An owner-only JSON manifest, up to 32 apps. Each app gets its own token and its own scopes: discovery, connections, identity, and an optional execution grant.",
    ),
    (
        "Embedded host",
        "Broker::authorize(policy, execution_grant)",
        "A Rust host issues a separately revocable credential per app. No protocol request can widen its own scope.",
    ),
    (
        "SDK request",
        "grants.request(...)",
        "Returns requires_host_approval. The app asks; the host or broker owner decides.",
    ),
];

impl Desktop {
    fn access_panel(&self, theme: &Theme) -> Div {
        ds::panel(theme)
            .child(ds::panel_head(
                "How an app gets access",
                Some(
                    ds::link("apps-doc", "Shared broker guide", theme, |_, _, cx| {
                        cx.open_url(SHARED_BROKER_DOC)
                    })
                    .into_any_element(),
                ),
                theme,
            ))
            .children(ACCESS.iter().enumerate().map(|(index, (title, code, body))| {
                ds::panel_row(index == 0, theme)
                    .py(px(sp::MD))
                    .items_start()
                    .child(
                        div()
                            .w(px(140.))
                            .flex_shrink_0()
                            .text_size(px(ty::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(*title),
                    )
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .gap(px(sp::XS))
                            .child(ds::mono(*code, ty::SMALL, theme.foreground, theme))
                            .child(ds::caption(*body, theme)),
                    )
            }))
            .child(
                div()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(ds::caption(
                        "This window does not approve requests. It lists apps connected right now and revokes them, using the separate administrator credential.",
                        theme,
                    )),
            )
    }

    pub fn page_apps(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(bundle) = self.bundle.as_ref() else {
            return div().into_any_element();
        };
        let apps = bundle.apps.clone();
        let apps_error = bundle.apps_error.clone();

        let connected: AnyElement = match (&apps_error, apps.is_empty()) {
            (Some(_), _) if self.example => ds::state(
                ds::StateKind::Empty,
                "No connected apps in example mode",
                "Example mode starts no runtime, so no application can connect. With a runtime and an administrator credential, connected apps appear here with their scopes.",
                &theme,
            )
            .into_any_element(),
            (Some(error), _) => ds::state(
                ds::StateKind::Error,
                "Listing apps needs the administrator credential",
                format!(
                    "The runtime refused apps.list: {error}. Start the broker with --admin-token-file and launch this window with that file (--token-file) to list and revoke apps. Ordinary app tokens cannot."
                ),
                &theme,
            )
            .into_any_element(),
            (None, true) => ds::state(
                ds::StateKind::Empty,
                "No apps connected",
                "No application is attached to this runtime right now. Apps appear while they hold a connection.",
                &theme,
            )
            .into_any_element(),
            (None, false) => div()
                .v_flex()
                .child(
                    ds::table_head(&theme)
                        .child(ds::th("App", None, &theme))
                        .child(ds::th("Scopes", Some(320.), &theme))
                        .child(ds::th("", Some(120.), &theme)),
                )
                .children(
                    apps.iter()
                        .enumerate()
                        .map(|(index, app)| self.app_row(index, app, &theme, cx)),
                )
                .into_any_element(),
        };

        ds::page()
            .child(
                ds::panel(&theme)
                    .child(ds::panel_head(
                        "Connected now",
                        Some(
                            ds::caption(
                                if self.example {
                                    "Example mode".to_string()
                                } else if apps_error.is_some() {
                                    "Not readable with this credential".to_string()
                                } else {
                                    ui::plural(apps.len(), "app")
                                },
                                &theme,
                            )
                            .into_any_element(),
                        ),
                        &theme,
                    ))
                    .child(connected),
            )
            .child(self.access_panel(&theme))
            .into_any_element()
    }

    fn app_row(
        &self,
        index: usize,
        app: &serde_json::Value,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = app
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let name = app
            .get("app")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        let flag = |key: &str| app.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        let (passive, admin, reveal) = (
            flag("allowPassiveDiscovery"),
            flag("admin"),
            flag("revealIdentity"),
        );
        let connections = app
            .get("connectionIds")
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or(0);
        let confirming = self.revoke_confirm.as_deref() == Some(id.as_str());
        let entity = cx.entity();

        let actions: AnyElement = if admin {
            ds::caption("This window", theme).into_any_element()
        } else if confirming {
            div()
                .h_flex()
                .gap(px(sp::XS))
                .child(
                    Button::new("revoke-cancel")
                        .ghost()
                        .label("Keep")
                        .on_click({
                            let entity = entity.clone();
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.revoke_confirm = None;
                                    cx.notify();
                                })
                            }
                        }),
                )
                .child(
                    Button::new("revoke-confirm")
                        .danger()
                        .label("Revoke")
                        .on_click({
                            let entity = entity.clone();
                            let target = id.clone();
                            move |_, _, cx| {
                                let target = target.clone();
                                entity.update(cx, |this, cx| {
                                    let _ = this
                                        .broker
                                        .commands
                                        .send(crate::broker::Command::RevokeApp(target));
                                    this.revoke_confirm = None;
                                    this.toast("Revocation requested. The list updates when the runtime confirms.");
                                    cx.notify();
                                });
                            }
                        }),
                )
                .into_any_element()
        } else {
            Button::new(SharedString::from(format!("revoke-{id}")))
                .secondary()
                .label("Revoke…")
                .on_click({
                    let target = id.clone();
                    move |_, _, cx| {
                        let target = target.clone();
                        entity.update(cx, |this, cx| {
                            this.revoke_confirm = Some(target);
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        };

        div()
            .v_flex()
            .child(
                ds::panel_row(index == 0, theme)
                    .py(px(sp::SM))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .h_flex()
                                    .gap(px(sp::SM))
                                    .child(
                                        div()
                                            .text_size(px(ty::BODY))
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .child(name),
                                    )
                                    .when(admin, |this| {
                                        this.child(ds::chip("Administrator", Tone::Amber, theme))
                                    }),
                            )
                            .child(ds::copy_value(
                                format!("app-{index}"),
                                id.clone(),
                                40,
                                theme,
                            )),
                    )
                    .child(
                        div()
                            .w(px(320.))
                            .flex_shrink_0()
                            .h_flex()
                            .flex_wrap()
                            .gap(px(sp::XS))
                            .child(ds::chip(
                                if passive {
                                    "Passive discovery"
                                } else {
                                    "No discovery"
                                },
                                Tone::Neutral,
                                theme,
                            ))
                            .child(ds::chip(
                                if reveal {
                                    "Identity visible"
                                } else {
                                    "Identity hidden"
                                },
                                Tone::Neutral,
                                theme,
                            ))
                            .child(ds::chip(
                                if connections == 0 {
                                    "All connections".to_string()
                                } else {
                                    ui::plural(connections, "connection")
                                },
                                Tone::Neutral,
                                theme,
                            )),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .flex_shrink_0()
                            .h_flex()
                            .justify_end()
                            .child(actions),
                    ),
            )
            .when(confirming, |this| {
                this.child(div().px(px(sp::LG)).pb(px(sp::MD)).child(ds::banner(
                    Tone::Red,
                    "Revoke this app?",
                    Some(
                        "Deletes its grant and closes its live connections now. Its token cannot reconnect; the app must be granted again."
                            .into(),
                    ),
                    theme,
                )))
            })
            .into_any_element()
    }
}
