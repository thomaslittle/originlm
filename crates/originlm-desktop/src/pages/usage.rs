//! Usage history: every allowance window with its current reading, and the
//! readings this app has recorded for the selected one.

use crate::app::{Desktop, Range};
use crate::data;
use crate::ds::{self, sp, ty};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, Theme, chart::LineChart};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

const RANGES: [Range; 3] = [Range::Day, Range::Week, Range::Month];

fn series_key(row: &data::QuotaRow) -> String {
    format!("{}\u{1f}{}", row.pool_id, row.window_id)
}

fn local_token_panel(
    report: Option<&originlm_contracts::CostReport>,
    theme: &Theme,
) -> Option<Div> {
    let report = report?;
    let providers = &report.historical.providers;
    Some(
        ds::panel(theme)
            .child(ds::panel_head(
                "Recorded session tokens",
                Some(
                    ds::caption(
                        if providers.is_empty() {
                            "No readable transcript history".to_string()
                        } else {
                            format!("{} providers", providers.len())
                        },
                        theme,
                    )
                    .into_any_element(),
                ),
                theme,
            ))
            .when(providers.is_empty(), |this| {
                this.child(
                    div()
                        .px(px(sp::LG))
                        .py(px(sp::MD))
                        .child(ds::body(
                            "No readable Claude Code, Codex, Gemini, Grok, or Kiro session history. This is not zero usage.",
                            theme,
                        )),
                )
            })
            .children(providers.iter().map(|provider| {
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::SM))
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.6))
                    .child(
                        ds::mono(
                            provider.provider_id.clone(),
                            ty::SMALL,
                            theme.foreground,
                            theme,
                        )
                        .flex_1(),
                    )
                    .child(ds::mono(
                        format!("{} in", provider.totals.total_input_tokens()),
                        ty::CAPTION,
                        theme.muted_foreground,
                        theme,
                    ))
                    .child(ds::mono(
                        format!("{} out", provider.totals.output_tokens),
                        ty::CAPTION,
                        theme.muted_foreground,
                        theme,
                    ))
                    .when(provider.totals.reasoning_tokens > 0, |this| {
                        this.child(ds::mono(
                            format!("{} reasoning", provider.totals.reasoning_tokens),
                            ty::CAPTION,
                            theme.muted_foreground,
                            theme,
                        ))
                    })
                    .when(!provider.diagnostics.is_empty(), |this| {
                        this.child(ds::chip(
                            format!("{} diagnostics", provider.diagnostics.len()),
                            Tone::Amber,
                            theme,
                        ))
                    })
                    .into_any_element()
            }))
            .child(
                div()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::caption(
                        "Fresh input plus cache reads/writes; reasoning is a subset of output. Local transcript totals, not invoices or quota.",
                        theme,
                    )),
            ),
    )
}

