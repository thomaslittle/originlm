use crate::bootstrap::Credential;
use crate::broker::{self, BrokerHandle, Bundle, ExecEvent, ExecHandle, ModelChoice};
use crate::consent::PaidConsent;
use crate::ds::{self, Focusable as _};
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
    collections::{BTreeMap, BTreeSet},
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
    Usage,
    Providers,
    Models,
    Developer,
    Apps,
    Sessions,
    Playground,
    Diagnostics,
    Settings,
}

/// Sidebar groups, in order. Usage first for people who opened the app to
/// check what is left; Build is the developer surface.
pub const NAV_GROUPS: [(&str, &[Page]); 4] = [
    ("Usage", &[Page::Overview, Page::Usage]),
    ("Inventory", &[Page::Providers, Page::Models]),
    (
        "Build",
        &[
            Page::Developer,
            Page::Apps,
            Page::Sessions,
            Page::Playground,
        ],
    ),
    ("System", &[Page::Diagnostics, Page::Settings]),
];

impl Page {
    pub const ALL: [Page; 10] = [
        Page::Overview,
        Page::Usage,
        Page::Providers,
        Page::Models,
        Page::Developer,
        Page::Apps,
        Page::Sessions,
        Page::Playground,
        Page::Diagnostics,
        Page::Settings,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Usage => "Usage history",
            Page::Providers => "Accounts",
            Page::Models => "Models",
            Page::Developer => "Developer",
            Page::Apps => "Apps & grants",
            Page::Sessions => "Sessions",
            Page::Playground => "Playground",
            Page::Diagnostics => "Diagnostics",
            Page::Settings => "Settings",
        }
    }
    /// Earlier titles, so saved preferences and capture scripts keep working.
    fn legacy_title(self) -> Option<&'static str> {
        match self {
            Page::Usage => Some("Usage"),
            Page::Providers => Some("Providers & accounts"),
            Page::Developer => Some("How to use"),
            Page::Apps => Some("Connected apps"),
            _ => None,
        }
    }
    pub fn from_name(name: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|page| {
            page.title().eq_ignore_ascii_case(name)
                || page
                    .legacy_title()
                    .is_some_and(|legacy| legacy.eq_ignore_ascii_case(name))
        })
    }
    pub fn subtitle(self) -> &'static str {
        match self {
            Page::Overview => {
                "What is left on each account, when it resets, and what needs attention."
            }
            Page::Usage => "Allowance readings this app has recorded over time.",
            Page::Providers => "Installations, profiles and the accounts behind them.",
            Page::Models => "Provider-native catalogs, capabilities and options.",
            Page::Developer => {
                "Build on this runtime: transports, routes, SDKs and the grant model."
            }
            Page::Apps => "Applications connected to this runtime and what they may do.",
            Page::Sessions => "Sessions this app started, and their turns.",
            Page::Playground => "One explicit test turn. Nothing runs in the background.",
            Page::Diagnostics => "Plain-language failures, freshness and a redacted export.",
            Page::Settings => "Appearance, data source, refresh cadence and local prices.",
        }
    }
    pub fn icon(self) -> IconName {
        match self {
            Page::Overview => IconName::LayoutDashboard,
            Page::Usage => IconName::ChartPie,
            Page::Providers => IconName::Bot,
            Page::Models => IconName::Cpu,
            Page::Developer => IconName::SquareTerminal,
            Page::Apps => IconName::Inbox,
            Page::Sessions => IconName::GalleryVerticalEnd,
            Page::Playground => IconName::Play,
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
    /// The runtime stopped answering and `bundle` is the last good snapshot.
    pub stale: bool,
    pub example: bool,
    pub last_event: Instant,
    pub palette_open: bool,
    pub palette_input: Entity<InputState>,
    pub palette_query: String,
    pub search: Entity<InputState>,
    /// Search field inside the Playground model popover.
    pub model_search: Entity<InputState>,
    pub model_query: String,
    pub prompt: Entity<TextareaState>,
    /// Images staged for the next Playground turn, in selection order.
    pub attachments: Vec<PathBuf>,
    /// Model option value chosen for the next Playground turn (usually effort).
    pub selected_effort: Option<String>,
    /// The model chip's picker is open below the composer.
    pub model_picker_open: bool,
    /// Provider tab selected in the model popover; `None` means all providers.
    pub model_provider_filter: Option<String>,
    /// The effort chip's small option menu is open.
    pub effort_picker_open: bool,
    pub price_fields: [Entity<InputState>; 6],
    pub price_edit_original: Option<originlm_contracts::ModelPrice>,
    pub query: String,
    pub selected_connection: Option<String>,
    pub selected_model: Option<String>,
    pub selected_series: Option<String>,
    pub range: Range,
    pub usage_table: bool,
    pub favorites: BTreeSet<String>,
    /// Models page shows favorites only.
    pub favorites_only: bool,
    pub sessions: Vec<UiSession>,
    pub transcript: Vec<TranscriptLine>,
    pub running: bool,
    pub active_session: Option<String>,
    pub active_turn: Option<String>,
    pub usage_note: Option<String>,
    pub playground_error: Option<String>,
    pub paid_consent: PaidConsent,
    pub revoke_confirm: Option<String>,
    pub toast: Option<(String, Instant)>,
    pub mini_view: bool,
    /// Last rendered viewport width, for responsive page layouts.
    pub viewport_w: f32,
    /// Selected SDK sample on the Developer page.
    pub dev_sample: usize,
    pub broker: BrokerHandle,
    pub executor: ExecHandle,
    pub credential: Credential,
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
        credential: Credential,
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
            credential.clone(),
            Duration::from_secs(refresh_seconds),
            example,
        );
        let executor = broker::spawn_executor(credential.clone(), example);
        let palette_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Jump to a page, model or action…"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search models…"));
        let model_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search models and providers…"));
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

        subscriptions.push(cx.subscribe_in(&model_search, window, {
            let input = model_search.clone();
            move |this: &mut Desktop, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.model_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }
        }));

        let mut this = Self {
            example,
            page: std::env::var("LAI_DESKTOP_PAGE")
                .ok()
                .and_then(|name| Page::from_name(&name))
                .or_else(|| prefs.page.as_deref().and_then(Page::from_name))
                .unwrap_or(Page::Overview),
            brand_marks,
            bundle: None,
            dark,
            refresh_seconds,
            error: None,
            connected: false,
            stale: false,
            last_event: Instant::now(),
            palette_open: false,
            palette_input,
            palette_query: String::new(),
            search,
            model_search,
            model_query: String::new(),
            prompt,
            attachments: Vec::new(),
            selected_effort: None,
            model_picker_open: false,
            model_provider_filter: None,
            effort_picker_open: false,
            price_fields,
            price_edit_original: None,
            query: String::new(),
            selected_connection: None,
            selected_model: None,
            selected_series: None,
            range: Range::Day,
            usage_table: false,
            favorites: prefs.favorites.into_iter().collect(),
            favorites_only: false,
            sessions: Vec::new(),
            transcript: Vec::new(),
            running: false,
            active_session: None,
            active_turn: None,
            usage_note: None,
            playground_error: None,
            paid_consent: PaidConsent::default(),
            revoke_confirm: None,
            toast: None,
            mini_view: example && std::env::var_os("LAI_DESKTOP_MINI_VIEW").is_some(),
            viewport_w: 1440.,
            dev_sample: 0,
            broker,
            executor,
            credential,
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
                    // A failed read arrives as an empty snapshot. If the last
                    // good snapshot is still here, keep showing it, marked
                    // stale with its real age, instead of blanking every page.
                    let keep_last = bundle.error.is_some()
                        && self.bundle.as_ref().is_some_and(|last| {
                            last.error.is_none() && !last.snapshot.connections.is_empty()
                        });
                    if keep_last {
                        self.stale = true;
                    } else {
                        self.stale = false;
                        self.bundle = Some(bundle);
                    }
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
                            let error = if error.contains("BILLING_CONSENT_REQUIRED") {
                                format!(
                                    "{error} Turn on \"Allow paid or unknown-billing routes\" and run again."
                                )
                            } else {
                                error
                            };
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
        // A shared broker gets one bounded metadata probe per authenticated
        // connection that is missing a quota pool. The private runtime starts
        // from a passive scan and lets its background lanes turn "not checked"
        // into verified, cached or sign-in-required. Neither path starts
        // inference or repeats a failed probe within this session.
        let local = self.credential.is_local();
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
                // The private runtime has autonomous refresh lanes that probe
                // providers concurrently. Queueing one probe per connection on
                // this worker serialized every slow client behind the others;
                // let the lanes fill metadata while the cached snapshot stays
                // interactive. A shared broker still needs an explicit nudge
                // for authenticated connections missing an allowance.
                let needed = !local && verified && !pooled;
                if needed && self.startup_probes.insert(connection.id.clone()) {
                    missing.push(connection.id.clone());
                }
            }
        }
        for id in missing {
            let _ = self.broker.commands.send(broker::Command::Probe(id));
        }
    }

    /// Width available to page content, for responsive page layouts.
    pub fn content_width(&self) -> f32 {
        let sidebar = if self.sidebar_collapsed {
            48.
        } else {
            self.sidebar_width
        };
        (self.viewport_w - sidebar - ds::GUTTER * 2.).max(0.)
    }

    /// Ask the runtime to re-read one connection's metadata and usage. Example
    /// mode reads nothing, and says so instead of pretending.
    pub fn request_probe(&mut self, connection_id: String, cx: &mut Context<Self>) {
        if self.example {
            self.toast("Example mode probes nothing. Run without --example to read this account.");
        } else if self
            .broker
            .commands
            .send(broker::Command::Probe(connection_id))
            .is_ok()
        {
            self.toast("Metadata refresh requested. The page updates when the runtime answers.");
        } else {
            self.toast("The runtime worker is not running; restart the app.");
        }
        cx.notify();
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
                // Cursor is read for metadata only; its text execution client
                // is not wired. OpenCode and OpenCode Go routes go through the
                // installed client's text-session transport and are runnable.
                if route.provider_id == "cursor" {
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

    /// The model's reasoning-effort descriptor, when it advertises one.
    fn effort_descriptor(&self) -> Option<&originlm_contracts::OptionDescriptor> {
        let snapshot = self.snapshot()?;
        let model = snapshot
            .models
            .iter()
            .find(|model| Some(&model.id) == self.selected_model.as_ref())?;
        model.options.iter().find(|option| {
            option.id.eq_ignore_ascii_case("effort")
                || option.id.eq_ignore_ascii_case("reasoningEffort")
                || option.label.to_ascii_lowercase().contains("effort")
        })
    }

    pub fn effort_choices(&self) -> Vec<(String, String)> {
        let Some(descriptor) = self.effort_descriptor() else {
            return Vec::new();
        };
        match &descriptor.control {
            originlm_contracts::OptionControl::Select { choices, .. } => choices
                .iter()
                .filter(|choice| choice.disabled_reason.is_none())
                .map(|choice| (choice.value.clone(), choice.label.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn effort_value(&self) -> Option<String> {
        let descriptor = self.effort_descriptor()?;
        let choices = self.effort_choices();
        if let Some(selected) = self
            .selected_effort
            .as_ref()
            .filter(|selected| choices.iter().any(|(value, _)| value == *selected))
        {
            return Some(selected.clone());
        }
        if let Some(current) = descriptor
            .current_value
            .as_ref()
            .and_then(|value| value.as_str())
            && choices.iter().any(|(value, _)| value == current)
        {
            return Some(current.to_owned());
        }
        match &descriptor.control {
            originlm_contracts::OptionControl::Select { default_value, .. } => default_value
                .clone()
                .filter(|default| choices.iter().any(|(value, _)| value == default)),
            _ => None,
        }
        .or_else(|| choices.first().map(|(value, _)| value.clone()))
    }

    pub fn effort_label(&self) -> Option<String> {
        let value = self.effort_value()?;
        self.effort_choices()
            .into_iter()
            .find(|(choice, _)| choice == &value)
            .map(|(_, label)| label)
            .or(Some(value))
    }

    pub fn attach_images(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            let image = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png"
                    )
                });
            if image && !self.attachments.contains(&path) && self.attachments.len() < 4 {
                self.attachments.push(path);
            }
        }
    }

    pub fn pick_attachments(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach images".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                this.update(cx, |this, cx| {
                    this.attach_images(paths);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
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
        self.model_picker_open = false;
        let mut options = BTreeMap::new();
        if let (Some(descriptor), Some(value)) = (self.effort_descriptor(), self.effort_value()) {
            options.insert(descriptor.id.clone(), serde_json::Value::String(value));
        }
        let images = std::mem::take(&mut self.attachments);
        self.transcript.push(TranscriptLine {
            role: TranscriptRole::You,
            text: prompt.clone(),
        });
        // Consent covers this submitted run only; dispatching clears the switch.
        let allow_paid = self
            .selected_model
            .as_deref()
            .is_some_and(|model| self.paid_consent.take_for_run(model));
        let _ = self
            .executor
            .commands
            .send(broker::ExecCommand::Run(Box::new(broker::RunRequest {
                prompt,
                choice,
                allow_paid,
                images,
                options,
            })));
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
        self.viewport_w = window.viewport_size().width.as_f32();
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

        let gated = matches!(
            self.page,
            Page::Overview
                | Page::Usage
                | Page::Providers
                | Page::Models
                | Page::Apps
                | Page::Diagnostics
        );
        let gate = if gated {
            self.data_gate(&theme, cx)
        } else {
            None
        };
        let page_body: AnyElement = if let Some(gate) = gate {
            gate
        } else {
            match self.page {
                Page::Overview => self.page_overview(cx),
                Page::Providers => self.page_providers(cx),
                Page::Models => self.page_models(cx),
                Page::Usage => self.page_usage(cx),
                Page::Apps => self.page_apps(cx),
                Page::Sessions => self.page_sessions(cx),
                Page::Playground => self.page_playground(cx),
                Page::Developer => self.page_developer(cx),
                Page::Diagnostics => self.page_diagnostics(cx),
                Page::Settings => self.page_settings(cx),
            }
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
                if key == "tab" && !command {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                } else if key == "escape" {
                    entity_keys.update(cx, |this, cx| {
                        if this.palette_open {
                            this.close_palette(cx);
                        } else if this.mini_view {
                            this.mini_view = false;
                            cx.notify();
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
                    .children(self.connection_banner(&theme, entity.clone()))
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
                            .text_size(px(crate::ds::ty::SMALL))
                            .text_color(theme.muted_foreground)
                            .child("OriginLM"),
                    )
                    .child(div().flex_1())
                    // gpui-kit's TitleBar draws caption buttons on Windows and on
                    // client-decorated Linux. Only add ours where it does not.
                    .when(cfg!(target_os = "linux") && !client_decorated, |this| {
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
                let tip: SharedString = page.title().into();
                div()
                    .id(("rail-page", page as usize))
                    .role(gpui::Role::Tab)
                    .aria_label(page.title())
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
                    .size(px(36.))
                    .rounded(px(ds::radius::CONTROL))
                    .focus_ring(theme)
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
        use crate::ds::{sp, ty};
        if collapsed {
            return self.sidebar_rail(theme, entity);
        }
        let nav_group = |label: &'static str, pages: &[Page]| {
            div()
                .v_flex()
                .gap(px(2.))
                .pb(px(sp::MD))
                .child(
                    div()
                        .px(px(sp::LG))
                        .pb(px(sp::XS))
                        .text_size(px(ty::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.muted_foreground.opacity(0.85))
                        .child(label),
                )
                .children(pages.iter().map(|page| {
                    let selected = self.page == *page;
                    let page = *page;
                    let entity = entity.clone();
                    div()
                        .id(("nav", page as usize))
                        .role(gpui::Role::Tab)
                        .aria_label(page.title())
                        .focus_ring(theme)
                        .h_flex()
                        .items_center()
                        .gap(px(10.))
                        .mx(px(sp::SM))
                        .px(px(6.))
                        .h(px(34.))
                        .rounded(px(crate::ds::radius::CONTROL))
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
                            15.,
                            if selected {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            },
                        ))
                        .child(
                            div()
                                .text_size(px(ty::BODY))
                                .font_weight(if selected {
                                    FontWeight::MEDIUM
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(page.title().to_string()),
                        )
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| {
                                this.set_page(page, cx);
                                window.refresh();
                            });
                        })
                        .into_any_element()
                }))
        };

        let (status_tone, status_text, status_detail) = if self.example {
            (
                Tone::Amber,
                "Example data",
                "No runtime. Nothing on this machine is read.",
            )
        } else if self.connected {
            (
                Tone::Green,
                "Runtime connected",
                if self
                    .bundle
                    .as_ref()
                    .is_some_and(|bundle| bundle.error.is_none() && bundle.apps_error.is_none())
                {
                    "Administrator access"
                } else {
                    "Scoped credential"
                },
            )
        } else {
            (Tone::Red, "Reconnecting", "The runtime is not answering.")
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
                    .h_flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(sp::LG))
                    .pt(px(sp::LG))
                    .pb(px(sp::XL))
                    .flex_shrink_0()
                    .child(self.brand_marks[0].clone())
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .text_size(px(ty::TITLE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child("OriginLM"),
                            )
                            .child(crate::ds::caption("Control center, alpha", theme)),
                    ),
            )
            .child(
                // Scrolls when the window is short, so the status block below
                // never overlaps the last items.
                div()
                    .id("nav-scroll")
                    .v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(
                        NAV_GROUPS
                            .iter()
                            .map(|(label, pages)| nav_group(label, pages)),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .flex_shrink_0()
                    .gap(px(2.))
                    .px(px(sp::LG))
                    .py(px(sp::MD))
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(crate::ds::status(status_text, status_tone, theme))
                    .child(crate::ds::caption(status_detail, theme).pl(px(13.))),
            )
            .into_any_element()
    }

    fn header(&self, theme: &Theme, entity: Entity<Self>) -> AnyElement {
        use crate::ds::{sp, ty};
        let updated = match &self.bundle {
            None => "Waiting for data".to_string(),
            Some(bundle) if bundle.error.is_some() && !self.stale => "Not connected".to_string(),
            Some(bundle) => {
                let age = chrono::Utc::now().timestamp() - bundle.fetched_at;
                format!(
                    "{} {}",
                    if self.stale { "Last read" } else { "Updated" },
                    ui::format_age(age)
                )
            }
        };
        let synthetic = self
            .bundle
            .as_ref()
            .is_some_and(|bundle| bundle.snapshot.mode == originlm_contracts::DataMode::Synthetic);

        div()
            .h_flex()
            .items_center()
            .px(px(crate::ds::GUTTER))
            .pt(px(sp::XL))
            .pb(px(sp::LG))
            .gap(px(sp::LG))
            .child(
                div()
                    .v_flex()
                    .gap(px(sp::XS))
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(sp::SM))
                            .child(
                                div()
                                    .text_size(px(ty::HEADING))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.page.title()),
                            )
                            .when(synthetic, |this| {
                                this.child(crate::ds::chip("Example data", Tone::Amber, theme))
                            }),
                    )
                    .child(crate::ds::body(
                        if synthetic {
                            "Synthetic records. No accounts inspected; execution disabled."
                        } else {
                            self.page.subtitle()
                        },
                        theme,
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(sp::SM))
                    .child({
                        let e = entity.clone();
                        Button::new("refresh")
                            .ghost()
                            .label(updated)
                            .icon(IconName::RefreshCw)
                            .tooltip("Refresh now (Ctrl+R)")
                            .on_click(move |_, _, cx| {
                                e.update(cx, |this, cx| this.refresh(cx));
                            })
                    })
                    .child({
                        let e = entity.clone();
                        div()
                            .id("palette-button")
                            .role(gpui::Role::Button)
                            .aria_label("Open the command palette")
                            .focus_ring(theme)
                            .h_flex()
                            .items_center()
                            .gap(px(sp::SM))
                            .h(px(32.))
                            .pl(px(sp::MD))
                            .pr(px(6.))
                            .w(px(240.))
                            .rounded(px(crate::ds::radius::CONTROL))
                            .border_color(theme.border)
                            .bg(crate::ds::surface(theme, crate::ds::Surface::Panel))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.secondary_hover))
                            .child(ui::icon(IconName::Search, 13., theme.muted_foreground))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(ty::SMALL))
                                    .text_color(theme.muted_foreground)
                                    .child("Jump to a page or model"),
                            )
                            .child(ui::keycap(command_key(), theme))
                            .on_click(move |_, window, cx| {
                                e.update(cx, |this, cx| this.open_palette(window, cx));
                            })
                    }),
            )
            .into_any_element()
    }

    fn connection_banner(&self, theme: &Theme, entity: Entity<Self>) -> Option<AnyElement> {
        // Without a retained snapshot the page itself shows the offline state.
        if !self.stale {
            return None;
        }
        let error = self.error.clone().unwrap_or_default();
        let age = self
            .bundle
            .as_ref()
            .map(|bundle| ui::format_age(chrono::Utc::now().timestamp() - bundle.fetched_at))
            .unwrap_or_else(|| "?".into());
        Some(
            div()
                .px(px(ds::GUTTER))
                .pb(px(ds::sp::LG))
                .child(
                    ds::banner(
                        Tone::Amber,
                        format!("Stale: showing the last snapshot, read {age} ago"),
                        Some(
                            format!(
                                "The runtime stopped answering ({error}). Nothing below is live. Retrying with {}. {}",
                                self.credential.label(),
                                self.credential.recovery_hint()
                            )
                            .into(),
                        ),
                        theme,
                    )
                    .child(
                        Button::new("stale-retry")
                            .secondary()
                            .label("Retry now")
                            .icon(IconName::RefreshCw)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.refresh(cx));
                            }),
                    ),
                )
                .into_any_element(),
        )
    }

    /// Loading and offline states for pages that need a snapshot.
    fn data_gate(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entity = cx.entity();
        let block = match &self.bundle {
            None => ds::state(
                ds::StateKind::Loading,
                if self.example {
                    "Loading example data"
                } else {
                    "Connecting to the runtime"
                },
                if self.example {
                    "Reading the bundled synthetic snapshot. No accounts on this machine are inspected."
                } else {
                    "Looking for this user's shared broker, or starting a private runtime for this window. Pages fill in when the first snapshot arrives; nothing is guessed before then."
                },
                theme,
            ),
            Some(bundle) if bundle.error.is_some() && !self.stale => ds::state(
                ds::StateKind::Error,
                "The runtime is not answering",
                format!(
                    "{} No snapshot has been read in this session, so there is nothing to show yet. Tried {}. {}",
                    bundle.error.clone().unwrap_or_default(),
                    self.credential.label(),
                    self.credential.recovery_hint()
                ),
                theme,
            )
            .child(
                div().pt(px(ds::sp::SM)).child(
                    Button::new("gate-retry")
                        .secondary()
                        .label("Retry now")
                        .icon(IconName::RefreshCw)
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| this.refresh(cx));
                        }),
                ),
            ),
            _ => return None,
        };
        Some(
            ds::page()
                .child(ds::panel(theme).child(block))
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
            .text_size(px(crate::ds::ty::CAPTION))
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
                        if self.example {
                            Tone::Amber
                        } else if self.connected {
                            Tone::Green
                        } else {
                            Tone::Red
                        },
                        theme,
                    ))
                    .child(match freshness.as_str() {
                        _ if self.stale => "stale, runtime not answering".to_string(),
                        "now" => "refreshed just now".to_string(),
                        "stale" => "runtime not answering".to_string(),
                        "offline" => "waiting for the runtime".to_string(),
                        _ => format!("refreshed {freshness} ago"),
                    }),
            )
            .child(div().flex_1())
            .child({
                div()
                    .id("mini-view-toggle")
                    .role(gpui::Role::Button)
                    .aria_label("Toggle the mini view (Ctrl+Shift+M)")
                    .focus_ring(theme)
                    .h_flex()
                    .items_center()
                    .h(px(26.))
                    .px(px(6.))
                    .rounded(px(ds::radius::CONTROL))
                    .gap_1()
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme.foreground))
                    .child(ui::icon(IconName::ChartPie, 11., theme.muted_foreground))
                    .child("Mini view")
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
                .rounded(px(ds::radius::PANEL))
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
                                .rounded(px(ds::radius::CONTROL))
                                .cursor_pointer()
                                .focus_ring(theme)
                                .hover(|s| s.bg(theme.list_hover))
                                .child(
                                    div()
                                        .size(px(24.))
                                        .rounded(px(ds::radius::CONTROL))
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
                                        .child(div().text_size(px(ds::ty::BODY)).child(title))
                                        .child(
                                            div()
                                                .text_size(px(ds::ty::CAPTION))
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
                .w(px(340.))
                .v_flex()
                .gap_2()
                .p_3()
                .rounded(px(ds::radius::PANEL))
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
                                .text_size(px(ds::ty::BODY))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Allowances"),
                        )
                        .child(ds::icon_button(
                            "mini-close",
                            IconName::Close,
                            "Close the mini view (Esc)",
                            theme.muted_foreground,
                            theme,
                            move |_, _, cx| {
                                close.update(cx, |this, cx| {
                                    this.mini_view = false;
                                    cx.notify();
                                });
                            },
                        )),
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
                                        .text_size(px(ds::ty::SMALL))
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(row.pool_label.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(ds::ty::CAPTION))
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
                                .text_size(px(ds::ty::BODY))
                                .child(remaining),
                        )
                        .into_any_element()
                }))
                .when(rows.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(ds::ty::SMALL))
                            .text_color(theme.muted_foreground)
                            .child(if self.bundle.is_none() {
                                "Waiting for the first snapshot."
                            } else {
                                "No client has reported an allowance window yet."
                            }),
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
                        .text_size(px(ds::ty::CAPTION))
                        .text_color(theme.muted_foreground)
                        .child(if self.stale {
                            "Stale: the runtime is not answering. Ctrl+Shift+M or Esc closes."
                        } else {
                            "Ctrl+Shift+M or Esc closes this view."
                        }),
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
            .rounded(px(ds::radius::PANEL))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .text_size(px(ds::ty::SMALL))
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
