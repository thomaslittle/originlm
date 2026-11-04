use crate::{app::Desktop, broker::Command, ui};
use gpui_kit::base::{Disableable as _, StyledExt as _};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn cost_trend(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::ds::{self, sp, ty};
        let theme = cx.theme().clone();
        let report = self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.costs.as_ref());
        let records: Vec<_> = report
            .map(|report| report.records.iter().rev().take(24).rev().collect())
            .unwrap_or_default();
        let amount = |record: &&originlm_contracts::CostRecord| {
            record
                .amount_usd
                .as_ref()
                .and_then(|value| value.as_str().parse::<f64>().ok())
                .filter(|value| value.is_finite())
        };
        let maximum = records
            .iter()
            .filter_map(amount)
            .fold(0.000001_f64, f64::max);
        let estimated = records
            .iter()
            .filter(|record| record.basis == "estimated")
            .count();
        let title = if self.example {
            "Observed turn cost (example data)"
        } else {
            "Observed turn cost"
        };
        ds::panel(&theme)
            .child(ds::panel_head(
                title,
                Some(
                    ds::caption(
                        if records.is_empty() {
                            "No turns yet".to_string()
                        } else {
                            format!(
                                "{} · {} estimated · max ${maximum:.4}",
                                ui::plural(records.len(), "turn"),
                                estimated
                            )
                        },
                        &theme,
                    )
                    .into_any_element(),
                ),
                &theme,
            ))
            .children(
                self.bundle
                    .as_ref()
                    .and_then(|bundle| bundle.cost_error.clone())
                    .map(|error| ds::body(error, &theme).px(px(sp::LG)).pt(px(sp::MD))),
            )
            .when(records.is_empty(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "No cost evidence yet",
                    "A turn shows here once the runtime reports its amount, or its token counts and a matching local price exist. Nothing is estimated without both.",
                    &theme,
                ))
            })
            .when(!records.is_empty(), |this| this.child(
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(sp::XL))
                    .p(px(sp::LG))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .v_flex()
                            .gap(px(sp::SM))
                            .child(
                                div()
                                    .h_flex()
                                    .items_end()
                                    .gap(px(3.))
                                    .h(px(72.))
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .children(records.iter().map(|record| {
                                        let value = amount(record);
                                        let height = value
                                            .map(|value| (value / maximum * 72.).max(2.) as f32)
                                            .unwrap_or(2.);
                                        div()
                                            .flex_1()
                                            .h(px(height))
                                            .rounded_t(px(1.))
                                            .when(record.basis == "estimated", |this| {
                                                this.border_1()
                                                    .border_color(theme.primary.opacity(0.8))
                                                    .bg(theme.primary.opacity(0.15))
                                            })
                                            .when(record.basis != "estimated", |this| {
                                                this.bg(if value.is_some() { theme.primary } else { theme.border })
                                            })
                                    })),
                            )
                            .child(ds::caption(
                                "Oldest to newest. Outlined bars are estimates from token counts and local prices; filled bars are provider-reported.",
                                &theme,
                            )),
                    )
                    .child(
                        div()
                            .w(px(300.))
                            .flex_shrink_0()
                            .v_flex()
                            .children(records.iter().rev().take(4).map(|record| {
                                div()
                                    .h_flex()
                                    .gap(px(sp::MD))
                                    .min_h(px(24.))
                                    .child(
                                        ds::mono(record.native_model_id.clone(), ty::CAPTION, theme.muted_foreground, &theme)
                                            .flex_1()
                                            .truncate(),
                                    )
                                    .child(ds::mono(
                                        match &record.amount_usd {
                                            Some(value) => format!("${}", value.as_str()),
                                            None => "unknown".into(),
                                        },
                                        ty::SMALL,
                                        theme.foreground,
                                        &theme,
                                    ))
                            }))
                            .child(
                                ds::caption(
                                    report
                                        .map(|report| report.scope.clone())
                                        .unwrap_or_else(|| "No cost evidence yet. A turn needs a reported amount, or token counts and a matching local price.".into()),
                                    &theme,
                                )
                                .pt(px(sp::SM)),
                            ),
                    ),
            ))
            .child(
                div()
                    .v_flex()
                    .gap(px(sp::XS))
                    .border_t_1()
                    .border_color(theme.border)
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .child(
                        div()
                            .text_size(px(ty::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Recorded session tokens"),
                    )
                    .children(report.map(|report| {
                        if report.historical.providers.is_empty() {
                            return ds::caption(
                                "No readable Claude Code, Codex, Gemini, Grok, or Kiro session history.",
                                &theme,
                            )
                            .into_any_element();
                        }
                        div()
                            .v_flex()
                            .children(report.historical.providers.iter().map(|provider| {
                                div()
                                    .h_flex()
                                    .gap(px(sp::SM))
                                    .py(px(3.))
                                    .child(ds::mono(
                                        provider.provider_id.clone(),
                                        ty::CAPTION,
                                        theme.foreground,
                                        &theme,
                                    ))
                                    .child(div().flex_1())
                                    .child(ds::mono(
                                        format!("{} in", provider.totals.total_input_tokens()),
                                        ty::CAPTION,
                                        theme.muted_foreground,
                                        &theme,
                                    ))
                                    .child(ds::mono(
                                        format!("{} out", provider.totals.output_tokens),
                                        ty::CAPTION,
                                        theme.muted_foreground,
                                        &theme,
                                    ))
                                    .when(provider.totals.reasoning_tokens > 0, |this| {
                                        this.child(ds::mono(
                                            format!(
                                                "{} reasoning",
                                                provider.totals.reasoning_tokens
                                            ),
                                            ty::CAPTION,
                                            theme.muted_foreground,
                                            &theme,
                                        ))
                                    })
                                    .when(!provider.diagnostics.is_empty(), |this| {
                                        this.child(ds::mono(
                                            format!("{} diagnostics", provider.diagnostics.len()),
                                            ty::CAPTION,
                                            theme.muted_foreground,
                                            &theme,
                                        ))
                                    })
                                    .into_any_element()
                            }))
                            .into_any_element()
                    }))
                    .child(ds::caption(
                        "Fresh input plus cache reads/writes; reasoning is a subset of output. These are local transcript totals, not invoices.",
                        &theme,
                    )),
            )
            .into_any_element()
    }

    pub fn price_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::ds::{self, sp, ty};
        let theme = cx.theme().clone();
        let report = self.bundle.as_ref().and_then(|bundle| bundle.costs.clone());
        let editable = report.as_ref().is_some_and(|report| report.can_edit_prices);
        let prices = report
            .as_ref()
            .map(|report| report.prices.clone())
            .unwrap_or_default();
        const LABELS: [&str; 6] = [
            "Provider ID",
            "Exact model ID",
            "Input USD per million",
            "Output USD per million",
            "Reviewed on (YYYY-MM-DD)",
            "Source or local rationale",
        ];
        let status = if report.is_none() {
            "No cost evidence from this runtime, so prices cannot be read or edited."
        } else if !editable {
            "Read-only: editing prices needs the administrator credential."
        } else if report
            .as_ref()
            .is_some_and(|report| report.prices_persisted)
        {
            "Saved in the runtime's state folder."
        } else {
            "This runtime keeps prices in memory until it restarts."
        };
        let selected = self.price_edit_original.clone();

        ds::panel(&theme)
            .child(ds::panel_head(
                "Local model prices",
                Some(ds::caption("Advanced", &theme).into_any_element()),
                &theme,
            ))
            .child(
                div().px(px(sp::LG)).pt(px(sp::MD)).child(ds::body(
                    "USD per million tokens, for exact model IDs. Used only to estimate turn cost from token counts; estimates exclude caching, tools and discounts and are not subscription invoices.",
                    &theme,
                )),
            )
            .children(
                report
                    .as_ref()
                    .and_then(|report| report.storage_error.clone())
                    .or_else(|| self.bundle.as_ref().and_then(|bundle| bundle.cost_error.clone()))
                    .map(|error| {
                        div().px(px(sp::LG)).pt(px(sp::MD)).child(ds::banner(
                            crate::ui::Tone::Amber,
                            "Prices unavailable",
                            Some(error.into()),
                            &theme,
                        ))
                    }),
            )
            .child(
                div()
                    .mt(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        ds::table_head(&theme)
                            .child(ds::th("Provider / model", None, &theme))
                            .child(ds::th("Input", Some(90.), &theme))
                            .child(ds::th("Output", Some(90.), &theme))
                            .child(ds::th("Reviewed", Some(100.), &theme)),
                    )
                    .when(prices.is_empty(), |this| {
                        this.child(
                            div()
                                .px(px(sp::LG))
                                .py(px(sp::MD))
                                .child(ds::body("No local prices yet.", &theme)),
                        )
                    })
                    .children(prices.iter().enumerate().map(|(index, price)| {
                        let row_price = price.clone();
                        let is_selected = selected.as_ref() == Some(price);
                        ds::select_row(("edit-price", index), is_selected, &theme)
                            .min_h(px(36.))
                            .px(px(sp::LG))
                            .aria_label(format!(
                                "Edit price for {} {}",
                                price.provider_id, price.native_model_id
                            ))
                            .child(
                                ds::mono(
                                    format!("{} / {}", price.provider_id, price.native_model_id),
                                    ty::SMALL,
                                    theme.foreground,
                                    &theme,
                                )
                                .flex_1()
                                .min_w(px(0.))
                                .truncate(),
                            )
                            .child(div().w(px(90.)).child(ds::mono(
                                format!("${}", price.input_usd_per_million.as_str()),
                                ty::SMALL,
                                theme.foreground,
                                &theme,
                            )))
                            .child(div().w(px(90.)).child(ds::mono(
                                format!("${}", price.output_usd_per_million.as_str()),
                                ty::SMALL,
                                theme.foreground,
                                &theme,
                            )))
                            .child(div().w(px(100.)).child(ds::caption(price.reviewed_at.clone(), &theme)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if !editable {
                                    return;
                                }
                                this.price_edit_original = Some(row_price.clone());
                                let values = [
                                    row_price.provider_id.clone(),
                                    row_price.native_model_id.clone(),
                                    row_price.input_usd_per_million.as_str().into(),
                                    row_price.output_usd_per_million.as_str().into(),
                                    row_price.reviewed_at.clone(),
                                    row_price.source.clone(),
                                ];
                                for (field, value) in this.price_fields.iter().zip(values) {
                                    field.update(cx, |field, cx| field.set_value(value, window, cx));
                                }
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(sp::MD))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.border)
                    .children(LABELS.into_iter().enumerate().map(|(index, label)| {
                        div()
                            .v_flex()
                            .gap(px(sp::XS))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .text_color(theme.muted_foreground)
                                    .child(label),
                            )
                            .child(Input::new(&self.price_fields[index]).disabled(!editable))
                    })),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap(px(sp::SM))
                    .px(px(sp::LG))
                    .pb(px(sp::MD))
                    .child(
                        Button::new("save-price")
                            .primary()
                            .disabled(!editable)
                            .label(if selected.is_some() { "Save changes" } else { "Add price" })
                            .on_click(cx.listener(|this, _, _, cx| this.save_price(cx))),
                    )
                    .child(
                        Button::new("new-price")
                            .secondary()
                            .disabled(!editable)
                            .label("Clear form")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.price_edit_original = None;
                                for field in &this.price_fields {
                                    field.update(cx, |field, cx| field.set_value("", window, cx));
                                }
                                this.price_fields[4].update(cx, |field, cx| {
                                    field.set_value(
                                        chrono::Utc::now().format("%Y-%m-%d").to_string(),
                                        window,
                                        cx,
                                    )
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("remove-price")
                            .danger()
                            .ghost()
                            .disabled(!editable || selected.is_none())
                            .label("Remove selected")
                            .on_click(cx.listener(|this, _, _, cx| this.remove_price(cx))),
                    )
                    .child(div().flex_1())
                    .child(ds::caption(status, &theme)),
            )
            .into_any_element()
    }

    fn remove_price(&mut self, cx: &mut Context<Self>) {
        let Some(original) = self.price_edit_original.clone() else {
            return;
        };
        let Some(report) = self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.costs.as_ref())
        else {
            return;
        };
        let mut prices = report.prices.clone();
        let Some(index) = prices.iter().position(|price| *price == original) else {
            self.toast("The price table changed; select the row again");
            cx.notify();
            return;
        };
        prices.remove(index);
        if self
            .broker
            .commands
            .send(Command::SetPrices(prices))
            .is_err()
        {
            self.toast("The runtime worker is not running");
        } else {
            self.price_edit_original = None;
            self.toast("Removing price. The form keeps its values if you want to restore it.");
        }
        cx.notify();
    }

    fn save_price(&mut self, cx: &mut Context<Self>) {
        let values: Vec<_> = self
            .price_fields
            .iter()
            .map(|field| field.read(cx).value().to_string())
            .collect();
        let parsed = serde_json::from_value::<originlm_contracts::ModelPrice>(serde_json::json!({
            "providerId": values[0],
            "nativeModelId": values[1],
            "inputUsdPerMillion": values[2],
            "outputUsdPerMillion": values[3],
            "reviewedAt": values[4],
            "source": values[5],
        }));
        let Ok(price) = parsed else {
            self.toast("Prices must be nonnegative plain decimals, like 3 or 0.25");
            cx.notify();
            return;
        };
        if price.provider_id.is_empty()
            || price.native_model_id.is_empty()
            || price.source.is_empty()
            || chrono::NaiveDate::parse_from_str(&price.reviewed_at, "%Y-%m-%d").is_err()
        {
            self.toast("Enter provider, model, source and a review date (YYYY-MM-DD)");
            cx.notify();
            return;
        }
        let Some(report) = self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.costs.as_ref())
        else {
            return;
        };
        let mut prices = report.prices.clone();
        if let Some(original) = &self.price_edit_original {
            if let Some(current) = prices.iter_mut().find(|current| *current == original) {
                *current = price;
            } else {
                self.toast("The price table changed; select the row again");
                cx.notify();
                return;
            }
        } else {
            prices.push(price);
        }
        if self
            .broker
            .commands
            .send(Command::SetPrices(prices))
            .is_err()
        {
            self.toast("The runtime worker is not running");
        } else {
            self.toast("Saving prices. The table updates when the runtime confirms.");
        }
        cx.notify();
    }
}
