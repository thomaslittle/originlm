use crate::broker::{self, BrokerHandle, Bundle, ExecEvent, ExecHandle, ModelChoice};
use crate::history::Sample;
use crate::theme;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme, IconName, Theme, TitleBar,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, TextareaState},
    resizable::{ResizableState, h_resizable, resizable_panel},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};

actions!(
    originlm_desktop,
    [Quit, OpenPalette, ClosePalette, RefreshNow, ToggleTheme]
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Providers,
    Models,
    Usage,
    Apps,
    Sessions,
    Playground,
    Guide,
    Diagnostics,
    Settings,
}

impl Page {
    pub const ALL: [Page; 10] = [
        Page::Overview,
        Page::Providers,
        Page::Models,
        Page::Usage,
        Page::Apps,
        Page::Sessions,
        Page::Playground,
        Page::Guide,
        Page::Diagnostics,
        Page::Settings,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Providers => "Providers & accounts",
            Page::Models => "Models",
            Page::Usage => "Usage",
            Page::Apps => "Connected apps",
            Page::Sessions => "Sessions",
            Page::Playground => "Playground",
            Page::Guide => "How to use",
            Page::Diagnostics => "Diagnostics",
            Page::Settings => "Settings",
        }
    }
    pub fn subtitle(self) -> &'static str {
        match self {
            Page::Overview => "What you can use right now, and what needs attention.",
            Page::Providers => "Installations, profiles and the accounts behind them.",
            Page::Models => "Provider-native catalogs, capabilities and options.",
            Page::Usage => "Allowance used over time, what remains, and when it resets.",
            Page::Apps => "Applications connected to this broker and their permissions.",
            Page::Sessions => "Work this broker is managing for this app.",
            Page::Playground => "One explicit test turn. Nothing runs in the background.",
            Page::Guide => "Broker API, client setup, and the components this app is built from.",
            Page::Diagnostics => "Plain-language failures, freshness and a redacted export.",
            Page::Settings => "Appearance, data source and refresh cadence.",
        }
    }
    pub fn icon(self) -> IconName {
        match self {
            Page::Overview => IconName::LayoutDashboard,
            Page::Providers => IconName::Bot,
            Page::Models => IconName::Cpu,
            Page::Usage => IconName::ChartPie,
            Page::Apps => IconName::SquareTerminal,
            Page::Sessions => IconName::Inbox,
            Page::Playground => IconName::Play,
            Page::Guide => IconName::BookOpen,
            Page::Diagnostics => IconName::CircleAlert,
            Page::Settings => IconName::Settings,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Range {
    Day,
    Week,
    Month,
}

impl Range {
    pub fn label(self) -> &'static str {
        match self {
            Range::Day => "24h",
            Range::Week => "7d",
            Range::Month => "30d",
        }
    }
    pub fn seconds(self) -> i64 {
        match self {
            Range::Day => 86_400,
            Range::Week => 604_800,
            Range::Month => 2_592_000,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TranscriptRole {
    You,
    Assistant,
}

#[derive(Clone)]
pub struct TranscriptLine {
    pub role: TranscriptRole,
    pub text: String,
}

#[derive(Clone)]
pub struct UiSession {
    pub id: String,
    pub label: String,
    pub status: String,
    pub started: Instant,
    pub usage: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Prefs {
    dark: bool,
    refresh_seconds: u64,
    favorites: Vec<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default = "default_sidebar_width")]
    sidebar_width: f32,
    #[serde(default)]
    sidebar_collapsed: bool,
    #[serde(default = "default_inspector_width")]
    models_inspector: f32,
    #[serde(default = "default_inspector_width")]
    providers_inspector: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: true,
            refresh_seconds: 30,
            favorites: Vec::new(),
            page: None,
            sidebar_width: default_sidebar_width(),
            models_inspector: default_inspector_width(),
            providers_inspector: default_inspector_width(),
            sidebar_collapsed: false,
        }
    }
}

fn prefs_path() -> Option<PathBuf> {
    crate::history::state_dir().map(|path| path.join("prefs.json"))
}

fn load_prefs() -> Prefs {
    prefs_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub struct Desktop {
    pub page: Page,
    pub brand_marks: [Entity<crate::brand::EyeLogo>; 3],
    pub bundle: Option<Bundle>,
    pub dark: bool,
    pub refresh_seconds: u64,
    pub error: Option<String>,
    pub connected: bool,
    pub example: bool,
    pub last_event: Instant,
    pub palette_open: bool,
    pub palette_input: Entity<InputState>,
    pub palette_query: String,
    pub search: Entity<InputState>,
    pub prompt: Entity<TextareaState>,
    pub price_fields: [Entity<InputState>; 6],
    pub price_edit_original: Option<originlm_contracts::ModelPrice>,
    pub query: String,
    pub selected_connection: Option<String>,
    pub selected_model: Option<String>,
    pub selected_series: Option<String>,
    pub range: Range,
    pub usage_table: bool,
    pub favorites: BTreeSet<String>,
    pub sessions: Vec<UiSession>,
    pub transcript: Vec<TranscriptLine>,
    pub running: bool,
    pub active_session: Option<String>,
    pub active_turn: Option<String>,
    pub usage_note: Option<String>,
    pub playground_error: Option<String>,
    pub revoke_confirm: Option<String>,
    pub toast: Option<(String, Instant)>,
    pub mini_view: bool,
    pub broker: BrokerHandle,
    pub executor: ExecHandle,
    pub token_path: PathBuf,
    pub poll_task: Option<Task<()>>,
    focus: FocusHandle,
    initial_focus: bool,
    startup_probes: BTreeSet<String>,
    sidebar_state: Entity<ResizableState>,
    pub models_split: Entity<ResizableState>,
    pub providers_split: Entity<ResizableState>,
    pub models_inspector: f32,
    pub providers_inspector: f32,
    pub sidebar_width: f32,
    pub sidebar_collapsed: bool,
    pub last_layout_save: Instant,
    sidebar_animation: u64,
    _subscriptions: Vec<Subscription>,
}

impl Desktop {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        token_path: PathBuf,
        refresh_seconds: u64,
        example: bool,
    ) -> Self {
        let prefs = load_prefs();
        let dark = match std::env::var("LAI_DESKTOP_THEME").ok().as_deref() {
            Some("light") => false,
            Some("dark") => true,
            _ => prefs.dark,
        };
        theme::install(cx, dark);
        let broker = broker::spawn(
            token_path.clone(),
            Duration::from_secs(refresh_seconds),
            example,
        );
        let executor = broker::spawn_executor(token_path.clone(), example);
        let palette_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Jump to a page, model or action…"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search models…"));
        let price_fields = std::array::from_fn(|index| {
            cx.new(|cx| {
                InputState::new(window, cx).placeholder(
                    [
                        "Provider ID",
                        "Exact model ID",
                        "Input USD per million",
                        "Output USD per million",
                        "YYYY-MM-DD",
                        "Source or local rationale",
                    ][index],
                )
            })
        });
        let prompt = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Describe a small test request. Nothing runs until you press Run.")
        });

        let focus = cx.focus_handle();
        let sidebar_state = cx.new(|_| ResizableState::default());
        let models_split = cx.new(|_| ResizableState::default());
        let providers_split = cx.new(|_| ResizableState::default());
        let sidebar_width = prefs.sidebar_width.clamp(190., 340.);
        let brand_marks = [
            cx.new(|_| crate::brand::EyeLogo::new(32.)),
            cx.new(|_| crate::brand::EyeLogo::new(30.)),
            cx.new(|_| crate::brand::EyeLogo::new(30.)),
        ];
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(&palette_input, window, {
            let input = palette_input.clone();
            move |this: &mut Desktop, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.palette_query = input.read(cx).value().to_string();
                    cx.notify();
                }
                if let InputEvent::PressEnter { .. } = event {
                    this.activate_palette(window, cx);
                }
            }
        }));
        subscriptions.push(cx.subscribe_in(&search, window, {
            let input = search.clone();
            move |this: &mut Desktop, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = input.read(cx).value().to_string();
                    let query = this.query.to_lowercase();
                    let found = this.snapshot().and_then(|snapshot| {
                        snapshot
                            .models
                            .iter()
                            .find(|model| {
                                model.name.to_lowercase().contains(&query)
                                    || model.native_id.to_lowercase().contains(&query)
                            })
                            .map(|model| model.id.clone())
                    });
                    if let Some(model) = found {
                        this.selected_model = Some(model);
                    }
                    cx.notify();
                }
            }
        }));

        let mut this = Self {
            example,
            page: std::env::var("LAI_DESKTOP_PAGE")
                .ok()
                .and_then(|name| {
                    Page::ALL
                        .into_iter()
                        .find(|page| page.title().eq_ignore_ascii_case(&name))
                })
                .or_else(|| {
                    prefs.page.as_deref().and_then(|name| {
                        Page::ALL
                            .into_iter()
                            .find(|page| page.title().eq_ignore_ascii_case(name))
                    })
                })
                .unwrap_or(Page::Overview),
            brand_marks,
            bundle: None,
            dark,
            refresh_seconds,
            error: None,
            connected: false,
            last_event: Instant::now(),
            palette_open: false,
            palette_input,
            palette_query: String::new(),
            search,
            prompt,
            price_fields,
            price_edit_original: None,
            query: String::new(),
            selected_connection: None,
            selected_model: None,
            selected_series: None,
            range: Range::Day,
            usage_table: false,
            favorites: prefs.favorites.into_iter().collect(),
            sessions: Vec::new(),
            transcript: Vec::new(),
            running: false,
            active_session: None,
            active_turn: None,
            usage_note: None,
            playground_error: None,
            revoke_confirm: None,
            toast: None,
            mini_view: example && std::env::var_os("LAI_DESKTOP_MINI_VIEW").is_some(),
            broker,
            executor,
            token_path,
            poll_task: None,
            focus,
            initial_focus: true,
            startup_probes: BTreeSet::new(),
            sidebar_state,
            models_split,
            providers_split,
            models_inspector: prefs.models_inspector.clamp(320., 620.),
            providers_inspector: prefs.providers_inspector.clamp(320., 620.),
            sidebar_width,
            sidebar_collapsed: prefs.sidebar_collapsed,
            last_layout_save: Instant::now() - Duration::from_secs(10),
            sidebar_animation: 0,
            _subscriptions: subscriptions,
        };
        window.focus(&this.focus, cx);
        this.poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.drain(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        this
    }

    fn drain(&mut self, cx: &mut Context<Self>) {
        if std::env::var_os("LAI_DEBUG").is_some() {
            eprintln!("[debug] drain tick");
        }
        let mut changed = false;
        loop {
            match self.broker.updates.try_recv() {
                Ok(bundle) => {
                    if self.selected_connection.is_none() {
                        self.selected_connection = bundle
                            .snapshot
                            .connections
                            .iter()
                            .find(|c| c.auth.is_known_authenticated())
                            .or_else(|| bundle.snapshot.connections.first())
                            .map(|c| c.id.clone());
                    }
                    if self.selected_model.is_none() {
                        self.selected_model = preferred_model(&bundle.snapshot);
                    }
                    if self.selected_series.is_none() {
                        self.selected_series = crate::data::quota_rows(&bundle.snapshot)
                            .into_iter()
                            .filter(|row| row.remaining.is_some())
                            .max_by(|a, b| {
                                a.remaining
                                    .unwrap_or(0.0)
                                    .partial_cmp(&b.remaining.unwrap_or(0.0))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|row| format!("{}\u{1f}{}", row.pool_id, row.window_id));
                    }
                    self.connected = bundle.error.is_none();
                    self.error = bundle.error.clone();
                    self.bundle = Some(bundle);
                    self.last_event = Instant::now();
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        loop {
            match self.executor.events.try_recv() {
                Ok(event) => {
                    changed = true;
                    match event {
                        ExecEvent::Started {
                            session_id,
                            turn_id,
                            label,
                        } => {
                            self.running = true;
                            self.active_session = Some(session_id.clone());
                            self.active_turn = Some(turn_id);
                            self.sessions.insert(
                                0,
                                UiSession {
                                    id: session_id,
                                    label,
                                    status: "running".into(),
                                    started: Instant::now(),
                                    usage: None,
                                },
                            );
                        }
                        ExecEvent::Delta { session_id, text } => {
                            if let Some(last) = self.transcript.last_mut() {
                                if last.role == TranscriptRole::Assistant {
                                    last.text.push_str(&text);
                                } else {
                                    self.transcript.push(TranscriptLine {
                                        role: TranscriptRole::Assistant,
                                        text,
                                    });
                                }
                            }
                            if let Some(session) =
                                self.sessions.iter_mut().find(|s| s.id == session_id)
                            {
                                session.status = "streaming".into();
                            }
                        }
                        ExecEvent::Usage {
                            session_id,
                            description,
                        } => {
                            self.usage_note = Some(description.clone());
                            if let Some(session) =
                                self.sessions.iter_mut().find(|s| s.id == session_id)
                            {
                                session.usage = Some(description);
                            }
                        }
                        ExecEvent::Finished { session_id, status } => {
                            self.running = false;
                            self.active_turn = None;
                            if let Some(session) =
                                self.sessions.iter_mut().find(|s| s.id == session_id)
                            {
                                session.status = status;
                            }
                        }
                        ExecEvent::Failed { error } => {
                            self.running = false;
                            self.playground_error = Some(error.clone());
                            if let Some(session) = self
                                .active_session
                                .as_ref()
                                .and_then(|id| self.sessions.iter_mut().find(|s| &s.id == id))
                            {
                                session.status = "failed".into();
                            }
                            self.toast = Some((error, Instant::now()));
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        if changed {
            cx.notify();
        }
        // One bounded metadata probe per authenticated connection that has no
        // quota pool in the cached snapshot. This restores usage after a broker
        // restart without polling or starting any inference.
        let mut missing = Vec::new();
        if let Some(bundle) = &self.bundle {
            for connection in &bundle.snapshot.connections {
                let verified = matches!(
                    connection.auth,
                    originlm_contracts::Observation::Known {
                        value: originlm_contracts::AuthState::Authenticated,
                        ..
                    }
                );
                let pooled = bundle
                    .snapshot
                    .quota_pools
                    .iter()
                    .any(|pool| pool.connection_ids.contains(&connection.id));
                if verified && !pooled && self.startup_probes.insert(connection.id.clone()) {
                    missing.push(connection.id.clone());
                }
            }
        }
        for id in missing {
            let _ = self.broker.commands.send(broker::Command::Probe(id));
        }
    }

    pub fn snapshot(&self) -> Option<&originlm_contracts::Snapshot> {
        self.bundle.as_ref().map(|b| &b.snapshot)
    }

    fn expanded_sidebar_width(&self) -> Pixels {
        px(self.sidebar_width.clamp(190., 340.))
    }

    pub fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = self
            .sidebar_state
            .read(cx)
            .sizes()
            .first()
            .copied()
            .unwrap_or_else(|| self.expanded_sidebar_width());
        let collapsed = from < px(120.);
        self.sidebar_collapsed = !collapsed;
        let target = if collapsed {
            self.expanded_sidebar_width()
        } else {
            px(48.)
        };
        self.sidebar_animation = self.sidebar_animation.wrapping_add(1);
        let generation = self.sidebar_animation;
        let state = self.sidebar_state.clone();
        let handle = window.window_handle();
        self.save_prefs();
        cx.spawn(async move |this, cx| {
            let start = Instant::now();
            loop {
                let elapsed = start.elapsed().as_secs_f32();
                let progress = (elapsed / 0.17).clamp(0.0, 1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
                let alive = this
                    .update(cx, |this, _| this.sidebar_animation == generation)
                    .unwrap_or(false);
                if !alive {
                    break;
                }
                let value = from.as_f32() + (target.as_f32() - from.as_f32()) * eased;
                let _ = handle.update(cx, |_, window, cx| {
                    state.update(cx, |state, cx| state.resize_panel(0, px(value), window, cx));
                });
                if progress >= 1.0 {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            }
        })
        .detach();
        cx.notify();
    }

    pub fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_width = default_sidebar_width();
        self.sidebar_collapsed = false;
        self.models_inspector = default_inspector_width();
        self.providers_inspector = default_inspector_width();
        self.sidebar_state.update(cx, |state, cx| {
            state.resize_panel(0, px(self.sidebar_width), window, cx)
        });
        self.models_split.update(cx, |state, cx| {
            state.resize_panel(1, px(self.models_inspector), window, cx)
        });
        self.providers_split.update(cx, |state, cx| {
            state.resize_panel(1, px(self.providers_inspector), window, cx)
        });
        self.save_prefs();
        self.toast("Layout reset");
        cx.notify();
    }

    pub fn save_prefs(&self) {
        let prefs = Prefs {
            dark: self.dark,
            refresh_seconds: self.refresh_seconds,
            favorites: self.favorites.iter().cloned().collect(),
            page: Some(self.page.title().to_string()),
            sidebar_width: self.sidebar_width,
            sidebar_collapsed: self.sidebar_collapsed,
            models_inspector: self.models_inspector,
            providers_inspector: self.providers_inspector,
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&prefs) {
            let Some(path) = prefs_path() else {
                return;
            };
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, bytes);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let _ = self.broker.commands.send(broker::Command::Refresh);
        cx.notify();
    }

    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.palette_open = false;
        self.revoke_confirm = None;
        self.save_prefs();
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.dark = !self.dark;
        theme::install(cx, self.dark);
        self.save_prefs();
        cx.notify();
    }

    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        let handle = self.palette_input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    pub fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette_open = false;
        cx.notify();
    }

    pub fn toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    fn activate_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.palette_query.to_lowercase();
        if query.contains("sidebar") {
            self.toggle_sidebar(window, cx);
            self.palette_open = false;
            return;
        }
        if query.contains("layout") {
            self.reset_layout(window, cx);
            self.palette_open = false;
            return;
        }
        if let Some(page) = Page::ALL
            .into_iter()
            .find(|page| page.title().to_lowercase().contains(&query))
        {
            self.set_page(page, cx);
            return;
        }
        if let Some(model) = self.snapshot().and_then(|snapshot| {
            snapshot
                .models
                .iter()
                .find(|m| m.name.to_lowercase().contains(&query))
                .map(|m| m.id.clone())
        }) {
            self.selected_model = Some(model);
            self.set_page(Page::Models, cx);
        }
    }

    // ---- data helpers shared by pages ------------------------------------

    pub fn provider_name(&self, id: &str) -> String {
        self.snapshot()
            .and_then(|snapshot| snapshot.providers.iter().find(|p| p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn model_choices(&self) -> Vec<ModelChoice> {
        let Some(snapshot) = self.snapshot() else {
            return Vec::new();
        };
        let mut ranked: Vec<(i32, ModelChoice)> = snapshot
            .models
            .iter()
            .filter_map(|model| {
                let route = snapshot.routes.iter().find(|r| r.id == model.route_id)?;
                // OpenCode 2 and Cursor are read for metadata only; their text
                // execution clients are not wired, so do not offer them as
                // runnable Playground routes.
                if matches!(
                    route.provider_id.as_str(),
                    "opencode" | "opencode-go" | "cursor"
                ) {
                    return None;
                }
                let connection = snapshot
                    .connections
                    .iter()
                    .find(|c| c.id == route.connection_id)?;
                let authenticated = data_auth(connection);
                let (billing, fresh) = crate::data::billing(route);
                let rank = if authenticated { 0 } else { 100 }
                    + if (billing.starts_with("subscription") || billing == "local") && fresh {
                        0
                    } else if billing == "unknown" {
                        20
                    } else {
                        10
                    }
                    + if model.is_default { 0 } else { 1 };
                Some((
                    rank,
                    ModelChoice {
                        route_id: route.id.clone(),
                        model_id: model.id.clone(),
                        revision: model.revision.clone(),
                        label: model.name.clone(),
                        connection: crate::human::without_provider_prefix(
                            &connection.display_name,
                            &self.provider_name(&route.provider_id),
                        ),
                        provider: self.provider_name(&route.provider_id),
                        provider_id: route.provider_id.clone(),
                        billing: crate::ui::billing_label(route).to_string(),
                    },
                ))
            })
            .collect();
        ranked.sort_by_key(|(rank, choice)| (*rank, choice.label.clone()));
        ranked.into_iter().map(|(_, choice)| choice).collect()
    }

    pub fn selected_choice(&self) -> Option<ModelChoice> {
        let id = self.selected_model.as_ref()?;
        self.model_choices().into_iter().find(|c| &c.model_id == id)
    }

    pub fn run_prompt(&mut self, cx: &mut Context<Self>) {
        if std::env::var_os("LAI_DEBUG").is_some() {
            eprintln!("[debug] run_prompt invoked");
        }
        if self.running {
            return;
        }
        let prompt = self.prompt.read(cx).value().to_string();
        if prompt.trim().is_empty() {
            self.playground_error = Some("Write a prompt before running a turn.".into());
            cx.notify();
            return;
        }
        let Some(choice) = self.selected_choice() else {
            self.playground_error =
                Some("Choose a model first so the route and billing are explicit.".into());
            cx.notify();
            return;
        };
        self.playground_error = None;
        self.usage_note = None;
        self.transcript.clear();
        self.running = true;
        self.transcript.push(TranscriptLine {
            role: TranscriptRole::You,
            text: prompt.clone(),
        });
        let _ = self
            .executor
            .commands
            .send(broker::ExecCommand::Run { prompt, choice });
        cx.notify();
    }

    pub fn cancel_turn(&mut self, cx: &mut Context<Self>) {
        let _ = self.executor.commands.send(broker::ExecCommand::Cancel);
        cx.notify();
    }

    pub fn history(&self) -> &[Sample] {
        self.bundle
            .as_ref()
            .map(|b| b.history.as_slice())
            .unwrap_or(&[])
    }

    pub fn series_key(&self) -> Option<(String, String)> {
        self.selected_series
            .as_ref()
            .and_then(|key| key.split_once('\u{1f}'))
            .map(|(pool, window)| (pool.to_string(), window.to_string()))
    }

    pub fn counts(&self) -> Counts {
        let snapshot = self.snapshot();
        let connections = snapshot.map(|s| s.connections.len()).unwrap_or(0);
        let authenticated = snapshot
            .map(|s| {
                s.connections
                    .iter()
                    .filter(|c| c.auth.is_known_authenticated())
                    .count()
            })
            .unwrap_or(0);
        let cached = snapshot
            .map(|s| {
                s.connections
                    .iter()
                    .filter(|c| crate::data::auth_view(c).kind == crate::data::AuthKind::Cached)
                    .count()
            })
            .unwrap_or(0);
        Counts {
            providers: snapshot.map(|s| s.providers.len()).unwrap_or(0),
            connections,
            authenticated,
            models: snapshot.map(|s| s.models.len()).unwrap_or(0),
            apps: self.bundle.as_ref().map(|b| b.apps.len()).unwrap_or(0),
            cached,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub providers: usize,
    pub connections: usize,
    pub authenticated: usize,
    pub models: usize,
    pub apps: usize,
    pub cached: usize,
}

pub trait AuthExt {
    fn is_known_authenticated(&self) -> bool;
}

impl AuthExt for originlm_contracts::Observation<originlm_contracts::AuthState> {
    fn is_known_authenticated(&self) -> bool {
        matches!(
            self,
            originlm_contracts::Observation::Known {
                value: originlm_contracts::AuthState::Authenticated,
                freshness: originlm_contracts::Freshness::Fresh,
                ..
            }
        )
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        if self.initial_focus {
            self.initial_focus = false;
            window.focus(&self.focus, cx);
        }
        let client_decorated = matches!(
            window.window_decorations(),
            gpui::Decorations::Client { .. }
        );

        // Expire toasts after a few seconds without a timer: the poll loop
        // notifies regularly, so this is refreshed with the next update.
        if let Some((_, at)) = &self.toast
            && at.elapsed() > Duration::from_secs(6)
        {
            self.toast = None;
        }

        let page_body: AnyElement = match self.page {
            Page::Overview => self.page_overview(cx),
            Page::Providers => self.page_providers(cx),
            Page::Models => self.page_models(cx),
            Page::Usage => self.page_usage(cx),
            Page::Apps => self.page_apps(cx),
            Page::Sessions => self.page_sessions(cx),
            Page::Playground => self.page_playground(cx),
            Page::Guide => self.page_guide(cx),
            Page::Diagnostics => self.page_diagnostics(cx),
            Page::Settings => self.page_settings(cx),
        };

        let entity_keys = entity.clone();
        let tracking_marks = self.brand_marks.clone();
        let resting_marks = self.brand_marks.clone();
        div()
            .v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .track_focus(&self.focus)
            .on_mouse_move(move |event, _, cx| {
                for mark in &tracking_marks {
                    mark.update(cx, |eye, cx| eye.aim(event.position, cx));
                }
            })
            .on_mouse_exit(move |_, _, cx| {
                for mark in &resting_marks {
                    mark.update(cx, |eye, cx| eye.rest(cx));
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let command =
                    event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                let key = event.keystroke.key.to_lowercase();
                if key == "escape" {
                    entity_keys.update(cx, |this, cx| {
                        if this.palette_open {
                            this.close_palette(cx);
                        }
                    });
                } else if command && key == "k" {
                    entity_keys.update(cx, |this, cx| this.open_palette(window, cx));
                } else if command && key == "r" {
                    entity_keys.update(cx, |this, cx| this.refresh(cx));
                } else if command && key == "t" {
                    entity_keys.update(cx, |this, cx| this.toggle_theme(cx));
                } else if command && key == "b" {
                    entity_keys.update(cx, |this, cx| this.toggle_sidebar(window, cx));
                } else if command && event.keystroke.modifiers.shift && key == "m" {
                    entity_keys.update(cx, |this, cx| {
                        this.mini_view = !this.mini_view;
                        cx.notify();
                    });
                }
            })
            .child(self.window_title_bar(&theme, entity.clone(), client_decorated))
            .child({
                let sidebar_width = self
                    .sidebar_state
                    .read(cx)
                    .sizes()
                    .first()
                    .copied()
                    .unwrap_or_else(|| self.expanded_sidebar_width());
                let collapsed = sidebar_width < px(120.);
                let entity_resize = entity.clone();
                let content = div()
                    .v_flex()
                    .flex_1()
                    .w_full()
                    .min_w(px(0.))
                    .h_full()
                    .child(self.header(&theme, entity.clone()))
                    .children(self.connection_banner(&theme))
                    .child(
                        div()
                            .id("page-scroll")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .child(page_body),
                    );
                div().flex_1().min_h(px(0.)).child(
                    h_resizable("workspace")
                        .with_state(&self.sidebar_state)
                        .on_resize(move |state, _, cx| {
                            let width = state.read(cx).sizes().first().copied();
                            entity_resize.update(cx, |this, cx| {
                                if let Some(width) = width {
                                    if width >= px(120.) {
                                        this.sidebar_width = width.as_f32();
                                    }
                                    if this.last_layout_save.elapsed() > Duration::from_millis(400)
                                    {
                                        this.last_layout_save = Instant::now();
                                        this.save_prefs();
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .child(
                            resizable_panel()
                                .size(px(if self.sidebar_collapsed {
                                    48.
                                } else {
                                    self.sidebar_width
                                }))
                                .size_range(px(48.)..px(340.))
                                .child(self.sidebar(&theme, entity.clone(), collapsed)),
                        )
                        .child(content.into_any_element()),
                )
            })
            .child(self.status_bar(&theme, entity.clone()))
            .children(self.palette_overlay(&theme, entity.clone(), window, cx))
            .children(self.mini_view_overlay(&theme, entity.clone()))
            .children(self.toast_overlay(&theme))
    }
}

impl Desktop {
    fn window_title_bar(
        &self,
        theme: &Theme,
        entity: Entity<Self>,
        client_decorated: bool,
    ) -> AnyElement {
        let control =
            |id: &'static str, icon: IconName, danger: bool, action: fn(&mut Window, &mut App)| {
                div()
                    .id(id)
                    .w(px(34.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(move |style| {
                        if danger {
                            style.bg(theme.danger).text_color(theme.danger_foreground)
                        } else {
                            style.bg(theme.secondary_hover).text_color(theme.foreground)
                        }
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        action(window, cx);
                    })
                    .child(ui::icon(icon, 12., theme.muted_foreground))
            };

        TitleBar::new()
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .flex_1()
                    .gap_3()
                    .child({
                        let entity = entity.clone();
                        Button::new("titlebar-sidebar")
                            .ghost()
                            .compact()
                            .icon(IconName::PanelLeft)
                            .tooltip("Toggle sidebar (Ctrl+B)")
                            .on_click(move |_, window, cx| {
                                entity.update(cx, |this, cx| this.toggle_sidebar(window, cx));
                            })
                    })
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child(self.page.title()),
                    )
                    .child(div().flex_1())
                    .when(!client_decorated, |this| {
                        this.child(
                            div()
                                .h_flex()
                                .items_center()
                                .child(control(
                                    "window-min",
                                    IconName::WindowMinimize,
                                    false,
                                    |window, _| window.minimize_window(),
                                ))
                                .child(control(
                                    "window-max",
                                    IconName::WindowMaximize,
                                    false,
                                    |window, _| window.zoom_window(),
                                ))
                                .child(control(
                                    "window-close",
                                    IconName::WindowClose,
                                    true,
                                    |window, cx| {
                                        window.remove_window();
                                        let _ = cx;
                                    },
                                )),
                        )
                    }),
            )
            .into_any_element()
    }

    fn sidebar_rail(&self, theme: &Theme, entity: Entity<Self>) -> AnyElement {
        div()
            .v_flex()
            .w_full()
            .h_full()
            .items_center()
            .gap_1()
            .py_3()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(self.brand_marks[1].clone())
            .child(div().h(px(10.)))
            .children(Page::ALL.into_iter().map(|page| {
                let selected = self.page == page;
                let entity = entity.clone();
                div()
                    .id(("rail-page", page as usize))
                    .size(px(34.))
                    .rounded(px(7.))
                    .h_flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(selected, |this| this.bg(theme.sidebar_accent))
                    .when(!selected, |this| {
                        this.hover(|style| style.bg(theme.sidebar_accent.opacity(0.6)))
                    })
                    .child(ui::icon(
                        page.icon(),
                        15.,
                        if selected {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        },
                    ))
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.set_page(page, cx);
                            window.refresh();
                        });
                    })
                    .into_any_element()
            }))
            .child(div().flex_1())
            .child({
                let entity = entity.clone();
                Button::new("expand-sidebar")
                    .ghost()
                    .icon(IconName::PanelLeftOpen)
                    .tooltip("Show sidebar")
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.toggle_sidebar(window, cx));
                    })
            })
            .into_any_element()
    }

    fn sidebar(&self, theme: &Theme, entity: Entity<Self>, collapsed: bool) -> AnyElement {
        if collapsed {
            return self.sidebar_rail(theme, entity);
        }
        let nav_group = |label: &'static str, pages: &[Page]| {
            div()
                .v_flex()
                .gap(px(1.))
                .child(
                    div()
                        .px_3()
                        .pt_3()
                        .pb_1()
                        .text_size(px(10.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.muted_foreground.opacity(0.8))
                        .child(label),
                )
                .children(pages.iter().map(|page| {
                    let selected = self.page == *page;
                    let page = *page;
                    let entity = entity.clone();
                    div()
                        .id(("nav", page as usize))
                        .h_flex()
                        .gap_2()
                        .mx_2()
                        .px_2()
                        .py(px(6.))
                        .rounded(px(7.))
                        .cursor_pointer()
                        .when(selected, |this| {
                            this.bg(theme.sidebar_accent).text_color(theme.foreground)
                        })
                        .when(!selected, |this| {
                            this.text_color(theme.muted_foreground).hover(|s| {
                                s.bg(theme.sidebar_accent.opacity(0.55))
                                    .text_color(theme.foreground)
                            })
                        })
                        .child(ui::icon(
                            page.icon(),
                            14.,
                            if selected {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            },
                        ))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(if selected {
                                    FontWeight::MEDIUM
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(page.title().to_string()),
                        )
                        .when(selected, |this| {
                            this.child(div().flex_1())
                                .child(div().size(px(5.)).rounded_full().bg(theme.primary))
                        })
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| {
                                this.set_page(page, cx);
                                window.refresh();
                            });
                        })
                        .into_any_element()
                }))
        };

        let status_tone = if self.example {
            Tone::Amber
        } else if self.connected {
            Tone::Green
        } else {
            Tone::Red
        };
        let status_text = if self.example {
            "Example data"
        } else if self.connected {
            "Broker connected"
        } else {
            "Reconnecting"
        };

        div()
            .v_flex()
            .w_full()
            .h_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                div()
                    .px(px(16.))
                    .pt(px(18.))
                    .pb(px(10.))
                    .child(self.brand_marks[0].clone()),
            )
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(nav_group(
                        "Monitor",
                        &[Page::Overview, Page::Providers, Page::Models, Page::Usage],
                    ))
                    .child(nav_group(
                        "Operate",
                        &[Page::Apps, Page::Sessions, Page::Playground],
                    ))
                    .child(nav_group(
                        "Support",
                        &[Page::Guide, Page::Diagnostics, Page::Settings],
                    )),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(ui::dot(status_tone, theme))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.foreground)
                                    .child(status_text),
                            ),
                    )
                    .child(
                        div().h_flex().gap(px(6.)).child(
                            div()
                                .text_size(px(10.))
                                .text_color(theme.muted_foreground)
                                .child(if self.example {
                                    "No runtime connected"
                                } else if self.bundle.as_ref().is_some_and(|bundle| {
                                    bundle.error.is_none() && bundle.apps_error.is_none()
                                }) {
                                    "Administrator access confirmed"
                                } else {
                                    "Scoped credential"
                                }),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn header(&self, theme: &Theme, entity: Entity<Self>) -> AnyElement {
        let updated = self
            .bundle
            .as_ref()
            .map(|b| {
                let age = chrono::Utc::now().timestamp() - b.fetched_at;
                ui::format_age(age)
            })
            .unwrap_or_else(|| "waiting".into());

        div()
            .h_flex()
            .px(px(ui::PAGE_PAD))
            .pt(px(20.))
            .pb(px(14.))
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap(px(2.))
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.page.title()),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.muted_foreground)
                            .child(
                                if self.bundle.as_ref().is_some_and(|bundle| {
                                    bundle.snapshot.mode == originlm_contracts::DataMode::Synthetic
                                }) {
                                    "Example data. No accounts inspected; execution disabled."
                                } else {
                                    self.page.subtitle()
                                },
                            ),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .h_flex()
                            .gap(px(5.))
                            .px_2()
                            .py(px(4.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(theme.border)
                            .text_size(px(10.5))
                            .text_color(theme.muted_foreground)
                            .child(ui::icon(IconName::RefreshCw, 11., theme.muted_foreground))
                            .child(updated),
                    )
                    .child({
                        let e = entity.clone();
                        Button::new("refresh")
                            .secondary()
                            .label("Reload")
                            .icon(IconName::RefreshCw)
                            .on_click(move |_, _, cx| {
                                e.update(cx, |this, cx| this.refresh(cx));
                            })
                    })
                    .child({
                        let e = entity.clone();
                        Button::new("theme")
                            .secondary()
                            .icon(if self.dark {
                                IconName::Sun
                            } else {
                                IconName::Moon
                            })
                            .on_click(move |_, _, cx| {
                                e.update(cx, |this, cx| this.toggle_theme(cx));
                            })
                    })
                    .child({
                        let e = entity.clone();
                        div()
                            .id("palette-button")
                            .h_flex()
                            .gap_2()
                            .px_3()
                            .py(px(5.))
                            .rounded(px(7.))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.secondary_hover))
                            .child(ui::icon(IconName::Search, 12., theme.muted_foreground))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(theme.muted_foreground)
                                    .child("Command"),
                            )
                            .child(ui::keycap(command_key(), theme))
                            .on_click(move |_, window, cx| {
                                e.update(cx, |this, cx| this.open_palette(window, cx));
                            })
                    }),
            )
            .into_any_element()
    }

    fn connection_banner(&self, theme: &Theme) -> Option<AnyElement> {
        let error = self.error.as_ref()?;
        Some(
            div()
                .mx(px(ui::PAGE_PAD))
                .mb(px(12.))
                .h_flex()
                .gap_2()
                .p_3()
                .rounded(px(9.))
                .border_1()
                .border_color(theme.danger.opacity(0.3))
                .bg(theme.danger.opacity(0.07))
                .child(ui::icon(IconName::CircleAlert, 13., theme.danger))
                .child(
                    div()
                        .v_flex()
                        .gap(px(1.))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(theme.danger)
                                .child(format!("Broker unavailable: {error}")),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "Retrying with {}. Start the shared broker or pass --token-file.",
                                    self.token_path.display()
                                )),
                        ),
                )
                .into_any_element(),
        )
    }

    fn status_bar(&self, theme: &Theme, entity: Entity<Self>) -> AnyElement {
        let revision = self
            .snapshot()
            .map(|s| s.revision.chars().take(18).collect::<String>())
            .unwrap_or_else(|| "—".into());
        let freshness = self
            .bundle
            .as_ref()
            .map(|b| {
                if b.error.is_some() {
                    "stale".to_string()
                } else {
                    ui::format_age(chrono::Utc::now().timestamp() - b.fetched_at)
                }
            })
            .unwrap_or_else(|| "offline".into());

        div()
            .h_flex()
            .h(px(30.))
            .px(px(16.))
            .gap_4()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.title_bar)
            .text_size(px(10.5))
            .text_color(theme.muted_foreground)
            .child(
                div()
                    .h_flex()
                    .gap(px(5.))
                    .child(ui::icon(
                        IconName::SquareTerminal,
                        11.,
                        theme.muted_foreground,
                    ))
                    .child(
                        div()
                            .font_family(theme.mono_font_family.clone())
                            .child(format!("revision {revision}")),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(5.))
                    .child(ui::dot(
                        if self.connected {
                            Tone::Green
                        } else {
                            Tone::Red
                        },
                        theme,
                    ))
                    .child(format!("refreshed {freshness} ago")),
            )
            .child(div().flex_1())
            .child({
                div()
                    .id("mini-view-toggle")
                    .h_flex()
                    .gap_1()
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme.foreground))
                    .child(ui::icon(IconName::ChartPie, 11., theme.muted_foreground))
                    .child("Today")
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.mini_view = !this.mini_view;
                            cx.notify();
                        });
                    })
            })
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(ui::keycap(command_key(), theme))
                    .child("command")
                    .child(ui::keycap("R", theme))
                    .child("refresh"),
            )
            .into_any_element()
    }

    fn palette_overlay(
        &self,
        theme: &Theme,
        entity: Entity<Self>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.palette_open {
            return None;
        }
        let query = self.palette_query.to_lowercase();
        #[derive(Clone)]
        enum PaletteAction {
            ToggleSidebar,
            ResetLayout,
        }
        type PaletteResult = (
            String,
            String,
            Option<Page>,
            Option<String>,
            Option<PaletteAction>,
        );
        let mut results: Vec<PaletteResult> = Vec::new();
        if "toggle sidebar".contains(&query) || "sidebar".contains(&query) {
            results.push((
                "Toggle sidebar".into(),
                "Layout · Ctrl+B".into(),
                None,
                None,
                Some(PaletteAction::ToggleSidebar),
            ));
        }
        if "reset layout".contains(&query) || "layout".contains(&query) {
            results.push((
                "Reset layout".into(),
                "Restore default panel widths".into(),
                None,
                None,
                Some(PaletteAction::ResetLayout),
            ));
        }
        for page in Page::ALL {
            if page.title().to_lowercase().contains(&query) {
                results.push((
                    page.title().to_string(),
                    format!("Go to {}", page.title()),
                    Some(page),
                    None,
                    None,
                ));
            }
        }
        if let Some(snapshot) = self.snapshot() {
            for model in snapshot.models.iter().take(400) {
                if results.len() >= 9 {
                    break;
                }
                if model.name.to_lowercase().contains(&query) {
                    results.push((
                        model.name.clone(),
                        "Open model".into(),
                        None,
                        Some(model.id.clone()),
                        None,
                    ));
                }
            }
        }
        let backdrop = div()
            .id("palette-backdrop")
            .absolute()
            .inset_0()
            .size_full()
            .bg(black().opacity(0.5))
            .on_click({
                let entity = entity.clone();
                move |_, _, cx| {
                    entity.update(cx, |this, cx| this.close_palette(cx));
                }
            });

        let panel =
            div()
                .absolute()
                .top(px(96.))
                .left(relative(0.5))
                .ml(px(-280.))
                .w(px(560.))
                .v_flex()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_lg()
                .overflow_hidden()
                .child(
                    div()
                        .p_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(Input::new(&self.palette_input)),
                )
                .child(div().v_flex().p_2().gap(px(2.)).children(
                    results.into_iter().enumerate().map(
                        |(index, (title, detail, page, model, action))| {
                            let entity = entity.clone();
                            div()
                                .id(("palette-item", index))
                                .h_flex()
                                .gap_3()
                                .px_3()
                                .py_2()
                                .rounded(px(7.))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.list_hover))
                                .child(
                                    div()
                                        .size(px(22.))
                                        .rounded(px(6.))
                                        .bg(theme.muted)
                                        .h_flex()
                                        .items_center()
                                        .justify_center()
                                        .child(ui::icon(
                                            page.map(Page::icon).unwrap_or(IconName::Cpu),
                                            12.,
                                            theme.muted_foreground,
                                        )),
                                )
                                .child(
                                    div()
                                        .v_flex()
                                        .gap(px(1.))
                                        .child(div().text_size(px(12.5)).child(title))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.muted_foreground)
                                                .child(detail),
                                        ),
                                )
                                .on_click(move |_, window, cx| {
                                    entity.update(cx, |this, cx| {
                                        if let Some(page) = page {
                                            this.set_page(page, cx);
                                        } else if let Some(model) = model.clone() {
                                            this.selected_model = Some(model);
                                            this.set_page(Page::Models, cx);
                                        } else if let Some(action) = action.clone() {
                                            match action {
                                                PaletteAction::ToggleSidebar => {
                                                    this.toggle_sidebar(window, cx)
                                                }
                                                PaletteAction::ResetLayout => {
                                                    this.reset_layout(window, cx)
                                                }
                                            }
                                            this.palette_open = false;
                                        }
                                        window.refresh();
                                    });
                                })
                                .into_any_element()
                        },
                    ),
                ));

        let panel = if cx.reduce_motion() {
            panel.into_any_element()
        } else {
            panel
                .with_animation(
                    "palette-in",
                    Animation::new(Duration::from_millis(140)),
                    |panel, progress| panel.opacity(progress).mt(px(6.) * (1.0 - progress)),
                )
                .into_any_element()
        };
        Some(
            div()
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(panel)
                .into_any_element(),
        )
    }

    fn mini_view_overlay(&self, theme: &Theme, entity: Entity<Self>) -> Option<AnyElement> {
        if !self.mini_view {
            return None;
        }
        let rows = self
            .snapshot()
            .map(crate::data::quota_rows)
            .unwrap_or_default();
        let mut seen = std::collections::BTreeSet::new();
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|row| seen.insert((row.pool_id.clone(), row.window_id.clone())))
            .take(4)
            .collect();
        let close = entity.clone();
        let open = entity.clone();
        Some(
            div()
                .absolute()
                .top(px(52.))
                .right(px(18.))
                .w(px(320.))
                .v_flex()
                .gap_2()
                .p_3()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_lg()
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Allowances"),
                        )
                        .child(
                            div()
                                .id("mini-close")
                                .size(px(32.))
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .text_color(theme.muted_foreground)
                                .hover(|style| style.bg(theme.secondary_hover))
                                .child("×")
                                .on_click(move |_, _, cx| {
                                    close.update(cx, |this, cx| {
                                        this.mini_view = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .children(rows.iter().map(|row| {
                    let remaining = row
                        .remaining
                        .map(|value| format!("{:.0}% left", value * 100.0))
                        .unwrap_or_else(|| "Unknown".into());
                    let reset = ui::format_countdown(row.resets_at.as_deref())
                        .map(|value| format!("Resets in {value}"))
                        .unwrap_or_else(|| "Reset not reported".into());
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .child(crate::icons::provider_tile(
                            &row.provider_id,
                            &row.pool_label,
                            22.,
                            theme,
                        ))
                        .child(
                            div()
                                .v_flex()
                                .gap(px(1.))
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(row.pool_label.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.5))
                                        .text_color(theme.muted_foreground)
                                        .child(format!(
                                            "{}: {reset}{}",
                                            row.window_label,
                                            if row.fresh { "" } else { " (stale)" }
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_size(px(14.))
                                .child(remaining),
                        )
                        .into_any_element()
                }))
                .when(rows.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.muted_foreground)
                            .child("No provider-reported allowance yet."),
                    )
                })
                .child(
                    Button::new("mini-open-overview")
                        .secondary()
                        .label("Open Overview")
                        .on_click(move |_, _, cx| {
                            open.update(cx, |this, cx| {
                                this.set_page(Page::Overview, cx);
                                this.mini_view = false;
                                cx.notify();
                            });
                        }),
                )
                .child(
                    div()
                        .text_size(px(9.))
                        .text_color(theme.muted_foreground)
                        .child("Optional mini view (Ctrl+Shift+M)"),
                )
                .into_any_element(),
        )
    }

    fn toast_overlay(&self, theme: &Theme) -> Option<AnyElement> {
        let (message, _) = self.toast.as_ref()?;
        let toast = div()
            .absolute()
            .bottom(px(46.))
            .right(px(24.))
            .max_w(px(420.))
            .px_4()
            .py_3()
            .rounded(px(9.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .text_size(px(12.))
            .child(message.clone());
        Some(
            toast
                .with_animation(
                    SharedString::from(format!("toast-{message}")),
                    Animation::new(Duration::from_millis(120)),
                    |toast, progress| {
                        toast
                            .opacity(progress)
                            .bottom(px(46.) - px(6.) * (1.0 - progress))
                    },
                )
                .into_any_element(),
        )
    }
}

fn data_auth(connection: &originlm_contracts::Connection) -> bool {
    matches!(
        &connection.auth,
        originlm_contracts::Observation::Known {
            value: originlm_contracts::AuthState::Authenticated,
            ..
        }
    )
}

fn preferred_model(snapshot: &originlm_contracts::Snapshot) -> Option<String> {
    let mut candidates: Vec<&originlm_contracts::Model> = snapshot
        .models
        .iter()
        .filter(|model| {
            snapshot
                .routes
                .iter()
                .find(|route| route.id == model.route_id)
                .and_then(|route| {
                    snapshot
                        .connections
                        .iter()
                        .find(|connection| connection.id == route.connection_id)
                })
                .is_some_and(data_auth)
        })
        .collect();
    candidates.sort_by_key(|model| !model.is_default);
    candidates
        .first()
        .map(|model| model.id.clone())
        .or_else(|| snapshot.models.first().map(|model| model.id.clone()))
}

pub fn command_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘K"
    } else {
        "Ctrl K"
    }
}

fn default_sidebar_width() -> f32 {
    226.0
}

fn default_inspector_width() -> f32 {
    430.0
}
