//! Overview: one card per account with every allowance window it reports,
//! then what needs attention and what a developer can build on.

use crate::app::{Desktop, Page};
use crate::data;
use crate::ds::{self, Focusable as _, sp, ty};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, IconName, Theme};
use gpui_kit::prelude::FluentBuilder as _;
#[cfg(test)]
use gpui_kit::test::TestSupportExt as _;
use gpui_kit::*;
use originlm_contracts::{BillingMode, Freshness, Observation, Snapshot};

/// What a single allowance window can honestly say.
#[derive(Clone)]
enum Reading {
    /// A fraction of the window remains.
    Fraction(f32),
    /// The provider reports a count but no total to compare it with.
    Count(String),
    Unknown,
    Unavailable,
}

#[derive(Clone)]
struct WindowView {
    label: String,
    reading: Reading,
    reset: Option<String>,
    stale: bool,
}

#[derive(Clone)]
struct AccountView {
    connection_id: String,
    provider_id: String,
    provider: String,
    account: String,
    status: String,
    diagnostics: Vec<String>,
    auth: data::AuthView,
    windows: Vec<WindowView>,
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
                        Observation::Unknown { .. } => (Reading::Unknown, false),
                        Observation::Unavailable { .. } => (Reading::Unavailable, false),
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
                status: connection.status.clone(),
                diagnostics: connection.diagnostics.clone(),
                auth: data::auth_view(connection),
                windows,
            }
        })
        .collect();
    // Accounts with readings first, tightest first; then the rest; sign-in last.
    views.sort_by(|a, b| {
        a.provider
            .to_lowercase()
            .cmp(&b.provider.to_lowercase())
            .then(a.account.to_lowercase().cmp(&b.account.to_lowercase()))
    });
    views
}

fn auth_chip(view: &data::AuthView, theme: &Theme) -> Div {
    let tone = match view.kind {
        data::AuthKind::DiscoveryOnly => Tone::Neutral,
        data::AuthKind::Unavailable => Tone::Red,
        data::AuthKind::Verified => Tone::Green,
        data::AuthKind::Cached => Tone::Amber,
        data::AuthKind::Required => Tone::Red,
        data::AuthKind::Unknown => Tone::Neutral,
    };
    let label = match view.kind {
        data::AuthKind::DiscoveryOnly => "Discovery only",
        data::AuthKind::Unavailable => "Reading unavailable",
        data::AuthKind::Verified => "Signed in",
        data::AuthKind::Cached => "Cached sign-in",
        data::AuthKind::Required => "Sign-in required",
        data::AuthKind::Unknown => "Not checked",
    };
    ds::chip(label, tone, theme)
}

