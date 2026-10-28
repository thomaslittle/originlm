use crate::{app::Desktop, broker::Command, ui};
use gpui_kit::base::{Disableable as _, StyledExt as _};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::*;

impl Desktop {
    pub fn cost_trend(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let report = self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.costs.as_ref());
        let records: Vec<_> = report
            .map(|report| report.records.iter().rev().take(24).rev().collect())
            .unwrap_or_default();
        let maximum = records
            .iter()
            .filter_map(|record| record.amount_usd.as_ref()?.as_str().parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .fold(0.000001_f64, f64::max);
        let maximum_label = if maximum >= 1_000_000. {
            format!("{maximum:.2e}")
        } else {
            maximum.to_string()
        };
        div().v_flex().gap_3().p_4().border_1().border_color(theme.border).rounded(px(ui::CARD_RADIUS))
            .child(ui::section_title(if self.example { "Cost trend — example data" } else { "Cost trend" }, &theme))
            .children(self.bundle.as_ref().and_then(|bundle| bundle.cost_error.clone()).map(|error| div().text_size(px(12.)).child(error)))
            .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(format!("Scale: $0 to ${maximum_label} per turn")))
            .child(div().h_flex().items_end().gap_2().h(px(100.)).children(records.iter().map(|record| {
                let amount = record.amount_usd.as_ref().and_then(|value| value.as_str().parse::<f64>().ok()).filter(|value| value.is_finite());
                div().flex_1().h(px(amount.map(|value| (value / maximum * 100.).max(1.) as f32).unwrap_or(1.)))
                    .border_1().border_color(theme.primary).bg(if record.basis == "estimated" { theme.primary.opacity(0.2) } else if amount.is_some() { theme.primary } else { theme.border })
            })))
            .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child(format!("Latest {} observed turns, oldest to newest. Latest four amounts below, newest first.", records.len())))
            .children(records.iter().rev().take(4).map(|record| div().h_flex().justify_between().gap_3().text_size(px(12.))
                .child(record.native_model_id.clone())
                .child(match &record.amount_usd { Some(value) => format!("${} {}", value.as_str(), if record.basis == "estimated" { "estimated" } else { "provider reported" }), None => "Unknown: usage or price missing".into() })))
            .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child(report.map(|report| report.scope.clone()).unwrap_or_else(|| "No cost evidence. A turn needs reported money, or token counts and a matching local price.".into())))
            .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("Latest observed turns. Outlined bars are estimates; gaps are unknown. Edit prices in Settings."))
            .into_any_element()
    }

    pub fn price_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let report = self.bundle.as_ref().and_then(|bundle| bundle.costs.clone());
        let editable = report.as_ref().is_some_and(|report| report.can_edit_prices);
        let prices = report
            .as_ref()
            .map(|report| report.prices.clone())
            .unwrap_or_default();
        div().v_flex().gap_3().p_4().border_1().border_color(theme.border).rounded(px(ui::CARD_RADIUS))
            .child(ui::section_title("Local model prices", &theme))
            .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child("USD per million tokens. Exact model IDs only. Estimates exclude caching, tools and discounts; they are not subscription invoices."))
            .children(report.as_ref().and_then(|report| report.storage_error.clone()).map(|error| div().text_size(px(12.)).child(error)))
            .children(self.bundle.as_ref().and_then(|bundle| bundle.cost_error.clone()).map(|error| div().text_size(px(12.)).child(error)))
            .children(prices.iter().enumerate().map(|(index, price)| {
                let price = price.clone();
                Button::new(("edit-price", index)).secondary().disabled(!editable)
                    .label(format!("{} / {}   ${} input   ${} output", price.provider_id, price.native_model_id, price.input_usd_per_million.as_str(), price.output_usd_per_million.as_str()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.price_edit_original = Some(price.clone());
                        let values = [price.provider_id.clone(), price.native_model_id.clone(), price.input_usd_per_million.as_str().into(), price.output_usd_per_million.as_str().into(), price.reviewed_at.clone(), price.source.clone()];
                        for (field, value) in this.price_fields.iter().zip(values) { field.update(cx, |field, cx| field.set_value(value, window, cx)); }
                        cx.notify();
                    }))
            }))
            .children(["Provider ID", "Exact model ID", "Input USD per million", "Output USD per million", "Reviewed on (YYYY-MM-DD)", "Source or local rationale"].into_iter().enumerate().map(|(index, label)| div().v_flex().gap_1().child(div().text_size(px(12.)).child(label)).child(Input::new(&self.price_fields[index]).disabled(!editable))))
            .child(div().h_flex().gap_3()
                .child(Button::new("new-price").secondary().disabled(!editable).label("New model price").on_click(cx.listener(|this, _, window, cx| {
                    this.price_edit_original = None;
                    for field in &this.price_fields { field.update(cx, |field, cx| field.set_value("", window, cx)); }
                    this.price_fields[4].update(cx, |field, cx| field.set_value(chrono::Utc::now().format("%Y-%m-%d").to_string(), window, cx));
                    cx.notify();
                })))
                .child(Button::new("remove-price").secondary().disabled(!editable || self.price_edit_original.is_none()).label("Remove selected price").on_click(cx.listener(|this, _, _, cx| {
                    let Some(original) = &this.price_edit_original else { return; };
                    let Some(report) = this.bundle.as_ref().and_then(|bundle| bundle.costs.as_ref()) else { return; };
                    let mut prices = report.prices.clone();
                    let Some(index) = prices.iter().position(|price| price == original) else { this.toast("Price table changed; select the row again"); cx.notify(); return; };
                    prices.remove(index);
                    if this.broker.commands.send(Command::SetPrices(prices)).is_err() { this.toast("Runtime worker is unavailable"); }
                    else { this.price_edit_original = None; this.toast("Removing price. Save the retained fields to restore it."); }
                    cx.notify();
                })))
                .child(Button::new("save-price").primary().disabled(!editable).label("Save price").on_click(cx.listener(|this, _, _, cx| {
                    let values: Vec<_> = this.price_fields.iter().map(|field| field.read(cx).value().to_string()).collect();
                    let parsed = serde_json::from_value::<originlm_contracts::ModelPrice>(serde_json::json!({"providerId":values[0],"nativeModelId":values[1],"inputUsdPerMillion":values[2],"outputUsdPerMillion":values[3],"reviewedAt":values[4],"source":values[5]}));
                    let Ok(price) = parsed else { this.toast("Use nonnegative plain decimal prices"); cx.notify(); return; };
                    if price.provider_id.is_empty() || price.native_model_id.is_empty() || price.source.is_empty() || chrono::NaiveDate::parse_from_str(&price.reviewed_at, "%Y-%m-%d").is_err() { this.toast("Enter provider, model, source and a valid review date"); cx.notify(); return; }
                    let Some(report) = this.bundle.as_ref().and_then(|bundle| bundle.costs.as_ref()) else { return; };
                    let mut prices = report.prices.clone();
                    if let Some(original) = &this.price_edit_original { if let Some(current) = prices.iter_mut().find(|current| *current == original) { *current = price; } else { this.toast("Price table changed; select the row again"); cx.notify(); return; } } else { prices.push(price); }
                    if this.broker.commands.send(Command::SetPrices(prices)).is_err() { this.toast("Runtime worker is unavailable"); } else { this.toast("Saving prices; refresh result appears here"); }
                    cx.notify();
                }))))
            .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child(if !editable { "Administrator access is required to edit prices." } else if report.as_ref().is_some_and(|report| report.prices_persisted) { "Saved locally in the runtime state directory." } else { "This host keeps prices in memory until it restarts." }))
            .into_any_element()
    }
}
