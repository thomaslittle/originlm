//! Native end-to-end interaction tests for the real page views.
//!
//! The page is rendered in GPUI's deterministic test window and driven with
//! GPUI pointer input. This follows Zed's GPUI test pattern and GPUI Kit's
//! view tests: no Windows UI Automation, SendKeys, focus changes, screenshots
//! or OS-level input are involved.

use crate::app::{Desktop, Page};
use crate::bootstrap::Credential;
use crate::broker::{Bundle, ExecCommand, ExecEvent, ExecHandle};
use gpui_kit::test::{TestAppContextExt, TestSupportExt as _, TestWindowExt};
use gpui_kit::{
    AppContext as _, Bounds, Entity, InteractiveElement as _, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, point, px, size,
};
use originlm_contracts::*;
use originlm_kit::{ClientPolicy, ExecutionGrant, LocatedClient, NativeService, ScanResult};
use originlm_runtime::{Agent, BoxFuture, PriorHistory, TurnContext};
use std::time::Duration;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

fn evidence() -> EvidenceSource {
    EvidenceSource::new(EvidenceKind::Synthetic, "native-e2e", "fixture")
}

fn reading(used: &str) -> Observation<UsageReading> {
    Observation::reported(
        UsageReading::try_from(UsageInput {
            unit: "percent".into(),
            used: used.to_owned().try_into().unwrap(),
            limit: Some("100".to_owned().try_into().unwrap()),
        })
        .unwrap(),
        evidence(),
    )
}

fn route(id: &str, connection: &str, provider: &str, billing: BillingMode) -> AccessRoute {
    AccessRoute {
        id: id.into(),
        connection_id: connection.into(),
        provider_id: provider.into(),
        display_name: id.into(),
        billing: Observation::reported(billing, evidence()),
        capabilities: vec![],
        catalog_revision: "1".into(),
    }
}

fn model(id: &str, route: &str, name: &str, native: &str) -> Model {
    Model {
        id: id.into(),
        route_id: route.into(),
        native_id: native.into(),
        name: name.into(),
        revision: "1".into(),
        is_default: false,
        capabilities: vec![],
        options: vec![],
        context_window: Observation::unknown("NOT_REPORTED"),
    }
}

