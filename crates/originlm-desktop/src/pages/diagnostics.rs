//! Diagnostics: runtime health in plain language, open issues with their
//! evidence and remediation, raw diagnostic lines, and a redacted report.

use crate::app::Desktop;
use crate::data;
use crate::ds::{self, sp, ty};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use originlm_contracts::{HealthStatus, Issue, IssueSeverity, Snapshot};

fn health_view(status: HealthStatus) -> (Tone, &'static str) {
    match status {
        HealthStatus::Healthy => (Tone::Green, "Healthy"),
        HealthStatus::Degraded => (Tone::Amber, "Degraded"),
        HealthStatus::Unavailable => (Tone::Red, "Unavailable"),
        HealthStatus::Unknown => (Tone::Neutral, "Not checked yet"),
    }
}

impl Desktop {
    pub fn page_diagnostics(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(bundle) = self.bundle.as_ref() else {
            return div().into_any_element();
        };
        let snapshot = bundle.snapshot.clone();
        let issues: Vec<Issue> = data::open_issues(&snapshot).into_iter().cloned().collect();
        let health = snapshot.health.clone();
        let (tone, status) = health_view(health.status);
        let checked = health
            .checked_at
            .as_ref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t.as_str()).ok())
            .map(|stamp| {
                stamp
                    .with_timezone(&chrono::Local)
                    .format("%b %d, %H:%M")
                    .to_string()
            });
        let document = originlm_control::diagnostic_document(
            &snapshot,
            &bundle.runtime,
            &bundle.apps,
            &bundle.adapters,
        );
        let entity = cx.entity();

        let summary = match (&checked, health.status) {
            (_, HealthStatus::Unknown) | (None, _) => {
                "The runtime has not run a health check yet. Issues below come from discovery."
                    .to_string()
            }
            (Some(at), _) => format!(
                "{} · last checked {at}{}",
                ui::plural(health.open_issues as usize, "open issue"),
                if health.hydrated {
                    " · loaded from cache, re-checking"
                } else {
                    ""
                }
            ),
        };

        let health_panel = ds::panel(&theme).child(
            div()
                .h_flex()
                .items_center()
                .flex_wrap()
                .gap(px(sp::MD))
                .px(px(sp::LG))
                .py(px(sp::MD))
                .child(
                    div()
                        .v_flex()
                        .flex_1()
                        .min_w(px(260.))
                        .gap(px(2.))
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap(px(sp::SM))
                                .child(ui::dot(tone, &theme))
                                .child(
                                    div()
                                        .text_size(px(ty::TITLE))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(status),
                                ),
                        )
                        .child(ds::body(summary, &theme)),
                )
                .child(
                    Button::new("copy-report")
                        .secondary()
                        .label("Copy redacted report")
                        .icon(IconName::Copy)
                        .tooltip("Copies the same redacted JSON the export writes. No credentials or account identities are included.")
                        .on_click({
                            let entity = entity.clone();
                            let document = document.clone();
                            move |_, _, cx| {
                                let text = serde_json::to_string_pretty(&document)
                                    .unwrap_or_else(|_| "{}".into());
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                entity.update(cx, |this, cx| {
                                    this.toast("Redacted report copied");
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    Button::new("export")
                        .ghost()
                        .label("Save to file")
                        .icon(IconName::FileText)
                        .on_click({
                            let entity = entity.clone();
                            move |_, _, cx| {
                                let path = originlm_control::default_export_path();
                                let result = originlm_control::write_export(&path, &document);
                                let shown = std::fs::canonicalize(&path)
                                    .unwrap_or_else(|_| path.clone());
                                entity.update(cx, |this, cx| {
                                    match result {
                                        Ok(()) => this.toast(format!(
                                            "Redacted report saved to {}",
                                            shown.display()
                                        )),
                                        Err(error) => this.toast(format!("Export failed: {error}")),
                                    }
                                    cx.notify();
                                });
                            }
                        }),
                ),
        );

        let issues_panel = ds::panel(&theme)
            .child(ds::panel_head(
                "Open issues",
                Some(ds::caption(ui::plural(issues.len(), "issue"), &theme).into_any_element()),
                &theme,
            ))
            .when(issues.is_empty(), |this| {
                this.child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(sp::SM))
                        .px(px(sp::LG))
                        .py(px(sp::MD))
                        .child(ui::icon(IconName::CircleCheck, 14., theme.success))
                        .child(ds::body("The runtime reports no open issues.", &theme)),
                )
            })
            .children(
                issues
                    .iter()
                    .enumerate()
                    .map(|(index, issue)| issue_row(index, issue, &snapshot, &theme)),
            );

        let lines = ds::panel(&theme)
            .child(ds::panel_head(
                "Runtime diagnostic lines",
                Some(
                    ds::caption(ui::plural(snapshot.diagnostics.len(), "line"), &theme)
                        .into_any_element(),
                ),
                &theme,
            ))
            .child(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .when(snapshot.diagnostics.is_empty(), |this| {
                        this.child(ds::body("No diagnostic lines.", &theme))
                    })
                    .children(snapshot.diagnostics.iter().map(|line| {
                        ds::mono(line.clone(), ty::SMALL, theme.muted_foreground, &theme)
                    })),
            );

        ds::page()
            .child(health_panel)
            .child(issues_panel)
            .child(lines)
            .into_any_element()
    }
}