fn compact_tokens(value: u64) -> String {
    let value = value as f64;
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 10_000.0 {
        format!("{:.0}k", value / 1_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

/// One allowance window: label, figure and reset on one line, meter beneath.
/// Same shape as the quota list on Usage history. Nothing is fixed-width, so
/// "Example session window" and "17 requests used" stay readable instead of
/// being cut mid-word by a 78px label column and a 72px figure column.
fn window_line(window: &WindowView, theme: &Theme) -> Div {
    let (meter_value, figure, figure_color) = match &window.reading {
        Reading::Fraction(value) => (
            Some(*value),
            format!("{:.0}%", value * 100.0),
            ui::tone_color(ds::remaining_tone(*value), theme),
        ),
        Reading::Count(text) => (None, text.clone(), theme.foreground),
        Reading::Unknown => (None, "Unknown".to_string(), theme.muted_foreground),
        Reading::Unavailable => (None, "Unavailable".to_string(), theme.muted_foreground),
    };
    let reset = match &window.reading {
        Reading::Fraction(_) => ui::format_countdown(window.reset.as_deref()),
        _ => None,
    };
    let label = if matches!(window.reading, Reading::Count(_)) {
        "Count only"
    } else {
        window.label.as_str()
    };
    div()
        .v_flex()
        .gap(px(4.))
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(sp::SM))
                .child(ds::caption(label, theme).flex_1().min_w(px(0.)).truncate())
                .child(ds::mono(figure, ty::CAPTION, figure_color, theme).flex_shrink_0())
                .when_some(reset, |this, reset| {
                    this.child(ds::caption(reset, theme).flex_shrink_0())
                })
                .when(window.stale, |this| this.child(ds::caption("Stale", theme))),
        )
        .child(ds::meter(meter_value, theme))
}

/// A compact count/label pair for the Routes strip. Four of these sit on one
/// row so the rail does not grow when the prose would wrap.
fn route_stat(count: usize, label: &'static str, theme: &Theme) -> Div {
    div()
        .h_flex()
        .items_end()
        .flex_none()
        .gap(px(4.))
        .child(ds::mono(
            count.to_string(),
            ty::SMALL,
            theme.foreground,
            theme,
        ))
        .child(ds::caption(label, theme))
}

/// A usage read that failed is different from a client that publishes no
/// quota window. Keep the distinction visible instead of claiming there is no
/// allowance.
fn usage_unavailable(diagnostics: &[String]) -> bool {
    diagnostics
        .iter()
        .any(|code| code == "USAGE_UNAVAILABLE" || code.starts_with("USAGE_READ_FAILED_"))
}

/// What the client can honestly say about allowance, including the cases
/// where it publishes no quota window at all.
fn allowance_cell(
    view: &AccountView,
    tokens: Option<&originlm_contracts::ProviderUsage>,
    theme: &Theme,
) -> Div {
    if view.windows.is_empty() {
        let (label, detail, tone) = if usage_unavailable(&view.diagnostics) {
            (
                "Usage unavailable",
                "The last usage read failed. Local totals remain; Diagnostics has the retry.",
                Tone::Amber,
            )
        } else if view.status == "configuration_only" {
            (
                "Not installed",
                "Configuration exists; the client is not installed on this machine.",
                Tone::Neutral,
            )
        } else {
            match view.auth.kind {
                data::AuthKind::DiscoveryOnly => (
                    "Discovery only",
                    "OriginLM has no metadata adapter for this provider; sign-in and models were not read.",
                    Tone::Neutral,
                ),
                data::AuthKind::Unavailable => (
                    "Reading unavailable",
                    "The provider read failed. Saved readings remain stale while it retries.",
                    Tone::Red,
                ),
                data::AuthKind::Required => (
                    "No reading",
                    "Sign in to the client to read its allowance.",
                    Tone::Neutral,
                ),
                data::AuthKind::Cached => (
                    "No quota window",
                    "Cached sign-in; the client published no allowance window.",
                    Tone::Neutral,
                ),
                data::AuthKind::Unknown => (
                    "No reading",
                    "Allowance has not been checked yet.",
                    Tone::Neutral,
                ),
                data::AuthKind::Verified if tokens.is_some() => (
                    "No quota window",
                    "This client publishes local totals, not an allowance window.",
                    Tone::Neutral,
                ),
                data::AuthKind::Verified => (
                    "No quota window",
                    "Signed in; this client does not publish an allowance window.",
                    Tone::Neutral,
                ),
            }
        };
        let token_text = tokens.map(|tokens| {
            format!(
                "{} in · {} out",
                compact_tokens(tokens.totals.total_input_tokens()),
                compact_tokens(tokens.totals.output_tokens)
            )
        });
        div()
            .v_flex()
            .w(px(336.))
            .flex_shrink_0()
            .gap(px(4.))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::SM))
                    .child(ds::chip(label, tone, theme))
                    .when_some(token_text, |this, text| {
                        this.child(ds::mono(text, ty::CAPTION, theme.muted_foreground, theme))
                    }),
            )
            .child(ds::caption(detail, theme).truncate())
    } else {
        div()
            .v_flex()
            .gap(px(3.))
            .w(px(336.))
            .flex_shrink_0()
            .children(view.windows.iter().map(|window| window_line(window, theme)))
    }
}

