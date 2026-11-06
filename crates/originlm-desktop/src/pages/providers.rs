//! Accounts: every discovered connection in one compact table, with an
//! inspector for the selected one (identity, evidence, routes, actions).

use crate::app::{Desktop, Page};
use crate::data;
use crate::ds::{self, sp, ty};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use originlm_contracts::{Snapshot, Support};

pub fn auth_tone(kind: data::AuthKind) -> Tone {
    match kind {
        data::AuthKind::Verified => Tone::Green,
        data::AuthKind::Cached => Tone::Amber,
        data::AuthKind::Required => Tone::Red,
        data::AuthKind::Unknown => Tone::Neutral,
    }
}

/// Short sign-in label for tables and chips.
pub fn auth_short(kind: data::AuthKind) -> &'static str {
    match kind {
        data::AuthKind::Verified => "Signed in",
        data::AuthKind::Cached => "Cached sign-in",
        data::AuthKind::Required => "Sign-in required",
        data::AuthKind::Unknown => "Not checked",
    }
}

fn model_count(snapshot: &Snapshot, connection_id: &str) -> usize {
    snapshot
        .routes
        .iter()
        .filter(|route| route.connection_id == connection_id)
        .map(|route| {
            snapshot
                .models
                .iter()
                .filter(|model| model.route_id == route.id)
                .count()
        })
        .sum()
}

/// Supported capabilities across the models of one route, deduplicated.
pub fn route_capabilities(snapshot: &Snapshot, route_id: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for model in snapshot.models.iter().filter(|m| m.route_id == route_id) {
        for cap in &model.capabilities {
            if matches!(cap.support, Support::Supported) && !out.contains(&cap.id) {
                out.push(cap.id.clone());
            }
        }
    }
    out
}

pub fn capability_name(id: &str) -> String {
    let label = data::capability_label(id);
    if label.is_empty() {
        id.to_string()
    } else {
        label.to_string()
    }
}

impl Desktop {
    pub fn page_providers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.snapshot().cloned() else {
            return div().into_any_element();
        };
        let entity = cx.entity();

        if snapshot.connections.is_empty() {
            return ds::page()
                .child(
                    ds::panel(&theme).child(
                        ds::state(
                            ds::StateKind::Empty,
                            "No AI clients found yet",
                            "Discovery looked for supported clients and local model servers and found none. Install or sign in to a supported client, then refresh. Nothing is created or signed in for you.",
                            &theme,
                        )
                        .child(
                            div()
                                .h_flex()
                                .gap(px(sp::SM))
                                .pt(px(sp::SM))
                                .child(
                                    Button::new("accounts-refresh")
                                        .secondary()
                                        .label("Refresh")
                                        .icon(IconName::RefreshCw)
                                        .on_click({
                                            let entity = entity.clone();
                                            move |_, _, cx| {
                                                entity.update(cx, |this, cx| this.refresh(cx))
                                            }
                                        }),
                                )
                                .child(ds::link("accounts-matrix", "Supported clients", &theme, |_, _, cx| {
                                    cx.open_url("https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md");
                                })),
                        ),
                    ),
                )
                .into_any_element();
        }

        let selected = self
            .selected_connection
            .clone()
            .filter(|id| snapshot.connections.iter().any(|c| &c.id == id))
            .or_else(|| snapshot.connections.first().map(|c| c.id.clone()));
        let signed_in = snapshot
            .connections
            .iter()
            .filter(|c| data::auth_view(c).kind == data::AuthKind::Verified)
            .count();

