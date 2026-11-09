//! Explicit developer smoke harness. No UI automation or credential material.
use crate::app::{Desktop, Page};
use gpui_kit::Context;
use serde::Deserialize;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    prompt: String,
    provider_id: String,
    allow_paid: bool,
    report: PathBuf,
}
pub struct ImageSmoke {
    config: Config,
    started: Instant,
    submitted: bool,
    done: bool,
}
pub fn load() -> Option<ImageSmoke> {
    let path = std::env::var_os("ORIGINLM_DESKTOP_IMAGE_SMOKE")?;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 32768 {
        return None;
    }
    let config: Config = serde_json::from_slice(&bytes).ok()?;
    if !config.report.is_absolute() || config.prompt.len() > 16000 {
        return None;
    }
    Some(ImageSmoke {
        config,
        started: Instant::now(),
        submitted: false,
        done: false,
    })
}
impl ImageSmoke {
    pub fn tick(&mut self, app: &mut Desktop, cx: &mut Context<Desktop>) {
        if self.done {
            return;
        }
        app.page = Page::Playground;
        if !self.submitted
            && let Some(choice) = app
                .model_choices()
                .into_iter()
                .find(|c| c.provider_id == self.config.provider_id)
        {
            app.selected_model = Some(choice.model_id.clone());
            app.paid_consent
                .set(Some(&choice.model_id), self.config.allow_paid);
            app.submit_prompt(self.config.prompt.clone(), cx);
            self.submitted = true;
        }
        if (self.submitted && !app.running) || self.started.elapsed() > Duration::from_secs(180) {
            let images: Vec<_> = app
                .transcript
                .iter()
                .filter_map(|line| line.image.clone())
                .collect();
            let texts: Vec<_> = app
                .transcript
                .iter()
                .map(|line| line.text.clone())
                .collect();
            let value = serde_json::json!({"passed":!images.is_empty() && app.playground_error.is_none(),"images":images,"transcript":texts,"error":app.playground_error,"timedOut":app.running || !self.submitted});
            if let Ok(bytes) = serde_json::to_vec_pretty(&value) {
                let _ = std::fs::write(&self.config.report, bytes);
            }
            self.done = true;
        }
        cx.notify();
    }
}
