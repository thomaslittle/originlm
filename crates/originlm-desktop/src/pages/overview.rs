//! Overview: one card per account with every allowance window it reports,
//! then what needs attention and what a developer can build on.

use crate::app::{Desktop, Page};
use crate::data;
use crate::ds::{self, sp, ty};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, IconName, Theme};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use originlm_contracts::{BillingMode, Freshness, Observation, Snapshot};

/// What a single allowance window can honestly say.
enum Reading {
    /// A fraction of the window remains.
    Fraction(f32),
    /// The provider reports a count but no total to compare it with.
    Count(String),
    Unknown(String),
    Unavailable(String),
}

struct WindowView {
    label: String,
    reading: Reading,
    reset: Option<String>,
    stale: bool,
}

struct AccountView {
    connection_id: String,
    provider_id: String,
    provider: String,
    account: String,
    plan: String,
    auth: data::AuthView,
    windows: Vec<WindowView>,
}

impl AccountView {
    /// Lowest remaining fraction, for ordering the grid by urgency.
    fn tightest(&self) -> f32 {
        self.windows
            .iter()
            .filter_map(|window| match window.reading {
                Reading::Fraction(value) => Some(value),
                _ => None,
            })
            .fold(2.0, f32::min)
    }
}

fn accounts(snapshot: &Snapshot) -> Vec<AccountView> {
    let mut views: Vec<AccountView> = snapshot
        .connections
        .iter()
        .map(|connection| {
            let windows = snapshot
                .quota_pools
                .iter()
                .filter(|pool| pool.connection_ids.contains(&connection.id))
                .flat_map(|pool| pool.windows.iter())
                .map(|window| {
                    let (reading, stale) = match &window.reading {
                        Observation::Known {
                            value, freshness, ..
                        } => (
                            match data::remaining_fraction(value) {
                                Some(fraction) => Reading::Fraction(fraction),
                                None => Reading::Count(format!(
                                    "{} {} used",
                                    value.used.as_str(),
                                    value.unit.replace('_', " ")
                                )),
                            },
                            matches!(freshness, Freshness::Stale),
                        ),
                        Observation::Unknown { reason, .. } => {
                            (Reading::Unknown(reason.clone()), false)
                        }
                        Observation::Unavailable { reason, .. } => {
                            (Reading::Unavailable(reason.clone()), false)
                        }
                    };
                    WindowView {
                        label: crate::human::title_case(&window.label.replace('_', " ")),
                        reading,
                        reset: window.resets_at.as_ref().map(|t| t.as_str().to_string()),
                        stale,
                    }
                })
                .collect();
            AccountView {
                connection_id: connection.id.clone(),
                provider_id: connection.provider_id.clone(),
                provider: data::provider_name(snapshot, &connection.provider_id),
                account: connection.display_name.clone(),
                plan: data::plan_label(connection),
                auth: data::auth_view(connection),
                windows,
            }
        })
        .collect();
    // Accounts with readings first, tightest first; then the rest; sign-in last.
    views.sort_by(|a, b| {
        let rank = |view: &AccountView| {
            if view.auth.kind == data::AuthKind::Required {
                2
            } else if view.windows.is_empty() {
                1
            } else {
                0
            }
        };
        rank(a)
            .cmp(&rank(b))
            .then(
                a.tightest()
                    .partial_cmp(&b.tightest())
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.provider.cmp(&b.provider))
    });
    views
}

fn auth_chip(view: &data::AuthView, theme: &Theme) -> Div {
    let tone = match view.kind {
        data::AuthKind::Verified => Tone::Green,
        data::AuthKind::Cached => Tone::Amber,
        data::AuthKind::Required => Tone::Red,
        data::AuthKind::Unknown => Tone::Neutral,
    };
    let label = match view.kind {
        data::AuthKind::Verified => "Signed in",
        data::AuthKind::Cached => "Cached sign-in",
        data::AuthKind::Required => "Sign-in required",
        data::AuthKind::Unknown => "Not checked",
    };
    ds::chip(label, tone, theme)
}