fn fixture() -> Snapshot {
    let mut snapshot = Snapshot::empty(DataMode::Synthetic);
    snapshot.providers.push(Provider {
        id: "opencode".into(),
        name: "OpenCode".into(),
        kind: "gateway".into(),
    });
    snapshot.providers.push(Provider {
        id: "codex".into(),
        name: "Codex".into(),
        kind: "native".into(),
    });
    snapshot.providers.push(Provider {
        id: "muse".into(),
        name: "Muse Code".into(),
        kind: "native".into(),
    });
    for (id, provider) in [("install-opencode", "opencode"), ("install-codex", "codex")] {
        snapshot.installations.push(Installation {
            id: id.into(),
            provider_id: provider.into(),
            environment_id: "test".into(),
            location_hint: "Executable candidate present; not launched".into(),
            version: Observation::unknown("NOT_REPORTED"),
        });
    }
    snapshot.profiles.push(Profile {
        id: "profile-opencode".into(),
        installation_id: "install-opencode".into(),
        label: "Default profile · Installation 2".into(),
    });
    snapshot.profiles.push(Profile {
        id: "profile-codex".into(),
        installation_id: "install-codex".into(),
        label: "Additional profile 2".into(),
    });
    snapshot.connections.push(Connection {
        id: "connection-opencode".into(),
        provider_id: "opencode".into(),
        installation_id: "install-opencode".into(),
        profile_id: "profile-opencode".into(),
        display_name: "OpenCode · Default profile · Installation 2".into(),
        account_id: None,
        auth: Observation::reported(AuthState::Authenticated, evidence()),
        plan: Observation::unknown("NOT_REPORTED"),
        status: "metadata_ready".into(),
        diagnostics: vec![],
    });
    snapshot.connections.push(Connection {
        id: "connection-codex".into(),
        provider_id: "codex".into(),
        installation_id: "install-codex".into(),
        profile_id: "profile-codex".into(),
        display_name: "Codex · Additional profile 2".into(),
        account_id: None,
        auth: Observation::reported(AuthState::Authenticated, evidence()),
        plan: Observation::unknown("NOT_REPORTED"),
        status: "metadata_ready".into(),
        diagnostics: vec![],
    });
    snapshot.connections.push(Connection {
        id: "connection-muse".into(),
        provider_id: "muse".into(),
        installation_id: "install-codex".into(),
        profile_id: "profile-codex".into(),
        display_name: "Muse Code · Local".into(),
        account_id: None,
        auth: Observation::reported(AuthState::Authenticated, evidence()),
        plan: Observation::unknown("NOT_REPORTED"),
        status: "metadata_ready".into(),
        diagnostics: vec![],
    });
    snapshot.routes.push(route(
        "route-zen",
        "connection-opencode",
        "opencode",
        BillingMode::ApiMetered,
    ));
    snapshot.routes.push(route(
        "route-go",
        "connection-opencode",
        "opencode-go",
        BillingMode::Subscription,
    ));
    snapshot.routes.push(route(
        "route-local",
        "connection-opencode",
        "ollama",
        BillingMode::Local,
    ));
    snapshot.routes.push(route(
        "route-codex",
        "connection-codex",
        "codex",
        BillingMode::Subscription,
    ));
    snapshot.routes.push(route(
        "route-muse",
        "connection-muse",
        "muse",
        BillingMode::Subscription,
    ));
    snapshot.models.push(model(
        "model-free",
        "route-zen",
        "Crouton Alpha Free",
        "crouton-alpha-free",
    ));
    snapshot.models.push(model(
        "model-zen",
        "route-zen",
        "Claude Haiku 5.5",
        "claude-haiku-5-5",
    ));
    snapshot.models.push(model(
        "model-go",
        "route-go",
        "Kimi K3.1 Pro",
        "kimi-k3.1-pro",
    ));
    let mut local = model("model-local", "route-local", "KAT-Coder", "kat-coder");
    local.options = vec![OptionDescriptor {
        id: "variant".into(),
        label: "Native variant".into(),
        scope: OptionScope::Turn,
        revision: "1".into(),
        control: OptionControl::Select {
            choices: vec![
                Choice {
                    value: "low".into(),
                    label: "Low".into(),
                    disabled_reason: None,
                },
                Choice {
                    value: "medium".into(),
                    label: "Medium".into(),
                    disabled_reason: None,
                },
                Choice {
                    value: "high".into(),
                    label: "High".into(),
                    disabled_reason: None,
                },
            ],
            default_value: Some("medium".into()),
        },
        current_value: None,
        category: Some("reasoning".into()),
        source: None,
        billing_sensitive: false,
        permission_sensitive: false,
    }];
    snapshot.models.push(local);
    snapshot.models.push(model(
        "model-codex",
        "route-codex",
        "GPT-6 Luna",
        "gpt-6-luna",
    ));
    snapshot.models.push(model(
        "model-muse",
        "route-muse",
        "Muse Spark 1.3",
        "muse-spark-1.3",
    ));
    snapshot.quota_pools.push(QuotaPool {
        id: "pool-codex".into(),
        account_id: None,
        connection_ids: vec!["connection-codex".into()],
        route_ids: vec!["route-codex".into()],
        sharing: "account".into(),
        windows: vec![
            UsageWindow {
                id: "five-hour".into(),
                label: "5 hr".into(),
                reading: reading("2"),
                resets_at: None,
                duration_seconds: Some(18_000),
            },
            UsageWindow {
                id: "weekly".into(),
                label: "Weekly".into(),
                reading: reading("30"),
                resets_at: None,
                duration_seconds: Some(604_800),
            },
        ],
    });
    crate::data::canonical_snapshot(snapshot)
}

fn bundle() -> Bundle {
    Bundle {
        snapshot: fixture(),
        runtime: serde_json::json!({"mode": "synthetic"}),
        apps: vec![],
        apps_error: None,
        adapters: vec![],
        local_models: vec![],
        detection: None,
        history: vec![],
        costs: None,
        cost_error: None,
        fetched_at: 0,
        error: None,
    }
}

/// Renders one page directly. The full shell's resizable panels are not part
/// of what these tests exercise; the page view is the production render.
struct PageHarness {
    desktop: Entity<Desktop>,
    page: Page,
}

