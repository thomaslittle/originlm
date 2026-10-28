mod app;
mod brand;
mod brand_tokens;
mod broker;
mod data;
mod history;
mod human;
mod icons;
mod pages;
mod theme;
mod ui;

use app::Desktop;
use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::{borrow::Cow, path::PathBuf, sync::Arc};

fn parse_args() -> (PathBuf, u64, bool) {
    let mut token: Option<PathBuf> = None;
    let mut refresh = 30u64;
    let mut example = false;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--example" => example = true,
            "--token-file" => token = args.next().map(PathBuf::from),
            "--refresh" => {
                refresh = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|value: &u64| (5..=600).contains(value))
                    .unwrap_or(30);
            }
            "--help" | "-h" => {
                println!(
                    "originlm-desktop [--token-file PATH] [--refresh SECONDS] [--example]\n\
                     Reads the shared broker over its public authenticated API."
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    if example {
        return (PathBuf::from("example-mode-no-credential"), refresh, true);
    }
    let token = token
        .or_else(|| std::env::var_os("LAI_DESKTOP_TOKEN_FILE").map(PathBuf::from))
        .or_else(default_token_file)
        // No implicit credential lookup in the current working directory.
        .unwrap_or_default();
    (token, refresh, example)
}

fn default_token_file() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LAI_BROKER_TOKEN_FILE") {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    let state = PathBuf::from(home).join(".originlm");
    [
        state.join("broker-admin-token.json"),
        state.join("broker-token.json"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn main() {
    let (token_path, refresh, example) = parse_args();
    let application = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    application.run(move |cx| {
        let _ = cx
            .text_system()
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../assets/fonts/RedHatDisplay-Variable.ttf"
            ) as &[u8])]);
        gpui_kit::init(cx);
        theme::install(cx, true);
        let token = token_path.clone();
        let (width, height) = std::env::var("LAI_DESKTOP_WINDOW")
            .ok()
            .and_then(|value| {
                let (w, h) = value.split_once('x')?;
                let (width, height) = (w.parse::<f32>().ok()?, h.parse::<f32>().ok()?);
                (width.is_finite() && height.is_finite())
                    .then_some((width.clamp(1000., 7680.), height.clamp(660., 4320.)))
            })
            .unwrap_or((1440.0, 920.0));
        let light = std::env::var_os("LAI_DESKTOP_THEME").is_some_and(|value| value == "light");
        if light {
            theme::install(cx, false);
        }
        let icon = image::load_from_memory(include_bytes!("../assets/brand/originlm-app-256.png"))
            .ok()
            .map(|image| Arc::new(image.to_rgba8()));
        let mut options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(110.), px(70.)),
                size: size(px(width), px(height)),
            })),
            window_min_size: Some(size(px(1000.), px(660.))),
            ..TitleBar::window_options()
        };
        if let Some(titlebar) = options.titlebar.as_mut() {
            titlebar.title = Some("OriginLM".into());
        }
        options.icon = icon;
        gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| Desktop::new(window, cx, token, refresh, example))
        })
        .expect("desktop window");
    });
}