fn window_row(window: &WindowView, theme: &Theme) -> Div {
    let (meter_value, figure, figure_color) = match &window.reading {
        Reading::Fraction(value) => (
            Some(*value),
            format!("{:.0}% left", value * 100.0),
            ui::tone_color(ds::remaining_tone(*value), theme),
        ),
        Reading::Count(text) => (None, text.clone(), theme.foreground),
        Reading::Unknown(_) => (None, "Unknown".to_string(), theme.foreground),
        Reading::Unavailable(_) => (None, "Unavailable".to_string(), theme.foreground),
    };
    let figure_color = if matches!(window.reading, Reading::Fraction(v) if v >= 0.35) {
        theme.foreground
    } else {
        figure_color
    };
    let detail = match &window.reading {
        Reading::Count(_) => "No published total".to_string(),
        Reading::Unknown(reason) | Reading::Unavailable(reason) => reason.clone(),
        Reading::Fraction(_) => match ui::format_countdown(window.reset.as_deref()) {
            Some(countdown) if countdown == "reset due" => "Reset due".to_string(),
            Some(countdown) => format!("Resets in {countdown}"),
            None => "Reset time not reported".to_string(),
        },
    };
    let reset_clock = window
        .reset
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|stamp| {
            stamp
                .with_timezone(&chrono::Local)
                .format("%a %H:%M")
                .to_string()
        });

    div()
        .v_flex()
        .gap(px(6.))
        .px(px(sp::LG))
        .py(px(sp::MD))
        .border_t_1()
        .border_color(theme.border.opacity(0.7))
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(sp::SM))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(ty::SMALL))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(window.label.clone()),
                )
                .when(window.stale, |this| {
                    this.child(ds::chip("Stale", Tone::Amber, theme))
                })
                .child(ds::mono(figure, ty::BODY, figure_color, theme)),
        )
        .child(ds::meter(meter_value, theme))
        .child(
            div()
                .h_flex()
                .gap(px(sp::SM))
                .child(ds::caption(detail, theme).flex_1().min_w(px(0.)).truncate())
                .when_some(
                    reset_clock.filter(|_| matches!(window.reading, Reading::Fraction(_))),
                    |this, clock| {
                        this.child(ds::mono(clock, ty::CAPTION, theme.muted_foreground, theme))
                    },
                ),
        )
}