impl Render for PageHarness {
    fn render(
        &mut self,
        _: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let page = self.page;
        let page = self.desktop.update(cx, |desktop, cx| match page {
            Page::Overview => desktop.page_overview(cx),
            Page::Models => desktop.page_models(cx),
            Page::Playground => desktop.page_playground(cx),
            _ => unreachable!("test harness only renders Overview, Models and Playground"),
        });
        gpui_kit::div()
            .id("page-harness")
            .w_full()
            .h(px(800.))
            .flex()
            .flex_col()
            .child(page)
            .test_support()
    }
}

#[gpui_kit::test]
async fn native_overview_and_models_surface_real_labels(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, harness, desktop) = cx.update(|cx| {
        let bounds = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1440.), px(920.)),
        };
        let (window, harness) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let desktop = cx.new(|cx| {
                    Desktop::new(
                        window,
                        cx,
                        Credential::Unavailable("native e2e".into()),
                        30,
                        true,
                    )
                });
                cx.new(|_| PageHarness {
                    desktop: desktop.clone(),
                    page: Page::Overview,
                })
            },
        )
        .expect("open the real page view");
        let desktop = harness.read(cx).desktop.clone();
        (window, harness, desktop)
    });

    // Example mode sends its bundle once and then waits for commands. Replace
    // it with the interaction fixture before driving the view.
    cx.wait_for(handle, Duration::from_secs(3), |_, cx| {
        desktop.read(cx).bundle.is_some()
    })
    .await;

    desktop.update(cx, |desktop, cx| {
        desktop.viewport_w = 1440.;
        desktop.bundle = Some(bundle());
        cx.notify();
    });
    harness.update(cx, |_, cx| cx.notify());

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let opencode = window
            .find("provider-row-connection-opencode")
            .label()
            .unwrap_or_default()
            .to_string();
        assert!(
            !opencode.contains("Installation") && !opencode.contains("Additional profile"),
            "internal discovery indices leaked into Overview: {opencode}"
        );
        let codex = window
            .find("provider-row-connection-codex")
            .label()
            .unwrap_or_default()
            .to_string();
        assert!(
            codex.contains("5 hr") && codex.contains("Weekly"),
            "Codex row lost its duration labels: {codex}"
        );

        // Native GPUI pointer click on the production provider row.
        window.click("provider-row-connection-codex", cx);
        assert_eq!(desktop.read(cx).page, Page::Providers);
        assert_eq!(
            desktop.read(cx).selected_connection.as_deref(),
            Some("connection-codex")
        );

        harness.update(cx, |harness, cx| {
            harness.page = Page::Models;
            cx.notify();
        });
        window.render_frame(cx);
        for (id, plan) in [
            ("model-model-free", "Free"),
            ("model-model-zen", "Zen"),
            ("model-model-go", "Go"),
            ("model-model-local", "Local"),
        ] {
            let label = window.find(id).label().unwrap_or_default().to_string();
            assert!(
                label.contains(plan),
                "model row {id} did not report plan {plan}: {label}"
            );
        }
    })
    .expect("drive the test window");
}

// ---------------------------------------------------------------------------
// Playground model-switch e2e
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct OpenedModel {
    model: String,
    prior: Option<PriorHistory>,
}

struct E2eAgent {
    model: String,
}

