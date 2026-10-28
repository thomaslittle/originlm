use crate::app::Desktop;
use crate::data;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_providers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.snapshot().cloned() else {
            return div()
                .px(px(ui::PAGE_PAD))
                .child(ui::empty_state(
                    IconName::Bot,
                    "Waiting for the broker",
                    "Providers appear once the shared broker publishes its first snapshot.",
                    &theme,
                ))
                .into_any_element();
        };

        let selected = self.selected_connection.clone().or_else(|| {
            snapshot
                .connections
                .first()
                .map(|connection| connection.id.clone())
        });

        let rows: Vec<AnyElement> = snapshot
            .connections
            .iter()
            .map(|connection| {
                let is_selected = selected.as_deref() == Some(connection.id.as_str());
                let view = data::auth_view(connection);
                let auth = view.label;
                let auth_tone = match view.kind {
                    data::AuthKind::Verified => Tone::Green,
                    data::AuthKind::Cached => Tone::Amber,
                    data::AuthKind::Required => Tone::Red,
                    data::AuthKind::Unknown => Tone::Neutral,
                };
                let stale = view.stale;
                let plan = data::plan_label(connection);
                let provider = data::provider_name(&snapshot, &connection.provider_id);
                let title =
                    crate::human::without_provider_prefix(&connection.display_name, &provider);
                let entity = cx.entity();
                let id = connection.id.clone();
                let models = snapshot
                    .routes
                    .iter()
                    .filter(|route| route.connection_id == connection.id)
                    .flat_map(|route| {
                        snapshot
                            .models
                            .iter()
                            .filter(move |model| model.route_id == route.id)
                    })
                    .count();
                div()
                    .id(SharedString::from(format!("connection-{id}")))
                    .v_flex()
                    .gap_1()
                    .p_3()
                    .rounded(px(9.))
                    .border_1()
                    .border_color(if is_selected {
                        theme.border
                    } else {
                        gpui::transparent_black()
                    })
                    .bg(if is_selected {
                        theme.list_active
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.list_hover))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(crate::icons::provider_tile(
                                &connection.provider_id,
                                &provider,
                                48.,
                                &theme,
                            ))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(1.))
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(title.clone()),
                                    )
                                    .child(ui::mono(provider.clone(), &theme)),
                            )
                            .child(ui::dot(auth_tone, &theme)),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .flex_wrap()
                            .child(ui::chip(plan, Tone::Neutral, &theme))
                            .child(ui::chip(
                                if stale {
                                    format!("{auth}, stale")
                                } else {
                                    auth.to_string()
                                },
                                auth_tone,
                                &theme,
                            ))
                            .child(ui::chip(ui::plural(models, "model"), Tone::Neutral, &theme)),
                    )
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.selected_connection = Some(id.clone());
                            cx.notify();
                        });
                    })
                    .into_any_element()
            })
            .collect();

        let detail: AnyElement = selected
            .as_ref()
            .and_then(|id| snapshot.connections.iter().find(|c| &c.id == id))
            .map(|connection| {
                let provider = data::provider_name(&snapshot, &connection.provider_id);
                let view = data::auth_view(connection);
                let auth = view.label;
                let authenticated = view.kind == data::AuthKind::Verified;
                let stale = view.stale;
                let plan = data::plan_label(connection);
                let title = crate::human::without_provider_prefix(
                    &connection.display_name,
                    &provider,
                );
                let installation = snapshot
                    .installations
                    .iter()
                    .find(|installation| installation.id == connection.installation_id);
                let profile = snapshot
                    .profiles
                    .iter()
                    .find(|profile| profile.id == connection.profile_id);
                let account = connection
                    .account_id
                    .as_ref()
                    .and_then(|id| snapshot.accounts.iter().find(|account| &account.id == id));
                let routes: Vec<_> = snapshot
                    .routes
                    .iter()
                    .filter(|route| route.connection_id == connection.id)
                    .collect();
                let has_models = routes.iter().any(|route| {
                    snapshot
                        .models
                        .iter()
                        .any(|model| model.route_id == route.id)
                });
                let entity_probe = cx.entity();
                let probe_id = connection.id.clone();
                let entity_models = cx.entity();

                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .child(
                                crate::icons::provider_tile(
                                    &connection.provider_id,
                                    &provider,
                                    72.,
                                    &theme,
                                ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(theme.muted_foreground)
                                            .child(format!("{provider}, {plan}")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(ui::chip(
                                if stale {
                                    format!("{auth}, stale")
                                } else {
                                    auth.to_string()
                                },
                                if authenticated {
                                    Tone::Green
                                } else {
                                    Tone::Red
                                },
                                &theme,
                            ))
                            .child(ui::chip(
                                ui::plural(routes.len(), "route"),
                                Tone::Neutral,
                                &theme,
                            )),
                    )
                    .child(ui::divider(&theme))
                    .child(kv("Connection", "connection", &connection.id, &theme))
                    .when_some(installation, |this, installation| {
                        this.child(kv(
                            "Installation",
                            "installation",
                            &installation.location_hint.clone(),
                            &theme,
                        ))
                    })
                    .when_some(profile, |this, profile| {
                        this.child(kv("Profile", "profile", &profile.label, &theme))
                    })
                    .when_some(account, |this, account| {
                        let label = match &account.display_label {
                            originlm_contracts::Observation::Known { value, .. } => value.clone(),
                            _ => "identity hidden".to_string(),
                        };
                        this.child(kv("Account", "account", &label, &theme))
                    })
                    .when(!connection.diagnostics.is_empty(), |this| {
                        this.child(
                            div()
                                .v_flex()
                                .gap_1()
                                .p_3()
                                .rounded(px(8.))
                                .bg(theme.warning.opacity(0.07))
                                .border_1()
                                .border_color(theme.warning.opacity(0.25))
                                .children(connection.diagnostics.iter().map(|code| {
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(theme.foreground)
                                        .child(crate::human::describe_diagnostic(code))
                                })),
                        )
                    })
                    .child(ui::section_title("Routes", &theme))
                    .children(routes.into_iter().map(|route| {
                        let (billing, fresh) = data::billing(route);
                        div()
                            .h_flex()
                            .gap_2()
                            .p_2()
                            .rounded(px(7.))
                            .bg(theme.muted)
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .child(crate::human::without_provider_prefix(
                                        &route.display_name,
                                        &provider,
                                    )),
                            )
                            .child(div().flex_1())
                            .child(ui::chip(
                                if fresh {
                                    billing.clone()
                                } else {
                                    format!("{billing}, stale")
                                },
                                Tone::Neutral,
                                &theme,
                            ))
                            .into_any_element()
                    }))
                    .when(!has_models, |this| {
                        this.child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.muted_foreground)
                                .child(if connection.provider_id == "cursor" {
                                    "Cursor's Agent CLI did not report a model catalog, so OriginLM is showing the local account cache only."
                                } else {
                                    "No local model catalog is available for this connection."
                                }),
                        )
                    })
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                Button::new("probe-connection")
                                    .secondary()
                                    .label("Refresh metadata")
                                    .icon(IconName::RefreshCw)
                                    .on_click(move |_, _, cx| {
                                        entity_probe.update(cx, |this, cx| {
                                            let _ = this.broker.commands.send(
                                                crate::broker::Command::Probe(probe_id.clone()),
                                            );
                                            this.toast("Metadata probe requested");
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new("open-models")
                                    .secondary()
                                    .label("Models")
                                    .on_click(move |_, _, cx| {
                                        entity_models.update(cx, |this, cx| {
                                            this.set_page(crate::app::Page::Models, cx)
                                        });
                                    }),
                            ),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                ui::empty_state(
                    IconName::Bot,
                    "Select a connection",
                    "Choose a discovered connection to inspect its evidence and routes.",
                    &theme,
                )
                .into_any_element()
            });

        div()
            .h_flex()
            .w_full()
            .items_start()
            .gap_4()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .child(
                div()
                    .flex_1()
                    .v_flex()
                    .gap_2()
                    .p_3()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("Connections", &theme))
                    .when(rows.is_empty(), |this| {
                        this.child(ui::empty_state(
                            IconName::Bot,
                            "Nothing discovered yet",
                            "Refresh passively to scan installed clients and local servers.",
                            &theme,
                        ))
                    })
                    .children(rows),
            )
            .child(
                div()
                    .w(px(430.))
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(detail),
            )
            .into_any_element()
    }
}

fn kv(
    prefix: &str,
    id: &'static str,
    value: &str,
    theme: &gpui_kit::component::Theme,
) -> AnyElement {
    let copy = value.to_string();
    div()
        .v_flex()
        .gap(px(2.))
        .child(ui::eyebrow(prefix, theme))
        .child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    div()
                        .id(SharedString::from(format!("copy-{id}")))
                        .font_family(theme.mono_font_family.clone())
                        .text_size(px(10.5))
                        .text_color(theme.foreground)
                        .cursor_pointer()
                        .child(ui::short(value, 30))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()));
                        }),
                )
                .child(ui::icon(IconName::Copy, 10., theme.muted_foreground)),
        )
        .into_any_element()
}