impl Desktop {
    pub fn page_usage(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.snapshot().cloned() else {
            return div().into_any_element();
        };
        let entity = cx.entity();
        let mut rows = data::quota_rows(&snapshot);
        // One row per pool and window.
        let mut seen = std::collections::BTreeSet::new();
        rows.retain(|row| seen.insert(series_key(row)));

        let local_tokens = local_token_panel(
            self.bundle
                .as_ref()
                .and_then(|bundle| bundle.costs.as_ref()),
            &theme,
        );
        if rows.is_empty() {
            let mut page = ds::page().child(ds::panel(&theme).child(ds::state(
                ds::StateKind::Empty,
                "No allowance windows reported",
                "None of the discovered clients reports a quota window. Local session-token totals and Accounts are still available below.",
                &theme,
            )));
            if let Some(panel) = local_tokens {
                page = page.child(panel);
            }
            return page.into_any_element();
        }

        if self
            .selected_series
            .as_ref()
            .is_none_or(|key| !rows.iter().any(|row| &series_key(row) == key))
        {
            self.selected_series = rows.first().map(series_key);
        }
        let selected_key = self.selected_series.clone();
        let current = rows
            .iter()
            .find(|row| Some(series_key(row)) == selected_key)
            .cloned();

        // Series list with meters.
        let list = ds::panel(&theme)
            .child(ds::panel_head(
                "Allowance windows",
                Some(ds::caption(ui::plural(rows.len(), "window"), &theme).into_any_element()),
                &theme,
            ))
            .child(
                div()
                    .v_flex()
                    .py(px(sp::XS))
                    .children(rows.iter().map(|row| {
                        let key = series_key(row);
                        let selected = Some(&key) == selected_key.as_ref();
                        let entity = entity.clone();
                        let figure = row
                            .remaining
                            .map(|value| format!("{:.0}% left", value * 100.0))
                            .unwrap_or_else(|| "Unknown".into());
                        let figure_color = match row.remaining {
                            Some(value) if value < 0.35 => {
                                ui::tone_color(ds::remaining_tone(value), &theme)
                            }
                            _ => theme.foreground,
                        };
                        ds::select_row(
                            SharedString::from(format!("series-{key}")),
                            selected,
                            &theme,
                        )
                        .aria_label(format!("{}, {figure}", data::quota_label(row)))
                        .py(px(sp::SM))
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w(px(0.))
                                .gap(px(6.))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(sp::SM))
                                        .child(crate::icons::provider_tile(
                                            &row.provider_id,
                                            &row.pool_label,
                                            20.,
                                            &theme,
                                        ))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .truncate()
                                                .text_size(px(ty::SMALL))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(data::quota_label(row)),
                                        )
                                        .when(!row.fresh && row.remaining.is_some(), |this| {
                                            this.child(ds::chip("Stale", Tone::Amber, &theme))
                                        })
                                        .child(ds::mono(figure, ty::SMALL, figure_color, &theme)),
                                )
                                .child(ds::meter(row.remaining, &theme)),
                        )
                        .on_click(move |_, _, cx| {
                            let key = key.clone();
                            entity.update(cx, |this, cx| {
                                this.selected_series = Some(key);
                                cx.notify();
                            });
                        })
                    })),
            );

        // History of the selected window.
        let since = chrono::Utc::now().timestamp() - self.range.seconds();
        let points = self
            .series_key()
            .map(|(pool, window)| data::series_points(self.history(), &pool, &window, since))
            .unwrap_or_default();
        let flat = points.len() >= 2 && data::chart_variation(&points) < 0.5;
        let range_index = RANGES.iter().position(|r| *r == self.range).unwrap_or(0);
        let controls = div()
            .h_flex()
            .gap(px(sp::SM))
            .child(ds::tabs(
                "usage-range",
                RANGES.map(|range| range.label()),
                range_index,
                &theme,
                {
                    let entity = entity.clone();
                    move |index, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.range = RANGES[index];
                            cx.notify();
                        })
                    }
                },
            ))
            .child(ds::tabs(
                "usage-view",
                ["Chart", "Table"],
                usize::from(self.usage_table),
                &theme,
                {
                    let entity = entity.clone();
                    move |index, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.usage_table = index == 1;
                            cx.notify();
                        })
                    }
                },
            ));

        let body: AnyElement = if self.example && points.len() < 2 {
            ds::state(
                ds::StateKind::Empty,
                "No recorded history in example mode",
                "Example mode records nothing. With a runtime attached, this app stores each allowance reading it sees on this machine and draws them here.",
                &theme,
            )
            .into_any_element()
        } else if points.len() < 2 {
            ds::state(
                ds::StateKind::Empty,
                "History starts with this app",
                format!(
                    "Clients report the current reading, not a past series. This app records each reading it sees and never invents earlier points. {} in this range so far.",
                    ui::plural(points.len(), "reading")
                ),
                &theme,
            )
            .into_any_element()
        } else if self.usage_table {
            self.usage_table_view(&points, &theme)
        } else if flat {
            let latest = points.last().map(|p| p.value).unwrap_or(0.0);
            div()
                .v_flex()
                .gap(px(sp::MD))
                .p(px(sp::LG))
                .child(ds::stat(
                    format!("{latest:.0}% left"),
                    format!(
                        "unchanged across {} readings in {}",
                        points.len(),
                        self.range.label()
                    ),
                    &theme,
                ))
                .child(ds::meter(Some((latest / 100.0) as f32), &theme))
                .child(ds::caption(
                    "A flat line adds nothing, so the latest reading is shown instead.",
                    &theme,
                ))
                .into_any_element()
        } else {
            let first = points.first().map(|p| p.value).unwrap_or(0.0);
            let latest = points.last().map(|p| p.value).unwrap_or(0.0);
            div()
                .v_flex()
                .gap(px(sp::SM))
                .p(px(sp::LG))
                .child(ds::stat(
                    format!("{latest:.0}% left"),
                    format!("{:+.1} points over {}", latest - first, self.range.label()),
                    &theme,
                ))
                .child(
                    div().h(px(220.)).w_full().child(
                        LineChart::new(points.clone())
                            .x(|point| point.label.clone())
                            .y(|point| point.value())
                            .name("Left")
                            .stroke(theme.chart_1)
                            .linear()
                            .y_padding(6., 0.)
                            .y_axis(true)
                            .y_tick_count(3)
                            .y_tick_format(|value| format!("{value:.0}%"))
                            .x_tick_count(6)
                            .tooltip_value(|_, value| {
                                SharedString::from(format!("{value:.1}% left"))
                            })
                            .grid_columns(4),
                    ),
                )
                .into_any_element()
        };

        let history = ds::panel(&theme)
            .flex_1()
            .min_w(px(0.))
            .child(ds::panel_head(
                current
                    .as_ref()
                    .map(data::quota_label)
                    .unwrap_or_else(|| "History".into()),
                Some(controls.into_any_element()),
                &theme,
            ))
            .child(body)
            .child(
                div()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(ds::caption(
                        "Percent left in this window, as the client reported it. Pools are never summed; a gap means no reading, not zero.",
                        &theme,
                    )),
            );

        // Reset schedule.
        let resets: Vec<&data::QuotaRow> =
            rows.iter().filter(|row| row.resets_at.is_some()).collect();
        let schedule = ds::panel(&theme)
            .child(ds::panel_head("Reset schedule", None, &theme))
            .when(resets.is_empty(), |this| {
                this.child(
                    div()
                        .px(px(sp::LG))
                        .py(px(sp::MD))
                        .child(ds::body("No client reported a reset time.", &theme)),
                )
            })
            .when(!resets.is_empty(), |this| {
                this.child(
                    ds::table_head(&theme)
                        .child(ds::th("Window", None, &theme))
                        .child(ds::th("Resets in", Some(110.), &theme))
                        .child(ds::th("Local time", Some(110.), &theme)),
                )
            })
            .children(resets.iter().enumerate().map(|(index, row)| {
                let clock = row
                    .resets_at
                    .as_deref()
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                    .map(|stamp| {
                        stamp
                            .with_timezone(&chrono::Local)
                            .format("%a %H:%M")
                            .to_string()
                    })
                    .unwrap_or_default();
                ds::panel_row(index == 0, &theme)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(ty::SMALL))
                            .child(data::quota_label(row)),
                    )
                    .child(
                        div().w(px(110.)).flex_shrink_0().child(ds::mono(
                            ui::format_countdown(row.resets_at.as_deref())
                                .unwrap_or_else(|| "unknown".into()),
                            ty::SMALL,
                            theme.foreground,
                            &theme,
                        )),
                    )
                    .child(div().w(px(110.)).flex_shrink_0().child(ds::mono(
                        clock,
                        ty::SMALL,
                        theme.muted_foreground,
                        &theme,
                    )))
            }));

        let wide = self.content_width() >= 1000.;
        let right = div()
            .v_flex()
            .flex_1()
            .min_w(px(0.))
            .gap(px(sp::MD))
            .child(history)
            .child(schedule);
        let mut page = ds::page().child(if wide {
            div()
                .h_flex()
                .items_start()
                .gap(px(sp::MD))
                .child(div().w(px(380.)).flex_shrink_0().child(list))
                .child(right)
                .into_any_element()
        } else {
            div()
                .v_flex()
                .gap(px(sp::MD))
                .child(list)
                .child(right)
                .into_any_element()
        });
        if let Some(panel) = local_tokens {
            page = page.child(panel);
        }
        page.into_any_element()
    }

    fn usage_table_view(&self, points: &[data::ChartPoint], theme: &Theme) -> AnyElement {
        div()
            .v_flex()
            .child(
                ds::table_head(theme)
                    .child(ds::th("Time", None, theme))
                    .child(ds::th("Left", Some(80.), theme)),
            )
            .children(
                points
                    .iter()
                    .rev()
                    .take(24)
                    .enumerate()
                    .map(|(index, point)| {
                        ds::panel_row(index == 0, theme)
                            .min_h(px(30.))
                            .child(
                                ds::mono(
                                    point.label.clone(),
                                    ty::SMALL,
                                    theme.muted_foreground,
                                    theme,
                                )
                                .flex_1(),
                            )
                            .child(div().w(px(80.)).child(ds::mono(
                                format!("{:.0}%", point.value),
                                ty::SMALL,
                                theme.foreground,
                                theme,
                            )))
                    }),
            )
            .when(points.len() > 24, |this| {
                this.child(div().px(px(sp::LG)).py(px(sp::SM)).child(ds::caption(
                    format!("Latest 24 of {} readings.", points.len()),
                    theme,
                )))
            })
            .into_any_element()
    }
}