impl Agent for E2eAgent {
    fn turn<'a>(
        &'a mut self,
        _: String,
        _: BTreeMap<String, serde_json::Value>,
        cx: TurnContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            cx.emit(EventPayload::TextDelta {
                text: format!("e2e reply from {}", self.model),
            })?;
            Ok(())
        })
    }
    fn close(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn handoff_snapshot() -> Snapshot {
    let mut snapshot = fixture();
    for id in ["model-local", "model-go"] {
        if let Some(model) = snapshot.models.iter_mut().find(|model| model.id == id) {
            model.capabilities.push(Capability {
                id: "generation.text".into(),
                support: Support::Supported,
                origin: CapabilityOrigin::Model,
                evidence: Observation::reported("native-e2e".into(), evidence()),
                conditions: vec![],
            });
        }
    }
    snapshot
}

fn handoff_service(
    opened: Arc<Mutex<Vec<OpenedModel>>>,
    workspace: std::path::PathBuf,
) -> Arc<NativeService> {
    let snapshot = Arc::new(handoff_snapshot());
    let scan = snapshot.clone();
    let service = NativeService::new(
        Arc::new(move |_| {
            let snapshot = (*scan).clone();
            let locations = snapshot
                .connections
                .iter()
                .map(|connection| {
                    (
                        connection.id.clone(),
                        LocatedClient {
                            executable_identity: None,
                            provider_id: connection.provider_id.clone(),
                            profile_home: std::env::temp_dir(),
                            executable: None,
                            scope: None,
                        },
                    )
                })
                .collect();
            Ok(ScanResult {
                snapshot,
                locations,
                metadata_probes: 0,
            })
        }),
        Duration::ZERO,
    )
    .with_execution(
        BTreeSet::from(["opencode".into()]),
        Arc::new(move |input| {
            opened.lock().unwrap().push(OpenedModel {
                model: input.model.id.clone(),
                prior: input.prior_history.clone(),
            });
            let model = input.model.id.clone();
            Box::pin(async move { Ok(Box::new(E2eAgent { model }) as Box<dyn Agent>) })
        }),
    );
    service
        .register_client("desktop", ClientPolicy::passive())
        .unwrap();
    service
        .register_execution(
            "desktop",
            ExecutionGrant {
                connection_ids: BTreeSet::from(["connection-opencode".into()]),
                workspaces: BTreeMap::from([("default".into(), workspace)]),
                ..Default::default()
            },
        )
        .unwrap();
    Arc::new(service)
}

fn test_executor(service: Arc<NativeService>) -> ExecHandle {
    let (commands_tx, commands_rx) = std::sync::mpsc::channel();
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let _ =
            runtime.block_on(service.handle("desktop", "discovery.refresh", serde_json::json!({})));
        let mut session: Option<String> = None;
        while let Ok(command) = commands_rx.recv() {
            match command {
                ExecCommand::Run(request) => {
                    let crate::broker::RunRequest {
                        prompt,
                        choice,
                        allow_paid,
                        ..
                    } = *request;
                    let selection = serde_json::json!({
                        "routeId": choice.route_id,
                        "modelId": choice.model_id,
                        "revision": choice.revision,
                        "options": {},
                    });
                    let created = match &session {
                        Some(id) => runtime.block_on(service.handle(
                            "desktop",
                            "sessions.switchModel",
                            serde_json::json!({
                                "sessionId": id,
                                "selection": selection,
                                "allowPaid": allow_paid,
                            }),
                        )),
                        None => runtime.block_on(service.handle(
                            "desktop",
                            "sessions.create",
                            serde_json::json!({
                                "selection": selection,
                                "workspaceId": "default",
                                "permissionMode": "read_only",
                                "reviewRequests": false,
                                "allowPaid": allow_paid,
                                "conversationId": "desktop-e2e",
                            }),
                        )),
                    };
                    let session_id = match created {
                        Ok(value) => value["id"].as_str().unwrap_or_default().to_string(),
                        Err(error) => {
                            let _ = events_tx.send(ExecEvent::Failed {
                                error: format!("session: {error}"),
                            });
                            continue;
                        }
                    };
                    session = Some(session_id.clone());
                    let started = runtime.block_on(service.handle(
                        "desktop",
                        "turns.start",
                        serde_json::json!({
                            "sessionId": session_id,
                            "prompt": prompt,
                            "requestId": format!("desktop-e2e-{}", chrono::Utc::now().timestamp_millis()),
                        }),
                    ));
                    let turn_id = match started {
                        Ok(value) => value["turnId"].as_str().unwrap_or_default().to_string(),
                        Err(error) => {
                            let _ = events_tx.send(ExecEvent::Failed {
                                error: format!("turn: {error}"),
                            });
                            continue;
                        }
                    };
                    let _ = events_tx.send(ExecEvent::Started {
                        session_id: session_id.clone(),
                        turn_id,
                        label: choice.label.clone(),
                    });
                    let deadline = std::time::Instant::now() + Duration::from_secs(10);
                    loop {
                        let state = runtime.block_on(service.handle(
                            "desktop",
                            "sessions.get",
                            serde_json::json!({ "sessionId": session_id }),
                        ));
                        let state = state
                            .ok()
                            .and_then(|value| value["state"].as_str().map(str::to_owned))
                            .unwrap_or_else(|| "error".into());
                        if state != "running" && state != "awaiting_approval" {
                            break;
                        }
                        if std::time::Instant::now() > deadline {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    let text = runtime
                        .block_on(service.handle(
                            "desktop",
                            "events.read",
                            serde_json::json!({ "sessionId": session_id }),
                        ))
                        .ok()
                        .and_then(|page| {
                            let mut text = String::new();
                            for event in page["events"].as_array()? {
                                if event["payload"]["type"] == "text_delta" {
                                    text.push_str(event["payload"]["text"].as_str().unwrap_or(""));
                                }
                            }
                            Some(text)
                        })
                        .unwrap_or_default();
                    if !text.is_empty() {
                        let _ = events_tx.send(ExecEvent::Delta {
                            session_id: session_id.clone(),
                            text,
                        });
                    }
                    let _ = events_tx.send(ExecEvent::Finished {
                        session_id,
                        status: "completed".into(),
                    });
                }
                ExecCommand::Cancel => {}
                ExecCommand::Close(session_id) => {
                    let _ = runtime.block_on(service.handle(
                        "desktop",
                        "sessions.close",
                        serde_json::json!({ "sessionId": session_id }),
                    ));
                    session = None;
                }
            }
        }
    });
    ExecHandle {
        commands: commands_tx,
        events: events_rx,
    }
}

#[gpui_kit::test]
async fn native_playground_model_switch_hands_off_the_conversation(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let workspace = tempfile::tempdir().unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let service = handoff_service(opened.clone(), workspace.path().canonicalize().unwrap());
    let executor = test_executor(service);
    let (handle, harness, desktop) = cx.update(|cx| {
        let bounds = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1440.), px(920.)),
        };
        let (window, harness) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let desktop = cx.new(|cx| {
                    Desktop::new(
                        window,
                        cx,
                        Credential::Unavailable("native e2e".into()),
                        30,
                        true,
                    )
                });
                desktop.update(cx, |desktop, cx| {
                    desktop.executor = executor;
                    desktop.page = Page::Playground;
                    desktop.viewport_w = 1440.;
                    cx.notify();
                });
                cx.new(|_| PageHarness {
                    desktop: desktop.clone(),
                    page: Page::Playground,
                })
            },
        )
        .expect("open the real Playground view");
        let desktop = harness.read(cx).desktop.clone();
        (window, harness, desktop)
    });

    cx.wait_for(handle, Duration::from_secs(3), |_, cx| {
        desktop.read(cx).bundle.is_some()
    })
    .await;

    cx.update_window(handle, |_, window, cx| {
        desktop.update(cx, |desktop, cx| {
            desktop.bundle = Some(bundle());
            cx.notify();
        });
        harness.update(cx, |_, cx| cx.notify());
        window.render_frame(cx);
        desktop.update(cx, |desktop, cx| {
            desktop.selected_model = Some("model-local".into());
            desktop.prompt.update(cx, |prompt, cx| {
                prompt.set_value("first e2e question", window, cx)
            });
            desktop.run_prompt(window, cx);
        });
    })
    .expect("run the first model turn");

    cx.wait_for(handle, Duration::from_secs(10), |_, cx| {
        let desktop = desktop.read(cx);
        !desktop.running
            && (desktop.playground_error.is_some()
                || desktop
                    .sessions
                    .iter()
                    .filter(|session| session.status == "completed")
                    .count()
                    >= 1)
    })
    .await;
    assert_eq!(
        cx.update(|cx| desktop.read(cx).playground_error.clone()),
        None,
        "first turn failed"
    );

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        desktop.update(cx, |desktop, cx| {
            desktop.bundle = Some(bundle());
            desktop.selected_model = Some("model-go".into());
            desktop.prompt.update(cx, |prompt, cx| {
                prompt.set_value("second e2e question", window, cx)
            });
            desktop.run_prompt(window, cx);
        });
    })
    .expect("switch models and run the second turn");

    cx.wait_for(handle, Duration::from_secs(10), |_, cx| {
        let desktop = desktop.read(cx);
        !desktop.running
            && (desktop.playground_error.is_some()
                || desktop
                    .sessions
                    .iter()
                    .filter(|session| session.status == "completed")
                    .count()
                    >= 2)
    })
    .await;
    assert_eq!(
        cx.update(|cx| desktop.read(cx).playground_error.clone()),
        None,
        "second turn failed"
    );

    let transcript = cx.update(|cx| {
        desktop
            .read(cx)
            .transcript
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
    });
    assert!(
        transcript.iter().any(|text| text == "first e2e question"),
        "the first user turn disappeared: {transcript:?}"
    );
    assert!(
        transcript
            .iter()
            .any(|text| text.contains("e2e reply from model-local")),
        "the first assistant reply disappeared: {transcript:?}"
    );
    assert!(
        transcript.iter().any(|text| text == "second e2e question"),
        "the second user turn is missing: {transcript:?}"
    );
    assert!(
        transcript
            .iter()
            .any(|text| text.contains("e2e reply from model-go")),
        "the second assistant reply is missing: {transcript:?}"
    );

    let opened = opened.lock().unwrap().clone();
    assert_eq!(
        opened.len(),
        2,
        "one provider session per model: {opened:?}"
    );
    assert_eq!(opened[0].model, "model-local");
    assert!(opened[0].prior.is_none(), "the first session is not seeded");
    assert_eq!(opened[1].model, "model-go");
    let prior = opened[1]
        .prior
        .as_ref()
        .expect("the switched model receives the prior conversation");
    assert_eq!(prior.turns.len(), 1);
    assert_eq!(prior.turns[0].user, "first e2e question");
    assert!(
        prior.turns[0]
            .assistant
            .contains("e2e reply from model-local"),
        "the prior assistant reply did not carry: {prior:?}"
    );

    harness.update(cx, |_, cx| cx.notify());
}

