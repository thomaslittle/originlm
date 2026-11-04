//! Developer: what an app built on this runtime can see and do, with the real
//! route IDs on this machine and SDK samples that match the published API.

use crate::app::{Desktop, Page};
use crate::data;
use crate::ds::{self, sp, ty};
use crate::pages::providers::{auth_short, auth_tone, capability_name, route_capabilities};
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

struct Sample {
    label: &'static str,
    file: &'static str,
    install: &'static str,
    code: &'static str,
}

const SAMPLES: [Sample; 3] = [
    Sample {
        label: "TypeScript",
        file: "run.ts",
        install: "npm install originlm@alpha",
        code: r#"import { createLocalAI } from "originlm";

const ai = await createLocalAI();
try {
  // Discover: read-only, no grant needed.
  const { quotaPools } = await ai.snapshot.get();

  // Select: eligible routes, with the reasons for each.
  const [pick] = await ai.selection.resolve({
    requiredCapabilities: ["generation.text"],
    eligibleOnly: true,
  });

  // Grant: a request only. The user approves it in their host.
  await ai.grants.request({ connectionIds: [pick.connectionId], allowPaid: false });

  // Run: one session, one turn, streamed.
  const prompt = "Summarize this repository in three lines.";
  const session = await ai.sessions.create({
    selection: pick.selection,
    workspaceId: "default",
    permissionMode: "read_only",
  });
  const events = ai.events.subscribe({ sessionId: session.id });
  await ai.turns.start({ sessionId: session.id, requestId: crypto.randomUUID(), prompt });
  for await (const event of events) {
    if (event.payload.type === "text_delta") process.stdout.write(event.payload.text);
    if (event.payload.type === "final") break;
  }
} finally {
  await ai.close();
}"#,
    },
    Sample {
        label: "Rust",
        file: "main.rs",
        install: "cargo add originlm@=0.1.0-alpha.25",
        code: r#"fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Uses a shared broker if one is running, or embeds the runtime.
    let mut ai = originlm::LocalAi::start()?;

    // An explicit passive refresh. Probes and execution need policy.
    let snapshot = ai.scan()?;
    println!("{:?}: {} routes", ai.mode(), snapshot.routes.len());
    Ok(())
}"#,
    },
    Sample {
        label: "Python",
        file: "usage.py",
        install: "pip install originlm==0.1.0a25",
        code: r#"from originlm import Client

# Importing never scans, spawns or reads credentials.
with Client.auto() as ai:
    snapshot = ai.snapshot.get()
    for pool in snapshot["quotaPools"]:
        for window in pool["windows"]:
            print(pool["id"], window["label"], window["reading"]["state"])"#,
    },
];

const STEPS: [(&str, &str, &str); 4] = [
    (
        "Discover",
        "snapshot.get",
        "Read-only inventory: accounts, routes, models, allowance windows. No grant.",
    ),
    (
        "Select",
        "selection.resolve",
        "Eligible routes for the capabilities you need, each with its reasons.",
    ),
    (
        "Grant",
        "grants.request",
        "Returns requires_host_approval. Approval happens in the host, never in the SDK.",
    ),
    (
        "Run",
        "sessions.create · turns.start",
        "Events stream from a cursor. Cancel touches only work this app started.",
    ),
];

const METHODS: [(&str, &str); 8] = [
    (
        "runtime.handshake",
        "Negotiate protocol 1.0 and list supported methods.",
    ),
    (
        "snapshot.get",
        "Providers, connections, routes, models, quota pools and issues.",
    ),
    (
        "usage.read",
        "Provider-reported allowance windows. Never summed across pools.",
    ),
    (
        "discovery.refresh",
        "Passive metadata scan. Needs a discovery grant.",
    ),
    (
        "connections.probe",
        "Refresh one account's metadata and usage. Needs a probe grant.",
    ),
    (
        "models.page",
        "Provider-native catalog with route, options and evidence.",
    ),
    (
        "apps.list · apps.revoke",
        "Connected apps and revocation. Administrator only.",
    ),
    (
        "events.subscribe",
        "Session events from a cursor. A gap raises EVENT_GAP.",
    ),
];