        let rows = snapshot.connections.iter().map(|connection| {
            let view = data::auth_view(connection);
            let provider = data::provider_name(&snapshot, &connection.provider_id);
            let title = crate::human::without_provider_prefix(&connection.display_name, &provider);
            let is_selected = selected.as_deref() == Some(connection.id.as_str());
            let id = connection.id.clone();
            let entity = entity.clone();
            ds::select_row(
                SharedString::from(format!("connection-{}", connection.id)),
                is_selected,
                &theme,
            )
            .aria_label(format!("{title}, {provider}, {}", auth_short(view.kind)))
            .child(crate::icons::provider_tile(
                &connection.provider_id,
                &provider,
                28.,
                &theme,
            ))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .text_size(px(ty::BODY))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .truncate()
                            .child(title),
                    )
                    .child(ds::caption(provider, &theme).truncate()),
            )
            .child(
                div()
                    .w(px(170.))
                    .flex_shrink_0()
                    .truncate()
                    .text_size(px(ty::SMALL))
                    .text_color(theme.muted_foreground)
                    .child(data::plan_label(connection)),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(sp::XS))
                    .w(px(170.))
                    .flex_shrink_0()
                    .child(ds::chip(
                        auth_short(view.kind),
                        auth_tone(view.kind),
                        &theme,
                    ))
                    .when(view.stale, |this| {
                        this.child(ds::chip("Stale", Tone::Amber, &theme))
                    }),
            )
            .child(div().w(px(56.)).flex_shrink_0().child(ds::mono(
                model_count(&snapshot, &connection.id).to_string(),
                ty::SMALL,
                theme.foreground,
                &theme,
            )))
            .on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| {
                    this.selected_connection = Some(id.clone());
                    cx.notify();
                });
            })
        });

        let list = ds::panel(&theme)
            .flex_1()
            .child(ds::panel_head(
                "Connections",
                Some(
                    ds::caption(
                        format!(
                            "{} · {signed_in} signed in",
                            ui::plural(snapshot.connections.len(), "account")
                        ),
                        &theme,
                    )
                    .into_any_element(),
                ),
                &theme,
            ))
            .child(
                ds::table_head(&theme)
                    // Match select_row's 12px inset so headers sit over rows.
                    .px(px(sp::MD))
                    .child(div().w(px(28.)).flex_shrink_0())
                    .child(ds::th("Account", None, &theme))
                    .child(ds::th("Plan", Some(170.), &theme))
                    .child(ds::th("Sign-in", Some(170.), &theme))
                    .child(ds::th("Models", Some(56.), &theme)),
            )
            .child(div().v_flex().py(px(sp::XS)).children(rows));

        let inspector = selected
            .as_ref()
            .and_then(|id| snapshot.connections.iter().find(|c| &c.id == id))
            .map(|connection| self.connection_inspector(&snapshot, connection, &theme, cx))
            .unwrap_or_else(|| div().into_any_element());

        let wide = self.content_width() >= 1000.;
        ds::page()
            .child(if wide {
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(sp::MD))
                    .child(list)
                    .child(
                        div()
                            .w(px(self.providers_inspector.clamp(360., 460.)))
                            .flex_shrink_0()
                            .child(inspector),
                    )
                    .into_any_element()
            } else {
                div()
                    .v_flex()
                    .gap(px(sp::MD))
                    .child(list)
                    .child(inspector)
                    .into_any_element()
            })
            .into_any_element()
    }

    fn connection_inspector(
        &self,
        snapshot: &Snapshot,
        connection: &originlm_contracts::Connection,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = data::provider_name(snapshot, &connection.provider_id);
        let view = data::auth_view(connection);
        let title = crate::human::without_provider_prefix(&connection.display_name, &provider);
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
        let has_models = model_count(snapshot, &connection.id) > 0;
        let entity = cx.entity();
        let probe_id = connection.id.clone();
        let example = self.example;

        let value = |text: String| {
            div()
                .text_size(px(ty::SMALL))
                .text_color(theme.foreground)
                .truncate()
                .child(text)
        };

        ds::panel(theme)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(crate::icons::provider_tile(
                        &connection.provider_id,
                        &provider,
                        40.,
                        theme,
                    ))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(ty::TITLE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(title),
                            )
                            .child(
                                ds::caption(
                                    format!("{provider} · {}", data::plan_label(connection)),
                                    theme,
                                )
                                .truncate(),
                            ),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(sp::XS))
                    .px(px(sp::LG))
                    .pt(px(sp::MD))
                    .child(ds::chip(auth_short(view.kind), auth_tone(view.kind), theme))
                    .when(view.stale, |this| {
                        this.child(ds::chip("Stale reading", Tone::Amber, theme))
                    })
                    .child(ds::chip(ui::plural(routes.len(), "route"), Tone::Neutral, theme)),
            )
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::kv("Sign-in", value(view.label.to_string()), theme))
                    .child(ds::kv(
                        "Connection ID",
                        ds::copy_value("conn-id", connection.id.clone(), 34, theme),
                        theme,
                    ))
                    .when_some(installation, |this, installation| {
                        this.child(ds::kv(
                            "Installation",
                            value(installation.location_hint.clone()),
                            theme,
                        ))
                    })
                    .when_some(profile, |this, profile| {
                        this.child(ds::kv("Profile", value(profile.label.clone()), theme))
                    })
                    .child(ds::kv(
                        "Account",
                        value(match account.map(|account| &account.display_label) {
                            Some(originlm_contracts::Observation::Known { value, .. }) => {
                                value.clone()
                            }
                            Some(_) => "Hidden unless an app is granted identity".into(),
                            None => "Not reported".into(),
                        }),
                        theme,
                    )),
            )
            .when(!connection.diagnostics.is_empty(), |this| {
                this.child(
                    div().px(px(sp::LG)).pb(px(sp::MD)).child(ds::banner(
                        Tone::Amber,
                        "Reported by discovery",
                        Some(
                            connection
                                .diagnostics
                                .iter()
                                .map(|code| crate::human::describe_diagnostic(code))
                                .collect::<Vec<_>>()
                                .join(" ")
                                .into(),
                        ),
                        theme,
                    )),
                )
            })
            .child(
                div()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(px(ty::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Routes"),
                    ),
            )
            .children(routes.iter().enumerate().map(|(index, route)| {
                let caps = route_capabilities(snapshot, &route.id);
                ds::panel_row(index == 0, theme)
                    .py(px(sp::SM))
                    .items_start()
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(crate::human::without_provider_prefix(
                                        &route.display_name,
                                        &provider,
                                    )),
                            )
                            .child(
                                ds::caption(
                                    if caps.is_empty() {
                                        "No supported capability reported".to_string()
                                    } else {
                                        caps.iter()
                                            .map(|cap| capability_name(cap))
                                            .collect::<Vec<_>>()
                                            .join(" · ")
                                    },
                                    theme,
                                ),
                            ),
                    )
                    .child(ds::chip(ui::billing_label(route), Tone::Neutral, theme))
            }))
            .when(routes.is_empty() || !has_models, |this| {
                this.child(
                    div().px(px(sp::LG)).py(px(sp::SM)).child(ds::caption(
                        if connection.provider_id == "cursor" {
                            "Cursor's Agent CLI did not report a model catalog; only the local account cache is shown."
                        } else {
                            "This connection reported no model catalog."
                        },
                        theme,
                    )),
                )
            })
            .child(
                div()
                    .h_flex()
                    .gap(px(sp::SM))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        Button::new("probe-connection")
                            .secondary()
                            .label("Refresh metadata")
                            .icon(IconName::RefreshCw)
                            .tooltip("Read this account's metadata and usage again. Never runs a model.")
                            .on_click({
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.request_probe(probe_id.clone(), cx);
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("open-models")
                            .ghost()
                            .icon(IconName::Cpu)
                            .label("Models")
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.set_page(Page::Models, cx));
                            }),
                    )
                    .when(example, |this| {
                        this.child(ds::caption("Example mode probes nothing.", theme))
                    }),
            )
            .into_any_element()
    }
}
