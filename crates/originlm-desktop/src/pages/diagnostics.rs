use crate::app::Desktop;
use crate::data;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl Desktop {
    pub fn page_diagnostics(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(bundle) = self.bundle.as_ref() else {
            return div()
                .px(px(ui::PAGE_PAD))
                .child(ui::empty_state(
                    IconName::CircleAlert,
                    "Nothing to diagnose yet",
                    "Diagnostics appear after the first broker snapshot.",
                    &theme,
                ))
                .into_any_element();
        };
        let snapshot = bundle.snapshot.clone();
        let issues: Vec<_> = data::open_issues(&snapshot).into_iter().cloned().collect();
        let health = snapshot.health.clone();
        let hydrated = health.hydrated;
        let checked = health
            .checked_at
            .as_ref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t.as_str()).ok())
            .map(|stamp| {
                stamp
                    .with_timezone(&chrono::Local)
                    .format("%b %d, %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "not checked".into());
        let entity_copy = cx.entity();
        let revision = snapshot.revision.clone();
        let export_snapshot = snapshot.clone();
        let export_runtime = bundle.runtime.clone();
        let export_apps = bundle.apps.clone();
        let export_adapters = bundle.adapters.clone();
        let entity_export = cx.entity();

        div()
            .v_flex()
            .w_full()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .gap_4()
            .child(
                div()
                    .h_flex()
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
                            .flex_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(ui::dot(
                                        match health.status {
                                            originlm_contracts::HealthStatus::Healthy => Tone::Green,
                                            originlm_contracts::HealthStatus::Degraded => Tone::Amber,
                                            originlm_contracts::HealthStatus::Unavailable => Tone::Red,
                                            _ => Tone::Neutral,
                                        },
                                        &theme,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(format!("{:?}", health.status)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "{} open, checked {checked}{}",
                                        health.open_issues,
                                        if hydrated {
                                            " (hydrated from cache, reverification pending)"
                                        } else {
                                            ""
                                        }
                                    )),
                            ),
                    )
                    .child(
                        Button::new("copy-revision")
                            .secondary()
                            .label("Copy revision")
                            .icon(IconName::Copy)
                            .on_click(move |_, _, cx| {
                                let revision = revision.clone();
                                entity_copy.update(cx, |this, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        revision,
                                    ));
                                    this.toast("Revision copied");
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new("export")
                            .secondary()
                            .label("Export redacted report")
                            .icon(IconName::ExternalLink)
                            .on_click(move |_, _, cx| {
                                let document = originlm_control::diagnostic_document(
                                    &export_snapshot,
                                    &export_runtime,
                                    &export_apps,
                                    &export_adapters,
                                );
                                let path = originlm_control::default_export_path();
                                let result = originlm_control::write_export(&path, &document);
                                entity_export.update(cx, |this, cx| {
                                    match result {
                                        Ok(()) => this.toast(format!(
                                            "Redacted report written to {}",
                                            path.display()
                                        )),
                                        Err(error) => this.toast(format!("Export failed: {error}")),
                                    }
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(ui::section_title("Open issues", &theme))
                    .when(issues.is_empty(), |this| {
                        this.child(
                            div()
                                .h_flex()
                                .gap_2()
                                .p_4()
                                .rounded(px(ui::CARD_RADIUS))
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.colors.list)
                                .child(ui::icon(IconName::CircleCheck, 14., theme.success))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(theme.muted_foreground)
                                        .child("The broker reports no open issues."),
                                ),
                        )
                    })
                    .children(issues.iter().map(|issue| {
                        let cached = issue.connection_id.as_ref().and_then(|id| {
                            snapshot.connections.iter().find(|connection| {
                                &connection.id == id
                                    && data::auth_view(connection).kind == data::AuthKind::Cached
                            })
                        });
                        let provider = cached.map(|connection| {
                            data::provider_name(&snapshot, &connection.provider_id)
                        });
                        let tone = match issue.severity {
                            originlm_contracts::IssueSeverity::Critical
                            | originlm_contracts::IssueSeverity::Error => Tone::Red,
                            originlm_contracts::IssueSeverity::Warning => Tone::Amber,
                            _ => Tone::Neutral,
                        };
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
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(issue.title.clone()),
                                    )
                                    .child(ui::chip(
                                        data::severity_label(issue.severity),
                                        tone,
                                        &theme,
                                    ))
                                    .child(div().flex_1())
                                    .child(ui::mono(
                                        format!("seen {}×", issue.occurrences),
                                        &theme,
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .child(match &provider {
                                        Some(provider) => format!(
                                            "The broker found {provider}'s local account cache but cannot verify the session. Its usage and model catalog stay inside the {provider} client."
                                        ),
                                        None => issue.summary.clone(),
                                    }),
                            )
                            .when(!issue.evidence.is_empty(), |this| {
                                this.child(
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .p_2()
                                        .rounded(px(7.))
                                        .bg(theme.muted)
                                        .children(issue.evidence.iter().map(|evidence| {
                                            div()
                                                .h_flex()
                                                .gap_2()
                                                .child(ui::mono(evidence.code.clone(), &theme))
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(theme.muted_foreground)
                                                        .child(evidence.message.clone()),
                                                )
                                                .into_any_element()
                                        })),
                                )
                            })
                            .when(!issue.remediation.is_empty(), |this| {
                                this.child(
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(ui::eyebrow("What to do", &theme))
                                        .children(issue.remediation.iter().map(|step| {
                                            div()
                                                .h_flex()
                                                .gap_2()
                                                .child(ui::icon(
                                                    IconName::ArrowRight,
                                                    10.,
                                                    theme.primary,
                                                ))
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(theme.foreground)
                                                        .child(step.description.clone()),
                                                )
                                                .into_any_element()
                                        })),
                                )
                            })
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
                    .child(ui::section_title("Broker diagnostics", &theme))
                    .when(snapshot.diagnostics.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.muted_foreground)
                                .child("No broker diagnostic lines."),
                        )
                    })
                    .children(snapshot.diagnostics.iter().map(|line| {
                        div()
                            .font_family(theme.mono_font_family.clone())
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child(line.clone())
                    })),
            )
            .into_any_element()
    }
}