#[gpui_kit::test]
async fn native_playground_selectors_keyboard_and_attachments(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, harness, desktop) = cx.update(|cx| {
        let bounds = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1280.), px(800.)),
        };
        let (window, harness) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let desktop = cx.new(|cx| {
                    Desktop::new(
                        window,
                        cx,
                        Credential::Unavailable("native e2e".into()),
                        30,
                        true,
                    )
                });
                cx.new(|_| PageHarness {
                    desktop: desktop.clone(),
                    page: Page::Playground,
                })
            },
        )
        .expect("open the Playground view");
        let desktop = harness.read(cx).desktop.clone();
        (window, harness, desktop)
    });
    cx.wait_for(handle, Duration::from_secs(3), |_, cx| {
        desktop.read(cx).bundle.is_some()
    })
    .await;
    desktop.update(cx, |desktop, cx| {
        desktop.viewport_w = 1280.;
        desktop.bundle = Some(bundle());
        cx.notify();
    });
    harness.update(cx, |_, cx| cx.notify());

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "playground-page",
            "playground-transcript",
            "playground-composer",
            "model-chip",
            "effort-chip",
            "attach-images",
        ] {
            assert!(window.try_find(id).is_some(), "{id} did not render");
        }
        let composer_top = window.find("playground-composer").bounds().origin.y;

        // The model picker is a real in-place surface: it opens, fits above the
        // composer, lists the fixture models and exposes provider marks.
        window.click("model-chip", cx);
        assert!(desktop.read(cx).model_picker_open);
        window.render_frame(cx);
        for id in [
            "model-picker",
            "model-picker-list",
            "model-provider-muse",
            "choice-model-local",
        ] {
            assert!(window.try_find(id).is_some(), "{id} missing from picker");
        }
        let picker = window.find("model-picker").bounds();
        assert!(
            picker.origin.y + picker.size.height <= composer_top + px(1.),
            "model picker clipped behind the composer: {picker:?}"
        );

        // Provider tabs filter the list, including the Muse Code mark.
        window.click("model-provider-muse", cx);
        window.render_frame(cx);
        assert!(window.try_find("choice-model-muse").is_some());
        assert!(window.try_find("choice-model-local").is_none());
        window.click("model-provider-all", cx);
        window.render_frame(cx);
        window.click("choice-model-local", cx);
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).selected_model.as_deref(),
            Some("model-local")
        );
        assert!(!desktop.read(cx).model_picker_open);

        // The effort menu sizes to its options. It must not paint a full-height
        // empty card below the last row.
        window.click("effort-chip", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).effort_picker_open);
        for id in ["effort-picker", "effort-option-low", "effort-option-high"] {
            assert!(
                window.try_find(id).is_some(),
                "{id} missing from effort menu"
            );
        }
        let effort = window.find("effort-picker").bounds();
        let last = window.find("effort-option-high").bounds();
        assert!(
            effort.size.height < px(260.),
            "effort menu kept empty space: {effort:?}"
        );
        assert!(
            effort.origin.y + effort.size.height - (last.origin.y + last.size.height) < px(16.),
            "effort menu did not fit its content: menu {effort:?}, last row {last:?}"
        );
        window.click("effort-option-low", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).selected_effort.as_deref(), Some("low"));
        assert!(!desktop.read(cx).effort_picker_open);

        // Attachment chips can hold any file; the image/non-image split happens
        // when the turn starts.
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("notes.md");
        std::fs::write(&notes, "attached notes").unwrap();
        desktop.update(cx, |desktop, cx| {
            desktop.attach_files(vec![notes]);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("attachment-0").is_some());
        window.click("remove-attachment-0", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).attachments.is_empty());
    })
    .expect("drive the Playground view");

    // Standard chat keyboard: Enter sends, Shift+Enter adds a line, Escape
    // closes an open selector. Keystroke actions and input events are delivered
    // by the effect loop, so park it before asserting.
    let before = cx.update(|cx| desktop.read(cx).transcript.len());
    cx.update_window(handle, |_, window, cx| {
        window.click_at("playground-composer", point(px(80.), px(34.)), cx);
        window.input("keyboard send", cx);
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).prompt.read(cx).value(),
            "keyboard send",
            "typing did not reach the composer"
        );
        window.press("enter", cx);
    })
    .expect("press Enter in the composer");
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).transcript.len(),
            before + 1,
            "Enter did not submit the chat"
        );
        assert_eq!(
            desktop.read(cx).transcript.last().unwrap().text,
            "keyboard send"
        );
        assert_eq!(desktop.read(cx).prompt.read(cx).value(), "");
        window.click_at("playground-composer", point(px(80.), px(34.)), cx);
        window.input("first line", cx);
        window.press("shift-enter", cx);
    })
    .expect("press Shift+Enter in the composer");
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).transcript.len(),
            before + 1,
            "Shift+Enter submitted the chat"
        );
        assert!(
            desktop.read(cx).prompt.read(cx).value().ends_with('\n'),
            "Shift+Enter did not insert a newline: {:?}",
            desktop.read(cx).prompt.read(cx).value()
        );
        window.click("model-chip", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).model_picker_open);
        window.press("escape", cx);
    })
    .expect("press Escape with the selector open");
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(!desktop.read(cx).model_picker_open);
    })
    .expect("verify Escape closed the selector");
}

