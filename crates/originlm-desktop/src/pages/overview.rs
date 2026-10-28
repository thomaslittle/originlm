use crate::app::{Desktop, Page};
use crate::data;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, IconName, chart::LineChart};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_overview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let counts = self.counts();
        let quota = self.snapshot().map(data::quota_rows).unwrap_or_default();
        let issues = self
            .snapshot()
            .map(|snapshot| data::open_issues(snapshot).len())
            .unwrap_or(0);

        let provider_rows: Vec<(String, usize, usize, usize)> = self
            .snapshot()
            .map(|snapshot| {
                snapshot
                    .providers
                    .iter()
                    .filter_map(|provider| {
                        let connections: Vec<_> = snapshot
                            .connections
                            .iter()
                            .filter(|connection| connection.provider_id == provider.id)
                            .collect();
                        if connections.is_empty() {
                            return None;
                        }
                        let verified = connections
                            .iter()
                            .filter(|c| data::auth_view(c).kind == data::AuthKind::Verified)
                            .count();
                        let cached = connections
                            .iter()
                            .filter(|c| data::auth_view(c).kind == data::AuthKind::Cached)
                            .count();
                        Some((provider.name.clone(), connections.len(), verified, cached))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // One card per allowance pool, most constrained first.
        let mut constrained: Vec<data::QuotaRow> = quota
            .iter()
            .filter(|row| row.remaining.is_some())
            .cloned()
            .collect();
        constrained.sort_by(|a, b| {
            a.remaining
                .partial_cmp(&b.remaining)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::BTreeSet::new();
        constrained.retain(|row| seen.insert(row.pool_id.clone()));
        constrained.truncate(3);

        let quota_cards = div()
            .h_flex()
            .gap_3()
            .children(constrained.iter().map(|row| {
                let remaining = row.remaining.unwrap_or(0.0);
                let tone = if remaining < 0.2 {
                    Tone::Red
                } else if remaining < 0.5 {
                    Tone::Amber
                } else {
                    Tone::Green
                };
                let reset = ui::format_countdown(row.resets_at.as_deref())
                    .unwrap_or_else(|| "reset not reported".into());
                div()
                    .flex_1()
                    .min_w(px(170.))
                    .p(px(14.))
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .child(crate::icons::provider_tile(
                                &row.provider_id,
                                &row.pool_label,
                                52.,
                                &theme,
                            ))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.))
                                    .min_w(px(0.))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .child(row.pool_label.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(theme.muted_foreground)
                                            .truncate()
                                            .child(crate::human::title_case(
                                                &row.window_label.replace('_', " "),
                                            )),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_end()
                            .gap_1()
                            .child(
                                div()
                                    .font_family(theme.mono_font_family.clone())
                                    .text_size(px(26.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{:.0}%", remaining * 100.0)),
                            )
                            .child(
                                div()
                                    .pb(px(4.))
                                    .text_size(px(10.5))
                                    .text_color(theme.muted_foreground)
                                    .child("remaining"),
                            ),
                    )
                    .child(ui::bar(remaining, tone, &theme))
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child(reset)
                            .child(if row.fresh {
                                "fresh".to_string()
                            } else {
                                "stale".to_string()
                            }),
                    )
                    .into_any_element()
            }));

        let soonest = quota
            .iter()
            .filter_map(|row| {
                let stamp = row
                    .resets_at
                    .as_deref()
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())?;
                Some((stamp, row))
            })
            .min_by_key(|(stamp, _)| *stamp);
        let next_reset = soonest.map(|(_, row)| {
            div()
                .w(px(252.))
                .p(px(14.))
                .rounded(px(ui::CARD_RADIUS))
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.list)
                .v_flex()
                .gap_2()
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_3()
                        .child(crate::icons::provider_tile(
                            &row.provider_id,
                            &row.pool_label,
                            52.,
                            &theme,
                        ))
                        .child(
                            div()
                                .v_flex()
                                .gap(px(2.))
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(theme.muted_foreground)
                                        .child("Next reset"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(row.pool_label.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(crate::human::title_case(
                                            &row.window_label.replace('_', " "),
                                        )),
                                ),
                        ),
                )
                .child(
                    div()
                        .font_family(theme.mono_font_family.clone())
                        .text_size(px(18.))
                        .child(
                            ui::format_countdown(row.resets_at.as_deref())
                                .unwrap_or_else(|| "unknown".into()),
                        ),
                )
                .child(ui::mono(
                    row.resets_at
                        .as_deref()
                        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                        .map(|stamp| {
                            stamp
                                .with_timezone(&chrono::Local)
                                .format("%a %H:%M")
                                .to_string()
                        })
                        .unwrap_or_default(),
                    &theme,
                ))
        });

        let cached_only: Vec<String> = self
            .snapshot()
            .map(|snapshot| {
                snapshot
                    .connections
                    .iter()
                    .filter(|connection| data::auth_view(connection).kind == data::AuthKind::Cached)
                    .map(|connection| data::provider_name(snapshot, &connection.provider_id))
                    .collect()
            })
            .unwrap_or_default();

        let unloaded: Vec<(String, String)> = self
            .snapshot()
            .map(|snapshot| {
                snapshot
                    .connections
                    .iter()
                    .filter(|connection| {
                        matches!(
                            connection.auth,
                            originlm_contracts::Observation::Known {
                                value: originlm_contracts::AuthState::Authenticated,
                                ..
                            }
                        ) && !snapshot
                            .quota_pools
                            .iter()
                            .any(|pool| pool.connection_ids.contains(&connection.id))
                    })
                    .map(|connection| {
                        (
                            data::provider_name(snapshot, &connection.provider_id),
                            connection.id.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let series = self.series_key();
        let series_label = series.as_ref().and_then(|(pool, window)| {
            quota
                .iter()
                .find(|row| &row.pool_id == pool && &row.window_id == window)
                .map(data::quota_label)
        });
        let points: Vec<data::ChartPoint> = series
            .as_ref()
            .map(|(pool, window)| {
                data::series_points(
                    self.history(),
                    pool,
                    window,
                    chrono::Utc::now().timestamp() - self.range.seconds(),
                )
            })
            .unwrap_or_default();

        let chart: AnyElement = if points.len() < 2 {
            ui::empty_state(
                IconName::ChartPie,
                "Collecting observations",
                "This app records the allowance the broker actually reports. A trend appears after the second observation.",
                &theme,
            )
            .into_any_element()
        } else if data::chart_variation(&points) < 0.5 {
            let latest = points.last().map(|point| point.value).unwrap_or(0.0);
            div()
                .v_flex()
                .justify_center()
                .gap_3()
                .h_full()
                .child(
                    div()
                        .h_flex()
                        .items_end()
                        .gap_2()
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_size(px(34.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{latest:.0}%")),
                        )
                        .child(
                            div()
                                .pb(px(5.))
                                .text_size(px(11.5))
                                .text_color(theme.muted_foreground)
                                .child("of this window used"),
                        ),
                )
                .child(ui::bar(
                    (latest / 100.0) as f32,
                    if latest > 85.0 { Tone::Red } else { Tone::Amber },
                    &theme,
                ))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child("No change in this range, so the current figure is shown instead of a flat line."),
                )
                .into_any_element()
        } else {
            let first = points.first().map(|point| point.value).unwrap_or(0.0);
            let latest = points.last().map(|point| point.value).unwrap_or(0.0);
            let delta = latest - first;
            div()
                .v_flex()
                .gap_2()
                .h_full()
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(ui::mono(format!("{latest:.0}% used now"), &theme))
                        .child(ui::mono(
                            format!("{delta:+.1} points in {}", self.range.label()),
                            &theme,
                        )),
                )
                .child(
                    div().flex_1().child(
                        LineChart::new(points.clone())
                            .x(|point| point.label.clone())
                            .y(|point| point.value())
                            .name("Used")
                            .stroke(theme.chart_1)
                            .linear()
                            .y_padding(6., 0.)
                            .y_axis(true)
                            .y_tick_count(3)
                            .y_tick_format(|value| format!("{value:.0}%"))
                            .x_tick_count(4)
                            .tooltip_value(|_, value| {
                                SharedString::from(format!("{value:.1}% used"))
                            })
                            .grid_columns(0),
                    ),
                )
                .into_any_element()
        };

        let mut used_rows: Vec<(String, f32)> = quota
            .iter()
            .filter_map(|row| {
                let remaining = row.remaining?;
                Some((data::quota_label(row), (1.0 - remaining).clamp(0.0, 1.0)))
            })
            .collect();
        used_rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        used_rows.truncate(5);
        let used_list: AnyElement = div()
            .v_flex()
            .gap_1()
            .when(used_rows.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child("No allowance window is reported."),
                )
            })
            .children(used_rows.into_iter().map(|(label, used)| {
                div()
                    .v_flex()
                    .gap(px(3.))
                    .py(px(4.))
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.muted_foreground)
                                    .child(ui::short(&label, 26)),
                            )
                            .child(div().flex_1())
                            .child(ui::numeric(format!("{:.0}%", used * 100.0), &theme)),
                    )
                    .child(ui::bar(
                        used,
                        if used > 0.85 { Tone::Red } else { Tone::Amber },
                        &theme,
                    ))
                    .into_any_element()
            }))
            .into_any_element();

        let attention_titles: Vec<String> = self
            .snapshot()
            .map(|snapshot| {
                data::open_issues(snapshot)
                    .iter()
                    .take(3)
                    .map(|issue| issue.title.clone())
                    .chain(
                        snapshot
                            .connections
                            .iter()
                            .filter(|connection| {
                                data::auth_view(connection).kind == data::AuthKind::Required
                            })
                            .map(|connection| format!("{} needs sign-in", connection.display_name)),
                    )
                    .collect()
            })
            .unwrap_or_default();
        let entity = cx.entity();
        let attention_card = div()
            .w(px(360.))
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
                    .child(ui::section_title("Needs attention", &theme))
                    .child({
                        let entity = entity.clone();
                        div()
                            .id("all-diagnostics")
                            .text_size(px(10.5))
                            .text_color(theme.primary)
                            .cursor_pointer()
                            .child("View all")
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.set_page(Page::Diagnostics, cx));
                            })
                    }),
            )
            .when(attention_titles.is_empty(), |this| {
                this.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .py_2()
                        .child(ui::icon(IconName::CircleCheck, 14., theme.success))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(theme.muted_foreground)
                                .child("No issues were reported."),
                        ),
                )
            })
            .children(attention_titles.into_iter().map(|title| {
                div()
                    .h_flex()
                    .gap_2()
                    .py(px(6.))
                    .child(ui::icon(IconName::CircleAlert, 12., theme.warning))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .into_any_element()
            }));

        div()
            .v_flex()
            .w_full()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .px_5()
                    .py_4()
                    .gap_8()
                    .child(ui::metric(
                        "Providers",
                        counts.providers.to_string(),
                        Some(format!("{} connections", counts.connections).into()),
                        &theme,
                    ))
                    .child(ui::metric(
                        "Verified",
                        format!("{} / {}", counts.authenticated, counts.connections),
                        Some(if counts.cached > 0 {
                            format!("{} cache-only", counts.cached).into()
                        } else {
                            "all checked".into()
                        }),
                        &theme,
                    ))
                    .child(ui::metric("Models", counts.models.to_string(), None, &theme))
                    .child(ui::metric(
                        "Broker clients",
                        counts.apps.to_string(),
                        Some(if self.example { "example mode; no broker".into() } else { "reported by the broker".into() }),
                        &theme,
                    ))
                    .child(div().flex_1())
                    .child(ui::metric(
                        "Reported issues",
                        if self.connected { issues.to_string() } else { "Unknown".into() },
                        Some(if issues == 0 {
                            "none reported".into()
                        } else {
                            "review in Diagnostics".into()
                        }),
                        &theme,
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .items_stretch()
                    .gap_3()
                    .child(quota_cards.flex_1())
                    .children(next_reset),
            )
            .child(self.cost_trend(cx))
            .when(!cached_only.is_empty(), |this| {
                this.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.muted)
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "{} has stale or cache-only sign-in evidence. Check the provider before relying on access.",
                                    cached_only.join(" and ")
                                )),
                        ),
                )
            })
            .when(!unloaded.is_empty(), |this| {
                let entity = cx.entity();
                this.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.muted)
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "No allowance reported for {}.",
                                    unloaded
                                        .iter()
                                        .map(|(name, _)| name.clone())
                                        .collect::<Vec<_>>()
                                        .join(" and ")
                                )),
                        )
                        .child(div().flex_1())
                        .children(unloaded.into_iter().map(|(name, id)| {
                            let entity = entity.clone();
                            div()
                                .id(SharedString::from(format!("check-usage-{id}")))
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.primary)
                                .cursor_pointer()
                                .child(format!("Check {name} usage"))
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    entity.update(cx, |this, cx| {
                                        let _ = this
                                            .broker
                                            .commands
                                            .send(crate::broker::Command::Probe(id));
                                        this.toast("Metadata refresh requested");
                                        cx.notify();
                                    });
                                })
                                .into_any_element()
                        })),
                )
            })
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .child(ui::section_title("Usage over time", &theme))
                            .child(ui::mono(
                                {
                                    let mut parts = Vec::new();
                                    if let Some(label) = series_label {
                                        parts.push(label);
                                    } else {
                                        parts.push("No series".to_string());
                                    }
                                    parts.push(format!("{} observations", points.len()));
                                    parts.push(self.range.label().to_string());
                                    parts.join(", ")
                                },
                                &theme,
                            )),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_start()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .h(px(168.))
                                    .child(chart),
                            )
                            .child(
                                div()
                                    .w(px(300.))
                                    .v_flex()
                                    .gap_2()
                                    .pl_4()
                                    .border_l_1()
                                    .border_color(theme.border.opacity(0.7))
                                    .child(ui::section_title("Used by window", &theme))
                                    .child(used_list),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("Share of each allowance used, provider-reported. Gaps mean the app was not running, not zero usage."),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_stretch()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .v_flex()
                            .gap_2()
                            .p_4()
                            .rounded(px(ui::CARD_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.colors.list)
                            .child(ui::section_title("Access", &theme))
                            .children(provider_rows.into_iter().map(|(name, total, verified, cached)| {
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .py(px(6.))
                                    .border_b_1()
                                    .border_color(theme.border.opacity(0.6))
                                    .child(ui::dot(
                                        if verified > 0 {
                                            Tone::Green
                                        } else if cached > 0 {
                                            Tone::Amber
                                        } else {
                                            Tone::Neutral
                                        },
                                        &theme,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(name),
                                    )
                                    .child(div().flex_1())
                                    .child(ui::mono(
                                        if verified > 0 {
                                            format!("{verified}/{total} authenticated")
                                        } else if cached > 0 {
                                            format!("{cached} signed in, unverified")
                                        } else {
                                            "No verified session".to_string()
                                        },
                                        &theme,
                                    ))
                                    .into_any_element()
                            })),
                    )
                    .child(attention_card),
            )
            .into_any_element()
    }
}
