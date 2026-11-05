//! Native end-to-end interaction tests for the real page views.
//!
//! The page is rendered in GPUI's deterministic test window and driven with
//! GPUI pointer input. This follows Zed's GPUI test pattern and GPUI Kit's
//! view tests: no Windows UI Automation, SendKeys, focus changes, screenshots
//! or OS-level input are involved.

use crate::app::{Desktop, Page};
use crate::bootstrap::Credential;
use crate::broker::Bundle;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AppContext as _, Bounds, Entity, InteractiveElement as _, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, point, px, size,
};
use originlm_contracts::*;
use std::time::Duration;

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
            _ => unreachable!("test harness only renders Overview and Models"),
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