fn provider_row(
    view: &AccountView,
    tokens: Option<&originlm_contracts::ProviderUsage>,
    compact: bool,
    entity: Entity<Desktop>,
    theme: &Theme,
) -> AnyElement {
    let connection_id = view.connection_id.clone();
    let identity = div()
        .h_flex()
        .items_center()
        .gap(px(sp::MD))
        .flex_1()
        .min_w(px(0.))
        .child(crate::icons::provider_tile(
            &view.provider_id,
            &view.provider,
            24.,
            theme,
        ))
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(ty::SMALL))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .truncate()
                        .child(view.provider.clone()),
                )
                .child(ds::caption(view.account.clone(), theme).truncate()),
        );
    let status = div()
        .w(px(104.))
        .flex_shrink_0()
        .child(auth_chip(&view.auth, theme));
    let body = if compact {
        div()
            .v_flex()
            .gap(px(sp::SM))
            .px(px(sp::LG))
            .py(px(sp::XS))
            .border_b_1()
            .border_color(theme.border.opacity(0.7))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .child(identity)
                    .child(status),
            )
            .child(allowance_cell(view, tokens, theme))
    } else {
        div()
            .h_flex()
            .items_center()
            .gap(px(sp::MD))
            // 36px keeps the two-line identity readable while recovering the
            // ~32px that the right-rail wrap cost the Overview fold. Still
            // above the 28px minimum hit target.
            .min_h(px(36.))
            .px(px(sp::LG))
            .py(px(0.))
            .border_b_1()
            .border_color(theme.border.opacity(0.7))
            .child(identity)
            .child(status)
            .child(allowance_cell(view, tokens, theme))
    };
    let window_summary = view
        .windows
        .iter()
        .map(|window| match &window.reading {
            Reading::Fraction(value) => format!("{} {:.0}%", window.label, value * 100.0),
            Reading::Count(text) => format!("{} {}", window.label, text),
            Reading::Unknown => format!("{} unknown", window.label),
            Reading::Unavailable => format!("{} unavailable", window.label),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let row_id = SharedString::from(format!("provider-row-{}", view.connection_id));
    let row = body
        .id(row_id)
        .role(gpui::Role::Button)
        .focus_ring(theme)
        .aria_label(if window_summary.is_empty() {
            format!("{}, {}", view.provider, view.account)
        } else {
            format!("{}, {}, {}", view.provider, view.account, window_summary)
        })
        .cursor_pointer()
        .hover(|style| style.bg(theme.foreground.opacity(0.03)))
        .on_click(move |_, _, cx| {
            let connection_id = connection_id.clone();
            entity.update(cx, |this, cx| {
                this.selected_connection = Some(connection_id);
                this.set_page(Page::Providers, cx);
            });
        });
    #[cfg(test)]
    let row = row.test_support();
    row.into_any_element()
}

impl Desktop {
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
                .py(px(sp::SM))
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
        let wide = content >= 1080.;
        let compact_rows = content < 860.;

        let mut tokens_by_provider: std::collections::BTreeMap<
            &str,
            &originlm_contracts::ProviderUsage,
        > = std::collections::BTreeMap::new();
        if let Some(costs) = self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.costs.as_ref())
        {
            for provider in &costs.historical.providers {
                tokens_by_provider.insert(provider.provider_id.as_str(), provider);
            }
        }

        let entity = cx.entity();
        let providers_panel = ds::panel(&theme)
            .child(ds::panel_head(
                "Providers",
                Some(
                    ds::caption(
                        format!(
                            "{} accounts · {} signed in · {} reporting usage",
                            views.len(),
                            views
                                .iter()
                                .filter(|view| view.auth.kind == data::AuthKind::Verified)
                                .count(),
                            with_readings
                        ),
                        &theme,
                    )
                    .into_any_element(),
                ),
                &theme,
            ))
            .when(!compact_rows, |this| {
                this.child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(sp::MD))
                        .px(px(sp::LG))
                        .py(px(4.))
                        .bg(theme.foreground.opacity(0.03))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().flex_1().child(ds::caption("Account", &theme)))
                        .child(div().w(px(104.)).child(ds::caption("Status", &theme)))
                        .child(div().w(px(336.)).child(ds::caption("Allowance", &theme))),
                )
            })
            .when(views.is_empty(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "No accounts discovered yet",
                    "Discovery found no supported AI client on this machine, so there is no allowance to show. Install or sign in to a client, then refresh. Accounts lists what is supported.",
                    &theme,
                ))
            })
            .children(views.iter().map(|view| {
                provider_row(
                    view,
                    tokens_by_provider
                        .get(view.provider_id.as_str())
                        .copied(),
                    compact_rows,
                    entity.clone(),
                    &theme,
                )
            }));

        // Needs attention.
        let attention: Vec<(Tone, String)> = self
            .snapshot()
            .map(|snapshot| {
                let open = data::open_issues(snapshot);
                let metadata_failures = open
                    .iter()
                    .filter(|issue| issue.title == "Provider metadata could not be refreshed")
                    .count();
                let mut items: Vec<(Tone, String)> = open
                    .iter()
                    .filter(|issue| issue.title != "Provider metadata could not be refreshed")
                    .take(4)
                    .map(|issue| (Tone::Amber, issue.title.clone()))
                    .collect();
                if metadata_failures > 0 {
                    items.push((
                        Tone::Neutral,
                        format!(
                            "{metadata_failures} provider metadata refreshes failed · see Diagnostics"
                        ),
                    ));
                }
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
                items.truncate(6);
                items
            })
            .unwrap_or_default();
        let attention_panel = ds::panel(&theme)
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
                                // Let long alerts wrap inside the 320px rail
                                // instead of being clipped mid-word.
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
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
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::SM))
                            .min_h(px(28.))
                            .child(ds::caption("Routes", &theme))
                            .child(div().flex_1())
                            .child(route_stat(subscription as usize, "subs", &theme))
                            .child(route_stat(metered as usize, "metered", &theme))
                            .child(route_stat(local as usize, "local", &theme))
                            .when(other > 0, |this| {
                                this.child(route_stat(other as usize, "unknown", &theme))
                            }),
                    )
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

        let rail = div()
            .v_flex()
            .gap(px(sp::MD))
            .when(wide, |this| this.w(px(320.)).flex_shrink_0())
            .child(attention_panel)
            .child(build_panel);

        let board = if wide {
            div()
                .h_flex()
                .items_start()
                .gap(px(sp::MD))
                .child(div().flex_1().min_w(px(0.)).child(providers_panel))
                .child(rail)
        } else {
            div()
                .v_flex()
                .gap(px(sp::MD))
                .child(providers_panel)
                .child(rail)
        };

        div()
            .v_flex()
            .w_full()
            .px(px(ds::GUTTER))
            .pb(px(sp::XXL))
            .gap(px(sp::LG))
            .child(strip)
            .child(board)
            .child(self.cost_trend(cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{compact_tokens, usage_unavailable};

    #[test]
    fn compact_tokens_keeps_small_values_exact() {
        assert_eq!(compact_tokens(0), "0");
        assert_eq!(compact_tokens(999), "999");
    }

    #[test]
    fn usage_unavailable_distinguishes_a_failed_read_from_no_quota() {
        assert!(usage_unavailable(&["USAGE_UNAVAILABLE".into()]));
        assert!(usage_unavailable(&[
            "REPORTED_BY_DISCOVERY".into(),
            "USAGE_READ_FAILED_Timeout".into()
        ]));
        assert!(!usage_unavailable(&["MODEL_CATALOG_NOT_REPORTED".into()]));
        assert!(!usage_unavailable(&[]));
    }

    #[test]
    fn compact_tokens_uses_readable_thresholds() {
        assert_eq!(compact_tokens(1_000), "1.0k");
        assert_eq!(compact_tokens(9_999), "10.0k");
        assert_eq!(compact_tokens(10_000), "10k");
        assert_eq!(compact_tokens(749_309), "749k");
        assert_eq!(compact_tokens(1_250_000), "1.2M");
    }
}