fn billing_tone(label: &str) -> Tone {
    match label {
        "subscription" | "local" => Tone::Neutral,
        "metered API" | "prepaid credits" => Tone::Amber,
        _ => Tone::Neutral,
    }
}

impl Desktop {
    fn runtime_panel(&self, theme: &Theme) -> Div {
        let mode = if self.example {
            "Example mode. No runtime is running.".to_string()
        } else if self.connected {
            self.credential.label()
        } else {
            "Not connected".to_string()
        };
        let revision = self
            .snapshot()
            .map(|snapshot| snapshot.revision.clone())
            .unwrap_or_else(|| "none".into());
        let value = |text: String| {
            div()
                .text_size(px(ty::SMALL))
                .text_color(theme.foreground)
                .child(text)
        };
        ds::panel(theme)
            .flex_1()
            .child(ds::panel_head("Runtime", None, theme))
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::kv("Connection", value(mode), theme))
                    .child(ds::kv(
                        "Protocol",
                        value("JSON-RPC 2.0, protocol 1.0".into()),
                        theme,
                    ))
                    .child(ds::kv(
                        "Transports",
                        value("Shared broker · stdio helper · loopback HTTP with pairing · browser extension".into()),
                        theme,
                    ))
                    .child(ds::kv(
                        "Snapshot revision",
                        ds::mono(revision, ty::SMALL, theme.foreground, theme),
                        theme,
                    ))
                    .child(ds::kv(
                        "Telemetry",
                        value("None. Requests go only to the provider a route names.".into()),
                        theme,
                    )),
            )
    }

    fn permission_panel(&self, theme: &Theme, columns: u16) -> Div {
        ds::panel(theme)
            .child(ds::panel_head(
                "Permission before execution",
                Some(ds::caption("What an app may do, in order", theme).into_any_element()),
                theme,
            ))
            .child(
                div()
                    .grid()
                    .grid_cols(columns)
                    .children(
                        STEPS
                            .iter()
                            .enumerate()
                            .map(|(index, (title, method, detail))| {
                                div()
                                    .v_flex()
                                    .gap(px(sp::XS))
                                    .px(px(sp::LG))
                                    .py(px(sp::MD))
                                    .when(!(index as u16).is_multiple_of(columns), |this| {
                                        this.border_l_1().border_color(theme.border)
                                    })
                                    .when(index as u16 >= columns, |this| {
                                        this.border_t_1().border_color(theme.border)
                                    })
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap(px(sp::SM))
                                            .child(ds::mono(
                                                format!("{}", index + 1),
                                                ty::SMALL,
                                                theme.primary,
                                                theme,
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(ty::BODY))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(*title),
                                            ),
                                    )
                                    .child(ds::mono(*method, ty::SMALL, theme.foreground, theme))
                                    .child(ds::caption(*detail, theme))
                            }),
                    ),
            )
    }

    fn routes_panel(&self, theme: &Theme) -> Div {
        struct RouteRow {
            id: String,
            provider: String,
            account: String,
            billing: String,
            models: usize,
            caps: Vec<String>,
            signed_in: data::AuthKind,
        }
        let rows: Vec<RouteRow> = self
            .snapshot()
            .map(|snapshot| {
                snapshot
                    .routes
                    .iter()
                    .map(|route| {
                        let connection = data::connection_for_route(snapshot, route);
                        let provider = data::provider_name(snapshot, &route.provider_id);
                        RouteRow {
                            id: route.id.clone(),
                            account: connection
                                .map(|c| {
                                    crate::human::without_provider_prefix(
                                        &c.display_name,
                                        &provider,
                                    )
                                })
                                .unwrap_or_else(|| route.display_name.clone()),
                            provider,
                            billing: ui::billing_label(route).to_string(),
                            models: snapshot
                                .models
                                .iter()
                                .filter(|model| model.route_id == route.id)
                                .count(),
                            caps: route_capabilities(snapshot, &route.id),
                            signed_in: connection
                                .map(|c| data::auth_view(c).kind)
                                .unwrap_or(data::AuthKind::Unknown),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let wide = self.content_width() >= 1100.;
        ds::panel(theme)
            .child(ds::panel_head(
                "Routes on this machine",
                Some(
                    ds::caption(
                        format!(
                            "{}. Copy a route ID into a selection.",
                            ui::plural(rows.len(), "route")
                        ),
                        theme,
                    )
                    .into_any_element(),
                ),
                theme,
            ))
            .child(
                ds::table_head(theme)
                    .child(ds::th("Provider and account", None, theme))
                    .child(ds::th("Billing", Some(130.), theme))
                    .when(wide, |this| this.child(ds::th("Capabilities (supported)", Some(230.), theme)))
                    .child(ds::th("Sign-in", Some(130.), theme))
                    .child(ds::th("Route ID", Some(250.), theme)),
            )
            .when(rows.is_empty(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "No routes yet",
                    "Discovery has not reported a client this runtime can route to. Accounts shows what was found.",
                    theme,
                ))
            })
            .children(rows.into_iter().enumerate().map(|(index, row)| {
                ds::panel_row(index == 0, theme)
                    .min_h(px(ds::ROW))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(ty::SMALL))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(row.provider),
                            )
                            .child(
                                ds::caption(
                                    format!("{} · {}", row.account, ui::plural(row.models, "model")),
                                    theme,
                                )
                                .truncate(),
                            ),
                    )
                    .child(div().h_flex().w(px(130.)).flex_shrink_0().child(ds::chip(
                        row.billing.clone(),
                        billing_tone(&row.billing),
                        theme,
                    )))
                    .when(wide, |this| {
                        this.child(
                            div().w(px(230.)).flex_shrink_0().child(
                                ds::caption(
                                    if row.caps.is_empty() {
                                        "None reported".to_string()
                                    } else {
                                        row.caps
                                            .iter()
                                            .map(|cap| capability_name(cap))
                                            .collect::<Vec<_>>()
                                            .join(" · ")
                                    },
                                    theme,
                                )
                                .truncate(),
                            ),
                        )
                    })
                    .child(div().h_flex().w(px(130.)).flex_shrink_0().child(ds::chip(
                        auth_short(row.signed_in),
                        auth_tone(row.signed_in),
                        theme,
                    )))
                    .child(
                        div()
                            .w(px(250.))
                            .flex_shrink_0()
                            .child(ds::copy_value(format!("route-{index}"), row.id, 28, theme)),
                    )
            }))
    }

    /// Recent turns from the runtime's cost evidence: what ran, token counts,
    /// and the cost basis. Nothing is computed that the runtime did not report.
    fn turns_panel(&self, theme: &Theme) -> Div {
        let report = self
            .bundle
            .as_ref()
            .and_then(|bundle| bundle.costs.as_ref());
        let records: Vec<_> = report
            .map(|report| report.records.iter().rev().take(6).collect())
            .unwrap_or_default();
        let total = report.map(|report| report.records.len()).unwrap_or(0);
        ds::panel(theme)
            .child(ds::panel_head(
                if self.example {
                    "Recent turns (example data)"
                } else {
                    "Recent turns"
                },
                Some(ds::caption(ui::plural(total, "turn"), theme).into_any_element()),
                theme,
            ))
            .when(report.is_none(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "No turn evidence from this runtime",
                    self.bundle
                        .as_ref()
                        .and_then(|bundle| bundle.cost_error.clone())
                        .unwrap_or_else(|| "The runtime did not answer costs.read.".into()),
                    theme,
                ))
            })
            .when(report.is_some() && records.is_empty(), |this| {
                this.child(ds::state(
                    ds::StateKind::Empty,
                    "No turns recorded yet",
                    "Turns appear here with their token counts once an app runs one on this runtime.",
                    theme,
                ))
            })
            .when(!records.is_empty(), |this| {
                this.child(
                    ds::table_head(theme)
                        .child(ds::th("Model", None, theme))
                        .child(ds::th("Tokens in / out", Some(130.), theme))
                        .child(ds::th("Cost", Some(90.), theme))
                        .child(ds::th("Basis", Some(110.), theme)),
                )
            })
            .children(records.into_iter().enumerate().map(|(index, record)| {
                let tokens = |value: &Option<originlm_contracts::DecimalString>| {
                    value
                        .as_ref()
                        .map(|v| v.as_str().to_string())
                        .unwrap_or_else(|| "?".into())
                };
                let (basis, tone) = match record.basis.as_str() {
                    "provider_reported" => ("Reported", Tone::Green),
                    "estimated" => ("Estimated", Tone::Amber),
                    _ => ("Unknown", Tone::Neutral),
                };
                ds::panel_row(index == 0, theme)
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                ds::mono(
                                    record.native_model_id.clone(),
                                    ty::SMALL,
                                    theme.foreground,
                                    theme,
                                )
                                .truncate(),
                            )
                            .child(ds::caption(
                                chrono::DateTime::parse_from_rfc3339(record.usage.observed_at.as_str())
                                    .map(|t| t.with_timezone(&chrono::Local).format("%a %H:%M").to_string())
                                    .unwrap_or_default(),
                                theme,
                            )),
                    )
                    .child(div().w(px(130.)).flex_shrink_0().child(ds::mono(
                        format!(
                            "{} / {}",
                            tokens(&record.usage.input_tokens),
                            tokens(&record.usage.output_tokens)
                        ),
                        ty::SMALL,
                        theme.foreground,
                        theme,
                    )))
                    .child(div().w(px(90.)).flex_shrink_0().child(ds::mono(
                        record
                            .amount_usd
                            .as_ref()
                            .map(|v| format!("${}", v.as_str()))
                            .unwrap_or_else(|| "unknown".into()),
                        ty::SMALL,
                        theme.foreground,
                        theme,
                    )))
                    .child(div().h_flex().w(px(110.)).flex_shrink_0().child(ds::chip(basis, tone, theme)))
            }))
    }

    fn access_summary(&self, theme: &Theme, entity: Entity<Self>) -> Div {
        let bundle = self.bundle.as_ref();
        let apps = bundle.map(|b| b.apps.len()).unwrap_or(0);
        let readable = bundle.is_some_and(|b| b.apps_error.is_none());
        ds::panel(theme)
            .child(ds::panel_head(
                "Apps with access",
                Some(
                    ds::link("dev-apps", "Apps & grants", theme, move |_, _, cx| {
                        entity.update(cx, |this, cx| this.set_page(Page::Apps, cx))
                    })
                    .into_any_element(),
                ),
                theme,
            ))
            .child(
                div()
                    .v_flex()
                    .px(px(sp::LG))
                    .py(px(sp::SM))
                    .child(ds::kv(
                        "Connected now",
                        div().text_size(px(ty::SMALL)).child(if self.example {
                            "None in example mode".to_string()
                        } else if readable {
                            ui::plural(apps, "app")
                        } else {
                            "Needs the administrator credential".to_string()
                        }),
                        theme,
                    ))
                    .child(ds::kv(
                        "Paid routes",
                        div()
                            .text_size(px(ty::SMALL))
                            .child("Refused unless an execution grant sets allowPaid"),
                        theme,
                    ))
                    .child(ds::kv(
                        "Revocation",
                        div()
                            .text_size(px(ty::SMALL))
                            .child("Per app; live connections close at once"),
                        theme,
                    )),
            )
    }

    pub fn page_developer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let current = self.dev_sample.min(SAMPLES.len() - 1);
        let sample = &SAMPLES[current];
        let width = self.content_width();
        let wide = width >= 1100.;

        let tabs = ds::tabs(
            "dev-sample",
            SAMPLES.iter().map(|item| item.label),
            current,
            &theme,
            {
                let entity = entity.clone();
                move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.dev_sample = index;
                        cx.notify();
                    })
                }
            },
        );

        let quick_start = ds::panel(&theme)
            .flex_1()
            .min_w(px(0.))
            .child(ds::panel_head(
                "Quick start",
                Some(tabs.into_any_element()),
                &theme,
            ))
            .child(
                div()
                    .v_flex()
                    .gap(px(sp::SM))
                    .p(px(sp::LG))
                    .child(ds::code(
                        SharedString::from(format!("dev-code-{current}")),
                        sample.file,
                        sample.code,
                        &theme,
                    ))
                    .child(ds::mono(
                        sample.install,
                        ty::SMALL,
                        theme.muted_foreground,
                        &theme,
                    )),
            );

        let reference = ds::panel(&theme)
            .when(wide, |this| this.w(px(380.)).flex_shrink_0())
            .child(ds::panel_head(
                "Protocol reference",
                Some(
                    ds::link("open-api-guide", "Full API", &theme, |_, _, cx| {
                        cx.open_url(
                            "https://github.com/thomaslittle/originlm/blob/main/docs/integration/generated-api.md",
                        );
                    })
                    .into_any_element(),
                ),
                &theme,
            ))
            .children(METHODS.iter().enumerate().map(|(index, (method, detail))| {
                ds::panel_row(index == 0, &theme)
                    .py(px(sp::SM))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .min_w(px(0.))
                            .child(ds::mono(*method, ty::SMALL, theme.foreground, &theme))
                            .child(ds::caption(*detail, &theme)),
                    )
            }));

        let guides = ds::panel(&theme)
            .child(ds::panel_head("Guides", None, &theme))
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(sp::XS))
                    .p(px(sp::SM))
                    .child(ds::link("doc-quickstarts", "Framework quickstarts", &theme, |_, _, cx| {
                        cx.open_url("https://github.com/thomaslittle/originlm/blob/main/docs/integration/quickstarts.md");
                    }))
                    .child(ds::link("doc-host-mcp", "Host MCP tools", &theme, |_, _, cx| {
                        cx.open_url("https://github.com/thomaslittle/originlm/blob/main/docs/integration/host-mcp.md");
                    }))
                    .child(ds::link("doc-broker", "Shared broker and grants", &theme, |_, _, cx| {
                        cx.open_url("https://github.com/thomaslittle/originlm/blob/main/docs/integration/shared-broker.md");
                    }))
                    .child(ds::link("doc-matrix", "Support matrix", &theme, |_, _, cx| {
                        cx.open_url("https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md");
                    }))
                    .child(ds::link("dev-playground", "Playground", &theme, {
                        let entity = entity.clone();
                        move |_, _, cx| entity.update(cx, |this, cx| this.set_page(Page::Playground, cx))
                    })),
            );

        let pair = |left: Div, right: Div| {
            if wide {
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(sp::MD))
                    .child(left.flex_1().min_w(px(0.)))
                    .child(right)
            } else {
                div().v_flex().gap(px(sp::MD)).child(left).child(right)
            }
        };

        ds::page()
            .child(self.permission_panel(&theme, if width >= 900. { 4 } else { 2 }))
            .child(pair(quick_start, reference))
            .child(self.routes_panel(&theme))
            .child(pair(
                self.turns_panel(&theme),
                self.access_summary(&theme, entity.clone())
                    .when(wide, |this| this.w(px(380.)).flex_shrink_0()),
            ))
            .child(pair(
                self.runtime_panel(&theme),
                guides.when(wide, |this| this.w(px(380.)).flex_shrink_0()),
            ))
            .into_any_element()
    }
}
