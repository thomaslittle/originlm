//! SPIKE: Longbridge NSH (`nsh-platform-gpui`) embedded in OriginLM as one
//! [`AppView`] on the Diagnostics page.
//!
//! Compatibility note: NSH's GPUI backend and `gpui-kit 0.9.0` (with
//! `gpui-fast`) both resolve to the same `gpui-fast 0.1.13`, so the `App`,
//! `Window` and `Entity` types are one type. See the draft PR for the
//! version graph.
//!
//! The panel is one TSX module (`nsh/ProviderCard.tsx`) compiled by
//! `build.rs` with `nsh-bundler` and loaded as *source* (`Bundle::source`):
//! bytecode loading is unsafe and this workspace forbids unsafe code. The
//! only host capability it declares is `host: ["getProviderCard"]`, which
//! the Rust side answers with hard-coded fixture JSON.

use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::{AppContext as _, Context, Entity, Window};
use nsh_platform_gpui::{AdaptiveColor, App, AppOptions, AppView, Appearance, Bundle};
use serde_json::json;

use crate::app::Desktop;

/// The bundled JS, built from `nsh/ProviderCard.tsx` by `build.rs`.
const PANEL_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/provider-card.js"));

/// Only the host capability the module uses (an app without a declaration
/// gets no host functions; a declaration is mandatory).
const CAPABILITIES: &str = r#"{"host":["getProviderCard"]}"#;

/// Fixture data for the card. No broker call and no provider request.
fn fixture() -> serde_json::Value {
    json!({
        "name": "Claude Code",
        "kind": "OAuth session, local installation",
        "statusLabel": "Connected",
        "models": 12,
        "usage": {
            "used": 382_000,
            "limit": 1_000_000,
            "resetsIn": "3d 4h"
        }
    })
}

/// GPUI Kit's active theme as NSH's shadcn tokens.
fn nsh_theme(theme: &Theme) -> nsh_platform_gpui::Theme {
    fn color(color: gpui_kit::Hsla) -> nsh_platform_gpui::Hsla {
        nsh_platform_gpui::Hsla {
            h: color.h,
            s: color.s,
            l: color.l,
            a: color.a,
        }
    }

    let mut nsh = nsh_platform_gpui::Theme::default();
    let colors = &theme.colors;
    for (name, value) in [
        // The surface sits inside a `ds::panel`, whose background is
        // `theme.popover`; make NSH's canvas and card match it.
        ("background", colors.popover),
        ("card", colors.popover),
        ("popover", colors.popover),
        ("card-foreground", colors.popover_foreground),
        ("popover-foreground", colors.popover_foreground),
        ("foreground", colors.foreground),
        ("muted", colors.muted),
        ("muted-foreground", colors.muted_foreground),
        ("primary", colors.primary),
        ("primary-foreground", colors.primary_foreground),
        ("secondary", colors.secondary),
        ("secondary-foreground", colors.secondary_foreground),
        ("accent", colors.accent),
        ("accent-foreground", colors.accent_foreground),
        ("destructive", colors.danger),
        ("success", colors.success),
        ("warning", colors.warning),
        ("border", colors.border),
        ("input", colors.input),
        ("ring", colors.ring),
    ] {
        nsh.set_color(name, AdaptiveColor::from(color(value)));
    }
    nsh.set_number("radius", f32::from(theme.radius));
    nsh
}

fn appearance(dark: bool) -> Appearance {
    if dark {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

/// Creates the surface, wires its host handler and loads the TSX bundle.
/// Called once, the first time the Diagnostics page renders.
pub fn create(window: &mut Window, cx: &mut Context<Desktop>, dark: bool) -> Entity<AppView> {
    let theme = nsh_theme(cx.theme());
    let appearance = appearance(dark);
    cx.new(|cx| {
        let view = AppView::new(AppOptions::default(), window, cx);
        view.host().handle("getProviderCard", |call| {
            call.resolve(fixture());
        });
        view.on_error(|message| eprintln!("nsh spike: {message}"));
        // Before the app starts: its first frame is in the host's theme.
        view.set_theme(Some(theme), cx);
        view.set_appearance(appearance, cx);
        view.load(App::Js(
            Bundle::source(PANEL_JS.to_owned())
                .with_capabilities_json(Some(CAPABILITIES))
                .expect("the NSH spike capabilities are valid JSON"),
        ));
        view
    })
}

/// Re-applies the host theme and light/dark appearance after a mode change.
pub fn apply_theme(view: &Entity<AppView>, cx: &mut Context<Desktop>, dark: bool) {
    let theme = nsh_theme(cx.theme());
    let appearance = appearance(dark);
    view.update(cx, |view, cx| {
        view.set_theme(Some(theme), cx);
        view.set_appearance(appearance, cx);
    });
}