fn issue_row(index: usize, issue: &Issue, snapshot: &Snapshot, theme: &Theme) -> Div {
    let cached = issue.connection_id.as_ref().and_then(|id| {
        snapshot.connections.iter().find(|connection| {
            &connection.id == id && data::auth_view(connection).kind == data::AuthKind::Cached
        })
    });
    let provider = cached.map(|connection| data::provider_name(snapshot, &connection.provider_id));
    let tone = match issue.severity {
        IssueSeverity::Critical | IssueSeverity::Error => Tone::Red,
        IssueSeverity::Warning => Tone::Amber,
        IssueSeverity::Info => Tone::Neutral,
    };
    div()
        .v_flex()
        .gap(px(sp::SM))
        .px(px(sp::LG))
        .py(px(sp::MD))
        .when(index > 0, |this| this.border_t_1().border_color(theme.border))
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(sp::SM))
                .child(ds::chip(data::severity_label(issue.severity), tone, theme))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(ty::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .child(issue.title.clone()),
                )
                .child(ds::caption(format!("seen {}×", issue.occurrences), theme)),
        )
        .child(ds::body(
            match &provider {
                Some(provider) => format!(
                    "The runtime found {provider}'s local account cache but cannot verify the session. Its usage and model catalog stay inside the {provider} client until you sign in there."
                ),
                None => issue.summary.clone(),
            },
            theme,
        ))
        .when(!issue.evidence.is_empty(), |this| {
            this.child(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .px(px(sp::MD))
                    .py(px(sp::SM))
                    .rounded(px(ds::radius::CONTROL))
                    .bg(ds::surface(theme, ds::Surface::Page))
                    .children(issue.evidence.iter().map(|evidence| {
                        div()
                            .h_flex()
                            .gap(px(sp::SM))
                            .child(ds::mono(
                                evidence.code.clone(),
                                ty::CAPTION,
                                theme.foreground,
                                theme,
                            ))
                            .child(ds::caption(evidence.message.clone(), theme))
                    })),
            )
        })
        .when(!issue.remediation.is_empty(), |this| {
            this.child(
                div()
                    .v_flex()
                    .gap(px(sp::XS))
                    .child(
                        div()
                            .text_size(px(ty::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("What to do"),
                    )
                    .children(issue.remediation.iter().enumerate().map(|(step, item)| {
                        div()
                            .h_flex()
                            .items_start()
                            .gap(px(sp::SM))
                            .child(ds::mono(
                                format!("{}.", step + 1),
                                ty::SMALL,
                                theme.muted_foreground,
                                theme,
                            ))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .text_color(theme.foreground)
                                    .child(item.description.clone()),
                            )
                    })),
            )
        })
}
