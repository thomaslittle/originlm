use crate::app::{Desktop, Range};
use crate::data;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, IconName, button::ButtonVariants, chart::LineChart};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_usage(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.snapshot().cloned() else {
            return div()
                .px(px(ui::PAGE_PAD))
                .child(ui::empty_state(
                    IconName::ChartPie,
                    "Waiting for usage",
                    "Allowance observations appear once the broker reports a provider quota.",
                    &theme,
                ))
                .into_any_element();
        };
        let rows = data::quota_rows(&snapshot);
        let options: Vec<(String, String)> = {
            let mut collected: Vec<(String, String)> = Vec::new();
            for row in &rows {
                let key = format!("{}\u{1f}{}", row.pool_id, row.window_id);
                if !collected.iter().any(|(k, _)| k == &key) {
                    collected.push((key, data::quota_label(row)));
                }
            }
            collected
        };
        if self.selected_series.is_none() {
            self.selected_series = rows
                .iter()
                .filter(|row| row.remaining.is_some())
                .max_by(|a, b| {
                    a.remaining
                        .unwrap_or(0.0)
                        .partial_cmp(&b.remaining.unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|row| format!("{}\u{1f}{}", row.pool_id, row.window_id))
                .or_else(|| options.first().map(|(key, _)| key.clone()));
        }
        let since = chrono::Utc::now().timestamp() - self.range.seconds();
        let series = self.series_key();
        let points = series
            .as_ref()
            .map(|(pool, window)| data::series_points(self.history(), pool, window, since))
            .unwrap_or_default();

        let chart: AnyElement = if points.len() < 2 {
            ui::empty_state(
                IconName::ChartPie,
                "History starts with this session",
                "The broker exposes point-in-time allowance readings, not a past series. This app records each reading it sees and never invents earlier points.",
                &theme,
            )
            .into_any_element()
        } else if data::chart_variation(&points) < 0.5 {
            let latest = points.last().map(|point| point.value).unwrap_or(0.0);
            div()
                .v_flex()
                .justify_center()
                .gap_4()
                .h_full()
                .child(
                    div()
                        .h_flex()
                        .items_end()
                        .gap_2()
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_size(px(44.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{latest:.0}%")),
                        )
                        .child(
                            div()
                                .pb(px(7.))
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("of this window used"),
                        ),
                )
                .child(ui::bar((latest / 100.0) as f32, Tone::Amber, &theme))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.muted_foreground)
                        .child("Usage has not changed in this range, so a flat line would add nothing. The current share is shown instead."),
                )
                .into_any_element()
        } else {
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
                .x_tick_count(6)
                .tooltip_value(|_, value| SharedString::from(format!("{value:.1}% used")))
                .grid_columns(4)
                .into_any_element()
        };

        let flat = data::chart_variation(&points) < 0.5;
        let flat_points = data::chart_variation(&points) < 0.5;
        let chart_height = if points.len() < 2 || flat_points {
            150.
        } else {
            240.
        };
        let table: AnyElement = div()
            .v_flex()
            .child(
                div()
                    .h_flex()
                    .gap_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(ui::eyebrow("Time", &theme))
                    .child(div().flex_1())
                    .child(ui::eyebrow("Used", &theme)),
            )
            .children(
                points
                    .iter()
                    .rev()
                    .take(if flat { 1 } else { 12 })
                    .map(|point| {
                        div()
                            .h_flex()
                            .gap_4()
                            .py_1()
                            .border_b_1()
                            .border_color(theme.border.opacity(0.45))
                            .child(ui::mono(point.label.clone(), &theme))
                            .child(div().flex_1())
                            .child(ui::numeric(format!("{:.0}%", point.value), &theme))
                            .into_any_element()
                    }),
            )
            .when(points.is_empty(), |this| {
                this.child(
                    div()
                        .py_4()
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child("No observations in this range."),
                )
            })
            .when(flat || points.len() > 12, |this| {
                this.child(
                    div()
                        .pt_2()
                        .text_size(px(10.))
                        .text_color(theme.muted_foreground)
                        .child(if flat {
                            format!(
                                "No change across {} observations; the latest is shown.",
                                points.len()
                            )
                        } else {
                            format!("Showing the latest 12 of {} observations.", points.len())
                        }),
                )
            })
            .into_any_element();

        let reset_rows: Vec<AnyElement> = rows
            .iter()
            .filter(|row| row.resets_at.is_some())
            .map(|row| {
                div()
                    .h_flex()
                    .gap_2()
                    .py(px(6.))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .child(div().text_size(px(11.5)).child(data::quota_label(row)))
                    .child(div().flex_1())
                    .child(ui::mono(
                        ui::format_countdown(row.resets_at.as_deref())
                            .unwrap_or_else(|| "unknown".into()),
                        &theme,
                    ))
                    .child(ui::mono(
                        row.resets_at
                            .as_deref()
                            .and_then(|value| {
                                chrono::DateTime::parse_from_rfc3339(value)
                                    .ok()
                                    .map(|stamp| {
                                        stamp
                                            .with_timezone(&chrono::Local)
                                            .format("%a %H:%M")
                                            .to_string()
                                    })
                            })
                            .unwrap_or_default(),
                        &theme,
                    ))
                    .into_any_element()
            })
            .collect();

        div()
            .v_flex()
            .w_full()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children([Range::Day, Range::Week, Range::Month].map(|range| {
                        let entity = cx.entity();
                        ui::tab_pill(
                            SharedString::from(format!("range-{}", range.label())),
                            range.label(),
                            self.range == range,
                            &theme,
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.range = range;
                                    cx.notify();
                                });
                            },
                        )
                        .into_any_element()
                    }))
                    .child(div().w(px(12.)))
                    .children(options.iter().map(|(key, label)| {
                        let selected = self.selected_series.as_deref() == Some(key.as_str());
                        let key = key.clone();
                        let entity = cx.entity();
                        ui::tab_pill(
                            SharedString::from(format!("series-{label}")),
                            label.clone(),
                            selected,
                            &theme,
                            move |_, _, cx| {
                                let key = key.clone();
                                entity.update(cx, |this, cx| {
                                    this.selected_series = Some(key.clone());
                                    cx.notify();
                                });
                            },
                        )
                        .into_any_element()
                    })),
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
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .child(ui::section_title("Used allowance", &theme))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(ui::mono(
                                        format!(
                                            "{} points, {}",
                                            points.len(),
                                            self.range.label()
                                        ),
                                        &theme,
                                    ))
                                    .child({
                                        let entity = cx.entity();
                                        let label = if self.usage_table {
                                            "Chart"
                                        } else {
                                            "Table"
                                        };
                                        gpui_kit::component::button::Button::new("usage-view")
                                            .secondary()
                                            .label(label)
                                            .on_click(move |_, _, cx| {
                                                entity.update(cx, |this, cx| {
                                                    this.usage_table = !this.usage_table;
                                                    cx.notify();
                                                });
                                            })
                                    }),
                            ),
                    )
                    .when(points.len() >= 2 && !self.usage_table, |this| {
                        let first = points.first().map(|point| point.value).unwrap_or(0.0);
                        let latest = points.last().map(|point| point.value).unwrap_or(0.0);
                        this.child(
                            div()
                                .h_flex()
                                .gap_3()
                                .child(ui::mono(format!("{latest:.0}% used now"), &theme))
                                .child(ui::mono(
                                    format!("{:+.1} points in {}", latest - first, self.range.label()),
                                    &theme,
                                )),
                        )
                    })
                    .child(
                        div()
                            .h(px(chart_height))
                            .w_full()
                            .when(self.usage_table, |this| this.hidden())
                            .child(chart),
                    )
                    .when(self.usage_table, |this| this.child(table))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("Share of each allowance used, as reported by the provider. Percentages from different pools are never summed; a gap means no observation, not zero usage."),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .v_flex()
                            .gap_2()
                            .p_4()
                            .rounded(px(ui::CARD_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.colors.list)
                            .child(ui::section_title("Reset schedule", &theme))
                            .when(reset_rows.is_empty(), |this| {
                                this.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.muted_foreground)
                                        .child("No provider reported a reset time."),
                                )
                            })
                            .children(reset_rows),
                    )
                    .child(
                        div()
                            .w(px(320.))
                            .v_flex()
                            .gap_2()
                            .p_4()
                            .rounded(px(ui::CARD_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.colors.list)
                            .child(ui::section_title("Observations", &theme))
                            .children(rows.iter().take(6).map(|row| {
                                let tone = if !row.fresh {
                                    Tone::Neutral
                                } else if row.remaining.unwrap_or(1.0) < 0.2 {
                                    Tone::Red
                                } else {
                                    Tone::Green
                                };
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .py(px(5.))
                                    .child(ui::dot(tone, &theme))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .child(data::quota_label(row)),
                                    )
                                    .child(div().flex_1())
                                    .child(ui::mono(
                                        if row.fresh {
                                            "fresh"
                                        } else {
                                            "stale"
                                        }
                                        .to_string(),
                                        &theme,
                                    ))
                                    .into_any_element()
                            }))
                            .child(
                                div()
                                    .pt_2()
                                    .text_size(px(10.))
                                    .text_color(theme.muted_foreground)
                                    .child("Stale means the broker cached a previous reading; it is not a live measurement."),
                            ),
                    ),
            )
            .into_any_element()
    }
}