#[gpui_kit::test]
async fn native_playground_sends_the_selected_reasoning_variant(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, harness, desktop) = cx.update(|cx| {
        let bounds = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1280.), px(800.)),
        };
        let (window, harness) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let desktop = cx.new(|cx| {
                    Desktop::new(
                        window,
                        cx,
                        Credential::Unavailable("native e2e".into()),
                        30,
                        true,
                    )
                });
                cx.new(|_| PageHarness {
                    desktop: desktop.clone(),
                    page: Page::Playground,
                })
            },
        )
        .expect("open the Playground view");
        let desktop = harness.read(cx).desktop.clone();
        (window, harness, desktop)
    });
    cx.wait_for(handle, Duration::from_secs(3), |_, cx| {
        desktop.read(cx).bundle.is_some()
    })
    .await;

    let (commands_tx, commands_rx) = std::sync::mpsc::channel();
    let (_events_tx, events_rx) = std::sync::mpsc::channel();
    cx.update_window(handle, |_, window, cx| {
        desktop.update(cx, |desktop, cx| {
            desktop.bundle = Some(bundle());
            desktop.executor = ExecHandle {
                commands: commands_tx,
                events: events_rx,
            };
            desktop.selected_model = Some("model-local".into());
            desktop.selected_effort = Some("high".into());
            desktop.prompt.update(cx, |prompt, cx| {
                prompt.set_value("use high reasoning", window, cx)
            });
            cx.notify();
            desktop.run_prompt(window, cx);
        });
        harness.update(cx, |_, cx| cx.notify());
        window.render_frame(cx);
    })
    .expect("run a turn with a reasoning variant");

    let crate::broker::ExecCommand::Run(request) = commands_rx
        .try_recv()
        .expect("the turn should be sent to the executor")
    else {
        panic!("expected a run command");
    };
    assert_eq!(
        request.options.get("variant"),
        Some(&serde_json::json!("high")),
        "the OpenCode reasoning variant was not sent with the turn"
    );
}
