//! Native end-to-end interaction tests for the real page views.
//!
//! The page is rendered in GPUI's deterministic test window and driven with
//! GPUI pointer input. This follows Zed's GPUI test pattern and GPUI Kit's
//! view tests: no Windows UI Automation, SendKeys, focus changes, screenshots
//! or OS-level input are involved.

use crate::app::{Desktop, Page};
use crate::bootstrap::Credential;
use crate::broker::{Bundle, ExecCommand, ExecEvent, ExecHandle};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
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
    snapshot.models.push(model(
        "model-local",
        "route-local",
        "KAT-Coder",
        "kat-coder",
    ));
    snapshot.models.push(model(
        "model-codex",
        "route-codex",
        "GPT-6 Luna",
        "gpt-6-luna",
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
        intelligence: originlm_contracts::IntelligenceCatalog::default(),
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
        gpui_kit::div().id("page-harness").size_full().child(page)
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
                ExecCommand::Run {
                    prompt,
                    choice,
                    allow_paid,
                } => {
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
            desktop.run_prompt(cx);
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
            desktop.run_prompt(cx);
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

/// Drives the real composer and result event path with a mocked image provider.
/// Native CLI/provider live qualification remains separate.
#[gpui_kit::test]
async fn native_playground_ask_renders_trusted_generated_image(cx: &mut TestAppContext) {
    use originlm_kit::images::{
        Canvas, GenerateRequest, Generation, ImageProvider, ImageTool, ProviderRequest,
    };
    struct ImageFixture;
    impl ImageProvider for ImageFixture {
        fn provider(&self) -> &str {
            "mock"
        }
        fn model(&self) -> &str {
            "mock-image"
        }
        fn generate<'a>(&'a self, r: ProviderRequest) -> Generation<'a> {
            assert!(r.prompt.contains("Starting Soon"));
            Box::pin(async {
                let mut png = std::io::Cursor::new(Vec::new());
                image::DynamicImage::new_rgb8(160, 90)
                    .write_to(&mut png, image::ImageFormat::Png)
                    .unwrap();
                Ok(vec![png.into_inner()])
            })
        }
    }
    cx.update(gpui_kit::init);
    let assets = tempfile::tempdir().unwrap();
    let tool = ImageTool::new(
        Some(Arc::new(ImageFixture)),
        assets.path().to_path_buf(),
        Canvas {
            width: 160,
            height: 90,
        },
    )
    .unwrap();
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let ExecCommand::Run { prompt, .. } = command_rx.recv().unwrap() else {
            panic!("expected composer run")
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let images = runtime
            .block_on(tool.generate(GenerateRequest {
                prompt,
                size: None,
                aspect: None,
                style: None,
                reference_image: None,
                count: 1,
            }))
            .unwrap();
        event_tx.send(ExecEvent::Images { images }).unwrap();
        event_tx
            .send(ExecEvent::Delta {
                session_id: "mock-image-turn".into(),
                text: "Here is your backdrop. Would you like a variation?".into(),
            })
            .unwrap();
        event_tx
            .send(ExecEvent::Finished {
                session_id: "mock-image-turn".into(),
                status: "completed".into(),
            })
            .unwrap();
    });
    let executor = ExecHandle {
        commands: command_tx,
        events: event_rx,
    };
    let (handle, harness, desktop) = cx.update(|cx| {
        let (handle, harness) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(1440.), px(920.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let desktop = cx.new(|cx| {
                    Desktop::new(
                        window,
                        cx,
                        Credential::Unavailable("image fixture".into()),
                        30,
                        true,
                    )
                });
                desktop.update(cx, |desktop, cx| {
                    desktop.executor = executor;
                    desktop.page = Page::Playground;
                    desktop.viewport_w = 1440.;
                    desktop.bundle = Some(bundle());
                    cx.notify();
                });
                cx.new(|_| PageHarness {
                    desktop: desktop.clone(),
                    page: Page::Playground,
                })
            },
        )
        .unwrap();
        let desktop = harness.read(cx).desktop.clone();
        (handle, harness, desktop)
    });
    cx.update_window(handle, |_, window, cx| {
        desktop.update(cx, |desktop, cx| {
            desktop.bundle = Some(bundle());
            desktop.selected_model = Some("model-local".into());
            desktop.prompt.update(cx, |prompt, cx| {
                prompt.set_value("Draw a Starting Soon backdrop", window, cx)
            });
            desktop.run_prompt(cx);
        });
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(5), |_, cx| {
        desktop.read(cx).transcript.len() >= 3 && !desktop.read(cx).running
    })
    .await;
    cx.update_window(handle, |_, window, cx| {
        harness.update(cx, |_, cx| cx.notify());
        window.render_frame(cx);
        assert!(
            window
                .find("generated-image-1")
                .label()
                .unwrap_or_default()
                .contains("mock-image")
        );
        let desktop = desktop.read(cx);
        assert!(desktop.transcript[1].image.as_ref().unwrap().path.is_file());
        assert!(desktop.transcript[1].text.contains("160×90"));
        assert_eq!(
            desktop.transcript[2].text,
            "Here is your backdrop. Would you like a variation?"
        );
    })
    .unwrap();
    worker.join().unwrap();
}
