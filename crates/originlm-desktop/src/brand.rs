//! The supplied outer mark stays fixed; only its inner opening follows the pointer.
use gpui_kit::*;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Instant,
};

pub struct EyeLogo {
    size: f32,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    current: (f32, f32),
    target: (f32, f32),
    last_frame: Instant,
}
impl EyeLogo {
    pub fn new(size: f32) -> Self {
        Self {
            size,
            bounds: Rc::new(Cell::new(None)),
            current: (0., 0.),
            target: (0., 0.),
            last_frame: Instant::now(),
        }
    }
    pub fn aim(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        if cx.reduce_motion() {
            self.rest(cx);
            return;
        }
        let Some(bounds) = self.bounds.get() else {
            return;
        };
        let scale = bounds.size.width.as_f32() / 1024.;
        let center_x = bounds.origin.x.as_f32() + (178.953_85 + 3.249_230_9 * 119.) * scale;
        let center_y = bounds.origin.y.as_f32() + (174.08 + 3.249_230_9 * 87.) * scale;
        let dx = pointer.x.as_f32() - center_x;
        let dy = pointer.y.as_f32() - center_y;
        let distance = dx.hypot(dy);
        let strength = (distance / (205. * 3.249_230_9 * scale * 3.)).min(1.);
        let next = if distance > 0. {
            (
                dx / distance * 12. * strength,
                dy / distance * 10. * strength,
            )
        } else {
            (0., 0.)
        };
        if (next.0 - self.target.0).hypot(next.1 - self.target.1) > 0.01 {
            self.target = next;
            cx.notify();
        }
    }
    pub fn rest(&mut self, cx: &mut Context<Self>) {
        if self.target != (0., 0.) || self.current != (0., 0.) {
            self.target = (0., 0.);
            cx.notify();
        }
    }
}
fn icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/brand/originlm-app-256.png").to_vec(),
        ))
    })
    .clone()
}
fn paint_counter(bounds: Bounds<Pixels>, offset: (f32, f32), color: u32, window: &mut Window) {
    static CONTOUR: OnceLock<Vec<f32>> = OnceLock::new();
    let values = CONTOUR.get_or_init(|| {
        let geometry: serde_json::Value =
            serde_json::from_str(include_str!("../../../brand/logo-geometry.json"))
                .expect("supplied logo geometry");
        geometry["mark"]["paths"][1]["d"]
            .as_str()
            .expect("supplied inner contour")
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | 'M' | 'C' | 'Z'))
            .filter(|word| !word.is_empty())
            .map(|word| word.parse().expect("logo coordinate"))
            .collect()
    });
    let scale = bounds.size.width.as_f32() / 1024.;
    let point_at = |x: f32, y: f32| {
        point(
            bounds.origin.x + px((178.953_85 + (x + offset.0) * 3.249_230_9) * scale),
            bounds.origin.y + px((174.08 + (y + offset.1) * 3.249_230_9) * scale),
        )
    };
    let mut path = PathBuilder::fill();
    path.move_to(point_at(values[0], values[1]));
    for curve in values[2..].as_chunks::<6>().0 {
        path.cubic_bezier_to(
            point_at(curve[4], curve[5]),
            point_at(curve[0], curve[1]),
            point_at(curve[2], curve[3]),
        );
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, rgb(color));
    }
}
impl Render for EyeLogo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let alpha =
            1. - (-now.duration_since(self.last_frame).as_secs_f32().min(0.05) / 0.055).exp();
        self.last_frame = now;
        if cx.reduce_motion() {
            self.current = (0., 0.);
            self.target = (0., 0.);
        } else {
            self.current.0 += (self.target.0 - self.current.0) * alpha;
            self.current.1 += (self.target.1 - self.current.1) * alpha;
            if (self.target.0 - self.current.0).hypot(self.target.1 - self.current.1) < 0.02 {
                self.current = self.target;
            } else {
                window.request_animation_frame();
            }
        }
        let offset = self.current;
        let bounds = self.bounds.clone();
        div()
            .relative()
            .size(px(self.size))
            .flex_shrink_0()
            .child(img(icon()).size_full())
            .child(
                canvas(
                    move |rect, _, _| {
                        bounds.set(Some(rect));
                    },
                    move |rect, (), window, _| {
                        if offset != (0., 0.) {
                            // Fill the original aperture, then draw the same contour at its bounded offset.
                            paint_counter(rect, (0., 0.), 0xEBA31A, window);
                            paint_counter(rect, offset, 0x111820, window);
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

/// Read the OS preference once at explicit application startup, with an override for hosts.
pub fn preferred_reduce_motion() -> bool {
    if let Ok(value) = std::env::var("ORIGINLM_REDUCED_MOTION") {
        return matches!(value.as_str(), "1" | "true" | "yes");
    }
    #[cfg(target_os = "linux")]
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output();
    #[cfg(target_os = "macos")]
    let output = std::process::Command::new("defaults")
        .args(["read", "com.apple.universalaccess", "reduceMotion"])
        .output();
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell.exe")
            .creation_flags(0x0800_0000)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-ItemPropertyValue 'HKCU:\\Control Panel\\Desktop\\WindowMetrics' MinAnimate",
            ])
            .output()
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return false;
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    output
        .ok()
        .filter(|value| value.status.success())
        .is_some_and(|value| {
            let text = String::from_utf8_lossy(&value.stdout);
            if cfg!(target_os = "linux") {
                text.trim() == "false"
            } else if cfg!(target_os = "windows") {
                text.trim() == "0"
            } else {
                matches!(text.trim(), "1" | "true")
            }
        })
}
