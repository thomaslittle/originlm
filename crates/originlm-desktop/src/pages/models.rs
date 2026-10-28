use crate::app::{Desktop, Page};
use crate::data;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_models(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(snapshot) = self.snapshot().cloned() else {
            return div()
                .px(px(ui::PAGE_PAD))
                .child(ui::empty_state(
                    IconName::Cpu,
                    "Waiting for models",
                    "The broker has not published a catalog yet.",
                    &theme,
                ))
                .into_any_element();
        };

        let query = self.query.to_lowercase();
        let favorites_only = false;
        let models: Vec<_> = snapshot
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
            .or_else(|| models.first().map(|model| model.id.clone()));

        let list: Vec<AnyElement> = models
            .iter()
            .take(200)
            .map(|model| {
                let route = data::route_for_model(&snapshot, model);
                let connection = route.and_then(|r| data::connection_for_route(&snapshot, r));
                let provider = connection
                    .map(|c| data::provider_name(&snapshot, &c.provider_id))
                    .unwrap_or_else(|| "unknown".into());
                let provider_id = route
                    .map(|route| route.provider_id.clone())
                    .unwrap_or_default();
                let is_selected = selected.as_deref() == Some(model.id.as_str());
                let favorite = self.favorites.contains(&model.id);
                let entity = cx.entity();
                let entity_star = cx.entity();
                let id = model.id.clone();
                let star_id = model.id.clone();

                div()
                    .id(SharedString::from(format!("model-{id}")))
                    .h_flex()
                    .gap_3()
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
                            .id(SharedString::from(format!("star-{star_id}")))
                            .cursor_pointer()
                            .text_color(if favorite {
                                theme.primary
                            } else {
                                theme.muted_foreground.opacity(0.6)
                            })
                            .child(ui::icon(
                                if favorite {
                                    IconName::StarFill
                                } else {
                                    IconName::Star
                                },
                                12.,
                                if favorite {
                                    theme.primary
                                } else {
                                    theme.muted_foreground.opacity(0.6)
                                },
                            ))
                            .on_click(move |_, _, cx| {
                                entity_star.update(cx, |this, cx| {
                                    if !this.favorites.remove(&star_id) {
                                        this.favorites.insert(star_id.clone());
                                    }
                                    this.save_prefs();
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .min_w(px(0.))
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(model.name.clone()),
                                    )
                                    .when(model.is_default, |this| {
                                        this.child(ui::chip("default", Tone::Amber, &theme))
                                    }),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .children(crate::icons::provider_mark(
                                        &provider_id,
                                        14.,
                                        theme.muted_foreground,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(theme.muted_foreground)
                                            .child(provider.clone()),
                                    )
                                    .when_some(
                                        connection.map(|c| {
                                            crate::human::without_provider_prefix(
                                                &c.display_name,
                                                &provider,
                                            )
                                        }),
                                        |this, profile| this.child(ui::mono(profile, &theme)),
                                    )
                                    .when_some(
                                        route
                                            .map(|r| data::billing(r).0)
                                            .filter(|billing| !billing.starts_with("unknown")),
                                        |this, billing| {
                                            this.child(ui::chip(billing, Tone::Neutral, &theme))
                                        },
                                    )
                                    .child(div().flex_1())
                                    .child(ui::mono(
                                        ui::plural(model.options.len(), "option"),
                                        &theme,
                                    )),
                            ),
                    )
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.selected_model = Some(id.clone());
                            cx.notify();
                        });
                    })
                    .into_any_element()
            })
            .collect();

        let detail: AnyElement = selected
            .as_ref()
            .and_then(|id| snapshot.models.iter().find(|m| &m.id == id))
            .map(|model| {
                let route = data::route_for_model(&snapshot, model);
                let connection = route.and_then(|r| data::connection_for_route(&snapshot, r));
                let provider = connection
                    .map(|c| data::provider_name(&snapshot, &c.provider_id))
                    .unwrap_or_else(|| "unknown".into());
                let (billing, fresh) = route.map(data::billing).unwrap_or(("unknown".into(), false));
                let caps: Vec<_> = model
                    .capabilities
                    .iter()
                    .filter(|cap| matches!(cap.support, originlm_contracts::Support::Supported))
                    .collect();

                let entity = cx.entity();
                let choice_id = model.id.clone();
                let provider_id = route
                    .map(|route| route.provider_id.clone())
                    .unwrap_or_default();
                // Metadata-only providers are catalogued but never offered as a
                // runnable Playground route.
                let executable = route.is_some_and(|route| {
                    !matches!(
                        route.provider_id.as_str(),
                        "opencode" | "opencode-go" | "cursor"
                    )
                });

                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .child(crate::icons::provider_tile(
                                route.map(|r| r.provider_id.as_str()).unwrap_or(""),
                                &provider,
                                44.,
                                &theme,
                            ))
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .min_w(px(0.))
                                    .child(
                                        div()
                                            .text_size(px(17.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(model.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(theme.muted_foreground)
                                            .child(
                                                connection
                                                    .map(|c| data::connection_title(&snapshot, c))
                                                    .unwrap_or_else(|| {
                                                        format!("{provider}, no connection")
                                                    }),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(ui::chip(
                                if fresh {
                                    billing.to_string()
                                } else {
                                    format!("{billing}, stale")
                                },
                                Tone::Neutral,
                                &theme,
                            ))
                            .when(model.is_default, |this| {
                                this.child(ui::chip("provider default", Tone::Amber, &theme))
                            })
                            .child(ui::chip(
                                ui::plural(model.options.len(), "option"),
                                Tone::Neutral,
                                &theme,
                            )),
                    )
                    .child(ui::divider(&theme))
                    .child(copyable("Native ID", "native", &model.native_id, &theme))
                    .child(copyable("Route", "route", &model.route_id, &theme))
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(ui::eyebrow("Capabilities", &theme))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_1()
                                    .flex_wrap()
                                    .when(caps.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(theme.muted_foreground)
                                                .child("No supported capability reported."),
                                        )
                                    })
                                    .children(caps.into_iter().map(|cap| {
                                        let label = data::capability_label(&cap.id);
                                        ui::chip(
                                            if label.is_empty() {
                                                cap.id.clone()
                                            } else {
                                                label.to_string()
                                            },
                                            Tone::Blue,
                                            &theme,
                                        )
                                        .into_any_element()
                                    })),
                            ),
                    )
                    .child(ui::section_title("Provider options", &theme))
                    .when(model.options.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.muted_foreground)
                                .child("This model advertises no adjustable options."),
                        )
                    })
                    .children(model.options.iter().map(|option| {
                        let control = match &option.control {
                            originlm_contracts::OptionControl::Select { choices, .. } => {
                                format!("select, {} choices", choices.len())
                            }
                            originlm_contracts::OptionControl::Boolean => "boolean".into(),
                            originlm_contracts::OptionControl::Number { min, max, step } => {
                                format!("{min}–{max}, step {step}")
                            }
                            originlm_contracts::OptionControl::Text { max_length } => {
                                format!("{max_length} characters")
                            }
                            originlm_contracts::OptionControl::Unknown { native_type } => {
                                format!("native {native_type}")
                            }
                        };
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .py_1()
                            .border_b_1()
                            .border_color(theme.border.opacity(0.5))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(div().text_size(px(11.5)).child(option.label.clone()))
                                    .when(option.billing_sensitive, |this| {
                                        this.child(ui::chip("billing", Tone::Amber, &theme))
                                    })
                                    .child(div().flex_1())
                                    .child(ui::mono(control, &theme)),
                            )
                            .when_some(
                                match &option.control {
                                    originlm_contracts::OptionControl::Select { choices, .. } => Some(
                                        choices
                                            .iter()
                                            .take(7)
                                            .map(|choice| choice.label.clone())
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                    ),
                                    _ => None,
                                },
                                |this, line| this.child(ui::mono(line, &theme)),
                            )
                            .when_some(
                                option.current_value.as_ref().map(|v| v.to_string()),
                                |this, value| {
                                    this.child(ui::mono(format!("current {value}"), &theme))
                                },
                            )
                            .child(ui::mono(format!("id {}", option.id), &theme))
                            .into_any_element()
                    }))
                    .when(executable, |this| {
                        this.child(
                            Button::new("use-model")
                                .primary()
                                .label("Open in Playground")
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.selected_model = Some(choice_id.clone());
                                        this.set_page(Page::Playground, cx);
                                    });
                                }),
                        )
                    })
                    .when(!executable, |this| {
                        this.child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.muted_foreground)
                                .child(if provider_id == "cursor" {
                                    "Metadata is available for this Cursor route; text execution is not wired yet."
                                } else {
                                    "Metadata is available for this OpenCode 2 route; text execution is not wired yet."
                                }),
                        )
                    })
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                ui::empty_state(
                    IconName::Cpu,
                    "Select a model",
                    "Choose a model to see its route, billing and provider-native options.",
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
                    .min_w(px(0.))
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.search)))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_1()
                                    .px_2()
                                    .py(px(5.))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_size(px(10.5))
                                    .text_color(theme.muted_foreground)
                                    .child(ui::icon(IconName::Star, 11., theme.primary))
                                    .child(format!("{} favorites", self.favorites.len())),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .p_2()
                            .rounded(px(ui::CARD_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.colors.list)
                            .when(list.is_empty(), |this| {
                                this.child(ui::empty_state(
                                    IconName::Search,
                                    "No matching models",
                                    "Try another name, native ID or clear the search.",
                                    &theme,
                                ))
                            })
                            .children(list),
                    ),
            )
            .child(
                div()
                    .w(px(self.models_inspector))
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

fn copyable(
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
                        .id(SharedString::from(format!("copy-model-{id}")))
                        .font_family(theme.mono_font_family.clone())
                        .text_size(px(10.5))
                        .text_color(theme.foreground)
                        .cursor_pointer()
                        .child(ui::short(value, 34))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()));
                        }),
                )
                .child(ui::icon(IconName::Copy, 10., theme.muted_foreground)),
        )
        .into_any_element()
}