impl Desktop {
    fn account_card(
        &self,
        view: &AccountView,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let header = div()
            .h_flex()
            .items_center()
            .gap(px(sp::MD))
            .px(px(sp::LG))
            .py(px(sp::MD))
            .child(crate::icons::provider_tile(
                &view.provider_id,
                &view.provider,
                32.,
                theme,
            ))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .text_size(px(ty::BODY))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .truncate()
                            .child(view.provider.clone()),
                    )
                    .child(
                        ds::caption(format!("{} · {}", view.account, view.plan), theme).truncate(),
                    ),
            )
            .child(auth_chip(&view.auth, theme));

        let body: AnyElement = if !view.windows.is_empty() {
            div()
                .v_flex()
                .children(view.windows.iter().map(|window| window_row(window, theme)))
                .into_any_element()
        } else {
            let id = view.connection_id.clone();
            let required = view.auth.kind == data::AuthKind::Required;
            let tokens = self
                .bundle
                .as_ref()
                .and_then(|bundle| bundle.costs.as_ref())
                .and_then(|costs| {
                    costs
                        .historical
                        .providers
                        .iter()
                        .find(|provider| provider.provider_id == view.provider_id)
                });
            div()
                .v_flex()
                .gap(px(sp::XS))
                .px(px(sp::LG))
                .py(px(sp::MD))
                .border_t_1()
                .border_color(theme.border.opacity(0.7))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(sp::SM))
                        .child(ds::meter(None, theme))
                        .child(ds::chip("No quota window", Tone::Neutral, theme))
                        .child(div().flex_1())
                        .when(tokens.is_some(), |this| {
                            let tokens = tokens.unwrap();
                            this.child(ds::mono(
                                format!("{} in · {} out", tokens.totals.total_input_tokens(), tokens.totals.output_tokens),
                                ty::CAPTION,
                                theme.muted_foreground,
                                theme,
                            ))
                        }),
                )
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(sp::SM))
                        .child(
                            ds::caption(
                                match view.auth.kind {
                                    data::AuthKind::Required => {
                                        "Sign in to the client to read its allowance."
                                    }
                                    data::AuthKind::Unknown => {
                                        "No allowance has been read. This client may not publish one."
                                    }
                                    _ if tokens.is_some() => {
                                        "This client publishes local transcript totals, not a quota window."
                                    }
                                    _ => "Signed in. This client does not publish an allowance window.",
                                },
                                theme,
                            )
                            .flex_1(),
                        )
                        .when(!required, |this| {
                            this.child(ds::link(
                                SharedString::from(format!("probe-{id}")),
                                "Check again",
                                theme,
                                move |_, _, cx| {
                                    let id = id.clone();
                                    entity.update(cx, |this, cx| this.request_probe(id, cx));
                                },
                            ))
                        }),
                )
                .into_any_element()
        };

        ds::panel(theme)
            .child(header)
            .child(body)
            .into_any_element()
    }

    pub fn page_overview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let counts = self.counts();
        let views = self.snapshot().map(accounts).unwrap_or_default();
        let with_readings = views.iter().filter(|view| !view.windows.is_empty()).count();

        // Route billing mix, for the developer panel.
        let (mut subscription, mut metered, mut local, mut other) = (0, 0, 0, 0);
        if let Some(snapshot) = self.snapshot() {
            for route in &snapshot.routes {
                match &route.billing {
                    Observation::Known { value, .. } => match value {
                        BillingMode::Subscription => subscription += 1,
                        BillingMode::ApiMetered | BillingMode::PrepaidCredits => metered += 1,
                        BillingMode::Local => local += 1,
                        _ => other += 1,
                    },
                    _ => other += 1,
                }
            }
        }
        let routes_total = subscription + metered + local + other;
        let issues = self
            .snapshot()
            .map(|snapshot| data::open_issues(snapshot).len())
            .unwrap_or(0);

        // Runtime strip.
        let (status_tone, status_text) = if self.example {
            (Tone::Amber, "Example data")
        } else if self.connected {
            (Tone::Green, "Runtime connected")
        } else {
            (Tone::Red, "Runtime not answering")
        };
        let strip = ds::panel(&theme).child(
            div()
                .h_flex()
                .items_center()
                .flex_wrap()
                .gap(px(sp::XL))
                .px(px(sp::LG))
                .py(px(sp::MD))
                .child(ds::status(status_text, status_tone, &theme))
                .child(ds::stat(
                    counts.connections.to_string(),
                    format!("accounts, {} providers", counts.providers),
                    &theme,
                ))
                .child(ds::stat(
                    format!("{}/{}", counts.authenticated, counts.connections),
                    if counts.cached > 0 {
                        format!("signed in, {} cached", counts.cached)
                    } else {
                        "signed in".to_string()
                    },
                    &theme,
                ))
                .child(ds::stat(
                    with_readings.to_string(),
                    "reporting usage",
                    &theme,
                ))
                .child(ds::stat(counts.models.to_string(), "models", &theme))
                .child(ds::stat(routes_total.to_string(), "routes", &theme))
                .child(ds::stat(
                    if self.connected || self.example {
                        issues.to_string()
                    } else {
                        "?".into()
                    },
                    "open issues",
                    &theme,
                )),
        );

        let sidebar = if self.sidebar_collapsed {
            48.
        } else {
            self.sidebar_width
        };
        let content = self.viewport_w - sidebar - ds::GUTTER * 2.;
        let columns: u16 = if content >= 1040. {
            3
        } else if content >= 640. {
            2
        } else {
            1
        };
        let grid = div()
            .grid()
            .grid_cols(columns)
            .gap(px(sp::MD))
            .children(views.iter().map(|view| self.account_card(view, &theme, cx)));

        // Needs attention.
        let attention: Vec<(Tone, String)> = self
            .snapshot()
            .map(|snapshot| {
                let mut items: Vec<(Tone, String)> = data::open_issues(snapshot)
                    .iter()
                    .take(4)
                    .map(|issue| (Tone::Amber, issue.title.clone()))
                    .collect();
                for connection in &snapshot.connections {
                    let view = data::auth_view(connection);
                    let name = data::connection_title(snapshot, connection);
                    match view.kind {
                        data::AuthKind::Required => {
                            items.push((Tone::Red, format!("{name}: sign-in required")))
                        }
                        data::AuthKind::Cached => items.push((
                            Tone::Amber,
                            format!("{name}: sign-in is cached, not verified"),
                        )),
                        _ => {}
                    }
                }
                items
            })
            .unwrap_or_default();
        let entity = cx.entity();
        let attention_panel = ds::panel(&theme)
            .flex_1()
            .child(ds::panel_head(
                "Needs attention",
                Some(
                    ds::link("all-diagnostics", "Diagnostics", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| {
                            entity.update(cx, |this, cx| this.set_page(Page::Diagnostics, cx));
                        }
                    })
                    .into_any_element(),
                ),
                &theme,
            ))
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .when(attention.is_empty(), |this| {
                        this.child(
                            div()
                                .h_flex()
                                .gap(px(sp::SM))
                                .py(px(sp::SM))
                                .child(ui::icon(IconName::CircleCheck, 14., theme.success))
                                .child(ds::body("Nothing reported.", &theme)),
                        )
                    })
                    .children(attention.into_iter().map(|(tone, text)| {
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::SM))
                            .min_h(px(30.))
                            .child(ui::icon(
                                IconName::CircleAlert,
                                13.,
                                ui::tone_color(tone, &theme),
                            ))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .text_color(theme.foreground)
                                    .child(text),
                            )
                    })),
            );

        let apps_value = match self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.apps_error.as_ref())
        {
            Some(_) if self.example => "None in example mode".to_string(),
            Some(_) => "Needs an administrator credential".to_string(),
            None => ui::plural(counts.apps, "app"),
        };
        let build_panel = ds::panel(&theme)
            .flex_1()
            .child(ds::panel_head(
                "Build on this runtime",
                Some(
                    ds::link("open-developer", "Developer page", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| {
                            entity.update(cx, |this, cx| this.set_page(Page::Developer, cx));
                        }
                    })
                    .into_any_element(),
                ),
                &theme,
            ))
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::kv(
                        "Routes",
                        ds::mono(
                            format!(
                                "{subscription} subscription · {metered} metered · {local} local{}",
                                if other > 0 {
                                    format!(" · {other} unknown")
                                } else {
                                    String::new()
                                }
                            ),
                            ty::SMALL,
                            theme.foreground,
                            &theme,
                        ),
                        &theme,
                    ))
                    .child(ds::kv(
                        "Connected apps",
                        div().text_size(px(ty::SMALL)).child(apps_value),
                        &theme,
                    ))
                    .child(ds::kv(
                        "Sessions here",
                        div()
                            .text_size(px(ty::SMALL))
                            .child(ui::plural(self.sessions.len(), "session")),
                        &theme,
                    ))
                    .child(ds::kv(
                        "Paid routes",
                        div()
                            .text_size(px(ty::SMALL))
                            .child("Off until a grant allows them"),
                        &theme,
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(sp::XS))
                    .px(px(sp::MD))
                    .pb(px(sp::MD))
                    .child(ds::link("ov-playground", "Open Playground", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| {
                            entity.update(cx, |this, cx| this.set_page(Page::Playground, cx));
                        }
                    }))
                    .child(ds::link("ov-apps", "Apps & grants", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| {
                            entity.update(cx, |this, cx| this.set_page(Page::Apps, cx));
                        }
                    })),
            );

        div()
            .v_flex()
            .w_full()
            .px(px(ds::GUTTER))
            .pb(px(sp::XXL))
            .gap(px(sp::XL))
            .child(strip)
            .child(
                div()
                    .v_flex()
                    .gap(px(sp::MD))
                    .child(ds::section_header(
                        "Allowances",
                        Some(
                            "Percent left in each window the client reports. Missing readings stay unknown."
                                .into(),
                        ),
                        &theme,
                    ))
                    .child(if views.is_empty() {
                        ds::panel(&theme)
                            .child(ds::state(
                                ds::StateKind::Empty,
                                "No accounts discovered yet",
                                "Discovery found no supported AI client on this machine, so there is no allowance to show. Install or sign in to a client, then refresh. Accounts lists what is supported.",
                                &theme,
                            ))
                            .into_any_element()
                    } else {
                        grid.into_any_element()
                    }),
            )
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(sp::MD))
                    .child(attention_panel)
                    .child(build_panel),
            )
            .child(self.cost_trend(cx))
            .into_any_element()
    }
}
