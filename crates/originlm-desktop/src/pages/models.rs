//! Models: the provider-native catalog across every account, with an
//! inspector for route, billing, capabilities and options.

use crate::app::{Desktop, Page};
use crate::data;
use crate::ds::{self, sp, ty};
use crate::pages::providers::capability_name;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName, Theme,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::prelude::FluentBuilder as _;
#[cfg(test)]
use gpui_kit::test::TestSupportExt as _;
use gpui_kit::*;
use originlm_contracts::{Model, OptionControl, Snapshot, Support};

impl Desktop {
    pub fn page_models(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.snapshot().cloned() else {
            return div().into_any_element();
        };
        let entity = cx.entity();

        if snapshot.models.is_empty() {
            return ds::page()
                .child(ds::panel(&theme).child(ds::state(
                    ds::StateKind::Empty,
                    "No model catalog reported",
                    "None of the discovered clients reported a model list. Some clients only expose models after sign-in; check Accounts for sign-in state.",
                    &theme,
                ).child(div().pt(px(sp::SM)).child(ds::link("models-accounts", "Open Accounts", &theme, {
                    let entity = entity.clone();
                    move |_, _, cx| entity.update(cx, |this, cx| this.set_page(Page::Providers, cx))
                })))))
                .into_any_element();
        }

        let query = self.query.to_lowercase();
        let favorites_only = self.favorites_only;
        let models: Vec<&Model> = snapshot
            .models
            .iter()
            .filter(|model| {
                query.is_empty()
                    || model.name.to_lowercase().contains(&query)
                    || model.native_id.to_lowercase().contains(&query)
            })
            .filter(|model| !favorites_only || self.favorites.contains(&model.id))
            .collect();
        let selected = self
            .selected_model
            .clone()
            .filter(|id| models.iter().any(|m| &m.id == id))
            .or_else(|| models.first().map(|model| model.id.clone()));

        let filter = ds::tabs(
            "models-filter",
            [
                format!("All {}", snapshot.models.len()),
                format!("Favorites {}", self.favorites.len()),
            ],
            usize::from(favorites_only),
            &theme,
            {
                let entity = entity.clone();
                move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.favorites_only = index == 1;
                        cx.notify();
                    })
                }
            },
        );

        let rows: Vec<AnyElement> = models
            .iter()
            .take(300)
            .map(|model| self.model_row(&snapshot, model, selected.as_deref(), &theme, cx))
            .collect();
        let list_body: AnyElement = if rows.is_empty() {
            ds::state(
                ds::StateKind::Empty,
                if favorites_only && query.is_empty() {
                    "No favorites yet".to_string()
                } else {
                    format!("No models match \u{201c}{}\u{201d}", self.query)
                },
                if favorites_only && query.is_empty() {
                    "Use the star on any model to keep it here."
                } else {
                    "Search matches model names and native IDs. Clear the search or switch to All."
                },
                &theme,
            )
            .into_any_element()
        } else {
            div()
                .v_flex()
                .py(px(sp::XS))
                .children(rows)
                .into_any_element()
        };

        let list = ds::panel(&theme)
            .flex_1()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::MD))
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().flex_1().min_w(px(0.)).child(Input::new(&self.search)))
                    .child(filter),
            )
            .child(
                ds::table_head(&theme)
                    .px(px(sp::MD + 2.))
                    .child(div().w(px(32.)).flex_shrink_0())
                    .child(ds::th("Model", None, &theme))
                    .child(ds::th("Billing", Some(130.), &theme))
                    .child(ds::th("Options", Some(64.), &theme)),
            )
            .child(list_body);

        let inspector = selected
            .as_ref()
            .and_then(|id| snapshot.models.iter().find(|m| &m.id == id))
            .map(|model| self.model_inspector(&snapshot, model, &theme, cx))
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
                            .w(px(self.models_inspector.clamp(360., 460.)))
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

    fn model_row(
        &self,
        snapshot: &Snapshot,
        model: &Model,
        selected: Option<&str>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let route = data::route_for_model(snapshot, model);
        let connection = route.and_then(|r| data::connection_for_route(snapshot, r));
        let provider = connection
            .map(|c| data::provider_name(snapshot, &c.provider_id))
            .unwrap_or_else(|| "Unknown provider".into());
        let account = connection
            .map(|c| crate::human::without_provider_prefix(&c.display_name, &provider))
            .unwrap_or_default();
        let favorite = self.favorites.contains(&model.id);
        let entity = cx.entity();
        let star_entity = cx.entity();
        let id = model.id.clone();
        let star_id = model.id.clone();
        let row = ds::select_row(
            SharedString::from(format!("model-{}", model.id)),
            selected == Some(model.id.as_str()),
            theme,
        )
        .aria_label(match data::model_plan(snapshot, model) {
            Some(plan) => format!("{}, {provider}, {}", model.name, plan.label()),
            None => format!("{}, {provider}", model.name),
        })
        .child(ds::icon_button(
            SharedString::from(format!("star-{}", model.id)),
            if favorite {
                IconName::StarFill
            } else {
                IconName::Star
            },
            if favorite {
                format!("Remove {} from favorites", model.name)
            } else {
                format!("Add {} to favorites", model.name)
            },
            if favorite {
                theme.primary
            } else {
                theme.muted_foreground
            },
            theme,
            move |_, _, cx| {
                cx.stop_propagation();
                star_entity.update(cx, |this, cx| {
                    if !this.favorites.remove(&star_id) {
                        this.favorites.insert(star_id.clone());
                    }
                    this.save_prefs();
                    cx.notify();
                });
            },
        ))
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .h_flex()
                        .gap(px(sp::SM))
                        .min_w(px(0.))
                        .child(
                            div()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(ty::BODY))
                                .font_weight(FontWeight::MEDIUM)
                                .child(model.name.clone()),
                        )
                        .when(model.is_default, |this| {
                            this.child(ds::chip("Default", Tone::Neutral, theme))
                        })
                        .when_some(data::model_plan(snapshot, model), |this, plan| {
                            this.child(ds::chip(plan.label(), Tone::Neutral, theme))
                        }),
                )
                .child(
                    div()
                        .h_flex()
                        .gap(px(6.))
                        .min_w(px(0.))
                        .children(route.and_then(|route| {
                            crate::icons::provider_mark(
                                &route.provider_id,
                                12.,
                                theme.muted_foreground,
                            )
                        }))
                        .child(
                            ds::caption(
                                if account.is_empty() {
                                    provider.clone()
                                } else {
                                    format!("{provider} · {account}")
                                },
                                theme,
                            )
                            .truncate(),
                        ),
                ),
        )
        .child(div().h_flex().w(px(130.)).flex_shrink_0().child(ds::chip(
            route.map(ui::billing_label).unwrap_or("billing unknown"),
            Tone::Neutral,
            theme,
        )))
        .child(div().w(px(64.)).flex_shrink_0().child(ds::mono(
            model.options.len().to_string(),
            ty::SMALL,
            theme.foreground,
            theme,
        )))
        .on_click(move |_, _, cx| {
            entity.update(cx, |this, cx| {
                this.selected_model = Some(id.clone());
                cx.notify();
            });
        });
        #[cfg(test)]
        let row = row.test_support();
        row.into_any_element()
    }

    fn model_inspector(
        &self,
        snapshot: &Snapshot,
        model: &Model,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let route = data::route_for_model(snapshot, model);
        let connection = route.and_then(|r| data::connection_for_route(snapshot, r));
        let provider = connection
            .map(|c| data::provider_name(snapshot, &c.provider_id))
            .unwrap_or_else(|| "Unknown provider".into());
        let caps: Vec<_> = model
            .capabilities
            .iter()
            .filter(|cap| matches!(cap.support, Support::Supported))
            .collect();
        // Cursor routes are metadata only; every other route has a text driver.
        let executable = route.is_some_and(|route| route.provider_id.as_str() != "cursor");
        let entity = cx.entity();
        let choice_id = model.id.clone();

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
                        route.map(|r| r.provider_id.as_str()).unwrap_or(""),
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
                                    .child(model.name.clone()),
                            )
                            .child(
                                ds::caption(
                                    connection
                                        .map(|c| data::connection_title(snapshot, c))
                                        .unwrap_or_else(|| format!("{provider}, no connection")),
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
                    .child(ds::chip(
                        route.map(ui::billing_label).unwrap_or("billing unknown"),
                        Tone::Neutral,
                        theme,
                    ))
                    .when(model.is_default, |this| {
                        this.child(ds::chip("Provider default", Tone::Neutral, theme))
                    })
                    .when_some(data::model_plan(snapshot, model), |this, plan| {
                        this.child(ds::chip(plan.label(), Tone::Neutral, theme))
                    }),
            )
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::kv(
                        "Native ID",
                        ds::copy_value("model-native", model.native_id.clone(), 30, theme),
                        theme,
                    ))
                    .child(ds::kv(
                        "Route ID",
                        ds::copy_value("model-route", model.route_id.clone(), 30, theme),
                        theme,
                    ))
                    .child(ds::kv(
                        "Capabilities",
                        div()
                            .h_flex()
                            .flex_wrap()
                            .gap(px(sp::XS))
                            .py(px(sp::XS))
                            .when(caps.is_empty(), |this| {
                                this.child(ds::caption("None reported as supported", theme))
                            })
                            .children(caps.iter().map(|cap| {
                                ds::chip(capability_name(&cap.id), Tone::Neutral, theme)
                            })),
                        theme,
                    )),
            )
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
                            .child(format!("Provider options ({})", model.options.len())),
                    ),
            )
            .when(model.options.is_empty(), |this| {
                this.child(div().px(px(sp::LG)).pb(px(sp::MD)).child(ds::caption(
                    "This model advertises no adjustable options.",
                    theme,
                )))
            })
            .children(model.options.iter().enumerate().map(|(index, option)| {
                let control = match &option.control {
                    OptionControl::Select { choices, .. } => {
                        format!("select · {}", ui::plural(choices.len(), "choice"))
                    }
                    OptionControl::Boolean => "on / off".into(),
                    OptionControl::Number { min, max, step } => {
                        format!("{min} to {max}, step {step}")
                    }
                    OptionControl::Text { max_length } => format!("text · {max_length} chars"),
                    OptionControl::Unknown { native_type } => format!("native {native_type}"),
                };
                let detail = match &option.control {
                    OptionControl::Select { choices, .. } => Some(
                        choices
                            .iter()
                            .take(6)
                            .map(|choice| choice.label.clone())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    _ => None,
                };
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
                                    .h_flex()
                                    .gap(px(sp::SM))
                                    .child(
                                        div()
                                            .text_size(px(ty::SMALL))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(option.label.clone()),
                                    )
                                    .when(option.billing_sensitive, |this| {
                                        this.child(ds::chip("Affects billing", Tone::Amber, theme))
                                    }),
                            )
                            .when_some(detail, |this, detail| {
                                this.child(ds::caption(detail, theme))
                            })
                            .child(
                                ds::mono(
                                    match option.current_value.as_ref() {
                                        Some(value) => format!("{} = {value}", option.id),
                                        None => option.id.clone(),
                                    },
                                    ty::CAPTION,
                                    theme.muted_foreground,
                                    theme,
                                )
                                .truncate(),
                            ),
                    )
                    .child(ds::caption(control, theme))
            }))
            .when(
                self.bundle.as_ref().is_some_and(|bundle| {
                    bundle
                        .local_models
                        .iter()
                        .any(|inventory| !inventory.models.is_empty())
                }),
                |this| {
                    this.child(
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
                                    .child("On-disk models"),
                            )
                            .children(
                                self.bundle
                                    .as_ref()
                                    .into_iter()
                                    .flat_map(|bundle| bundle.local_models.iter())
                                    .filter(|inventory| !inventory.models.is_empty())
                                    .map(|inventory| {
                                        div()
                                            .v_flex()
                                            .gap(px(2.))
                                            .child(ds::mono(
                                                format!(
                                                    "{} · {}",
                                                    inventory.provider_id,
                                                    inventory.models.len()
                                                ),
                                                ty::CAPTION,
                                                theme.foreground,
                                                theme,
                                            ))
                                            .children(inventory.models.iter().take(8).map(
                                                |model| {
                                                    ds::caption(model.native_id.clone(), theme)
                                                        .into_any_element()
                                                },
                                            ))
                                            .into_any_element()
                                    }),
                            ),
                    )
                },
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::SM))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.border)
                    .when(executable, |this| {
                        this.child(
                            Button::new("use-model")
                                .primary()
                                .label("Try in Playground")
                                .icon(IconName::Play)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.selected_model = Some(choice_id.clone());
                                        this.set_page(Page::Playground, cx);
                                    });
                                }),
                        )
                    })
                    .when(!executable, |this| {
                        this.child(ds::caption(
                            "Metadata only: OriginLM has no text driver for Cursor routes yet.",
                            theme,
                        ))
                    }),
            )
            .into_any_element()
    }
}
