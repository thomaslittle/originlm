use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use originlm_contracts::Snapshot;
use originlm_control::{
    BrokerClient, account_lines, adapter_lines, app_lines, default_export_path,
    diagnostic_document, issue_lines, model_lines, overview_lines, usage_lines, write_export,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Accounts,
    Models,
    Usage,
    Issues,
    Apps,
    Diagnostics,
}
impl Tab {
    const ALL: [Tab; 7] = [
        Tab::Overview,
        Tab::Accounts,
        Tab::Models,
        Tab::Usage,
        Tab::Issues,
        Tab::Apps,
        Tab::Diagnostics,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Accounts => "Accounts",
            Self::Models => "Models",
            Self::Usage => "Usage",
            Self::Issues => "Issues",
            Self::Apps => "Apps",
            Self::Diagnostics => "Diagnostics",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }
}

struct App {
    client: BrokerClient,
    snapshot: Snapshot,
    runtime: Value,
    apps: Vec<Value>,
    adapters: Vec<Value>,
    tab: Tab,
    selected: usize,
    scroll: usize,
    status: String,
    confirm_revoke: Option<String>,
    export_path: PathBuf,
    refresh_seconds: u64,
}

struct Screen;
impl Screen {
    fn enter() -> Result<Self, String> {
        enable_raw_mode().map_err(|_| "Terminal raw mode is unavailable".to_owned())?;
        execute!(std::io::stdout(), EnterAlternateScreen)
            .map_err(|_| "Alternate screen is unavailable".to_owned())?;
        Ok(Self)
    }
}
impl Drop for Screen {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

fn load(app: &mut App) -> Result<(), String> {
    app.snapshot = app.client.snapshot()?;
    app.runtime = app.client.call("runtime.health", json!({}))?;
    match app.client.apps() {
        Ok(apps) => app.apps = apps,
        Err(error) => {
            app.apps.clear();
            app.status = format!("Apps unavailable: {error}");
        }
    }
    app.adapters = app
        .client
        .call("adapters.list", json!({}))
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    clamp_selection(app);
    Ok(())
}

fn current_len(app: &App) -> usize {
    match app.tab {
        Tab::Overview => 0,
        Tab::Accounts => app.snapshot.connections.len(),
        Tab::Models => app.snapshot.models.len(),
        Tab::Usage => app
            .snapshot
            .quota_pools
            .iter()
            .map(|pool| pool.windows.len())
            .sum(),
        Tab::Issues => app.snapshot.issues.len(),
        Tab::Apps => app.apps.len(),
        Tab::Diagnostics => 0,
    }
}
fn clamp_selection(app: &mut App) {
    let len = current_len(app);
    if len == 0 {
        app.selected = 0;
    } else {
        app.selected = app.selected.min(len - 1);
    }
}

fn lines_for(app: &App) -> Vec<String> {
    match app.tab {
        Tab::Overview => overview_lines(&app.snapshot, &app.runtime, &app.apps),
        Tab::Accounts => account_lines(&app.snapshot),
        Tab::Models => model_lines(&app.snapshot),
        Tab::Usage => usage_lines(&app.snapshot),
        Tab::Issues => issue_lines(&app.snapshot),
        Tab::Apps => app_lines(&app.apps),
        Tab::Diagnostics => {
            let mut lines = vec![
                format!("Export: {}", app.export_path.display()),
                format!(
                    "Broker runtime: {}",
                    serde_json::to_string_pretty(&app.runtime).unwrap_or_default()
                ),
                String::new(),
                "Adapters".into(),
            ];
            lines.extend(
                adapter_lines(&app.adapters)
                    .into_iter()
                    .map(|line| format!("  {line}")),
            );
            lines.push(String::new());
            lines.push("Diagnostics are contract-filtered and redacted. No provider token, cookie, or database file is read by this UI.".into());
            lines
        }
    }
}

fn selected_id(app: &App) -> Option<String> {
    match app.tab {
        Tab::Accounts => app
            .snapshot
            .connections
            .get(app.selected)
            .map(|connection| connection.id.clone()),
        Tab::Issues => app
            .snapshot
            .issues
            .get(app.selected)
            .map(|issue| issue.id.clone()),
        Tab::Apps => app
            .apps
            .get(app.selected)
            .and_then(|app| app["id"].as_str())
            .map(str::to_owned),
        _ => None,
    }
}

fn switch(app: &mut App, tab: Tab) {
    app.tab = tab;
    app.selected = 0;
    app.scroll = 0;
    app.status.clear();
}

fn act(app: &mut App, key: KeyCode) {
    if let Some(target) = app.confirm_revoke.clone() {
        match key {
            KeyCode::Char('y') => {
                match app
                    .client
                    .call("apps.revoke", json!({"targetClientId": target}))
                {
                    Ok(_) => app.status = "Application revoked; its next request is denied.".into(),
                    Err(error) => app.status = error,
                }
                app.confirm_revoke = None;
                let _ = load(app);
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.confirm_revoke = None;
                app.status = "Revocation cancelled".into();
            }
            _ => {}
        }
        return;
    }
    match key {
        KeyCode::Tab => {
            let next = (app.tab.index() + 1) % Tab::ALL.len();
            switch(app, Tab::ALL[next]);
        }
        KeyCode::BackTab => {
            let previous = (app.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len();
            switch(app, Tab::ALL[previous]);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if current_len(app) > 0 {
                app.selected = (app.selected + 1).min(current_len(app) - 1);
            } else {
                // Overview and Diagnostics are read-only reports without
                // selectable rows; let the keyboard page through them.
                app.scroll = app.scroll.saturating_add(1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if current_len(app) > 0 {
                app.selected = app.selected.saturating_sub(1);
            } else {
                app.scroll = app.scroll.saturating_sub(1);
            }
        }
        KeyCode::Char('1') => switch(app, Tab::Overview),
        KeyCode::Char('2') => switch(app, Tab::Accounts),
        KeyCode::Char('3') => switch(app, Tab::Models),
        KeyCode::Char('4') => switch(app, Tab::Usage),
        KeyCode::Char('5') => switch(app, Tab::Issues),
        KeyCode::Char('6') => switch(app, Tab::Apps),
        KeyCode::Char('7') => switch(app, Tab::Diagnostics),
        KeyCode::Char('r') => {
            app.status = "Refreshing discovery…".into();
            match app
                .client
                .call("discovery.refresh", json!({"level":"passive"}))
            {
                Ok(_) => {
                    app.status = match load(app) {
                        Ok(()) => "Refreshed".into(),
                        Err(error) => error,
                    };
                }
                Err(error) => app.status = error,
            }
        }
        KeyCode::Char('p') if app.tab == Tab::Accounts => {
            if let Some(id) = selected_id(app) {
                app.status = format!("Probing {id}…");
                match app
                    .client
                    .call("connections.probe", json!({"connectionId": id}))
                {
                    Ok(_) => {
                        app.status = match load(app) {
                            Ok(()) => "Metadata refreshed".into(),
                            Err(error) => error,
                        };
                    }
                    Err(error) => app.status = error,
                }
            }
        }
        KeyCode::Char('a') if app.tab == Tab::Issues => {
            if let Some(id) = selected_id(app) {
                match app
                    .client
                    .call("issues.acknowledge", json!({"issueId": id}))
                {
                    Ok(_) => {
                        app.status = "Issue acknowledged (still open)".into();
                        let _ = load(app);
                    }
                    Err(error) => app.status = error,
                }
            }
        }
        KeyCode::Char('s') if app.tab == Tab::Issues => {
            if let Some(id) = selected_id(app) {
                let until = (chrono::Utc::now() + chrono::Duration::hours(1))
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                match app
                    .client
                    .call("issues.snooze", json!({"issueId": id, "until": until}))
                {
                    Ok(_) => {
                        app.status = "Snoozed for one hour; the issue remains open".into();
                        let _ = load(app);
                    }
                    Err(error) => app.status = error,
                }
            }
        }
        KeyCode::Char('x') if app.tab == Tab::Apps => {
            if let Some(id) = selected_id(app) {
                app.confirm_revoke = Some(id);
            }
        }
        KeyCode::Char('e') => {
            let document =
                diagnostic_document(&app.snapshot, &app.runtime, &app.apps, &app.adapters);
            match write_export(&app.export_path, &document) {
                Ok(()) => {
                    app.status = format!("Redacted export written to {}", app.export_path.display())
                }
                Err(error) => app.status = error,
            }
        }
        _ => {}
    }
}

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(5),
        ])
        .split(frame.area());
    let titles: Vec<Line> = Tab::ALL.iter().map(|tab| Line::from(tab.title())).collect();
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("OriginLM · Control Center"),
        )
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan));
    frame.render_widget(tabs, chunks[0]);

    let lines = lines_for(app);
    let line_count = lines.len();
    let items: Vec<ListItem> = lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let selected = index == app.selected && current_len(app) > 0;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if line.starts_with("Issue") || line.contains("Critical") {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(
                if selected {
                    format!("› {line}")
                } else {
                    line
                },
                style,
            )))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", app.tab.title())),
    );
    let mut state = ListState::default();
    if current_len(app) > 0 {
        state.select(Some(app.selected.min(current_len(app).saturating_sub(1))));
    } else {
        let visible = usize::from(chunks[1].height.saturating_sub(2));
        app.scroll = app.scroll.min(line_count.saturating_sub(visible));
        *state.offset_mut() = app.scroll;
    }
    frame.render_stateful_widget(list, chunks[1], &mut state);

    let prompt = app
        .confirm_revoke
        .as_ref()
        .map(|id| format!("Revoke {id}? y/n — active grants and sessions end immediately."))
        .unwrap_or_else(|| app.status.clone());
    let footer = Paragraph::new(vec![
        Line::from(prompt),
        Line::from("Tab switch · ↑/↓ select · r refresh · e export · q quit"),
        Line::from("p probe account · a acknowledge · s snooze · x revoke app"),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Status "));
    frame.render_widget(footer, chunks[2]);
}

fn once(app: &mut App) {
    println!("OriginLM control center (read-only view)");
    println!("{}", "=".repeat(60));
    println!(
        "{}",
        overview_lines(&app.snapshot, &app.runtime, &app.apps).join("\n")
    );
    println!("\nAccounts\n{}", account_lines(&app.snapshot).join("\n"));
    println!("\nModels\n{}", model_lines(&app.snapshot).join("\n"));
    println!("\nUsage\n{}", usage_lines(&app.snapshot).join("\n"));
    println!("\nIssues\n{}", issue_lines(&app.snapshot).join("\n"));
    println!("\nApplications\n{}", app_lines(&app.apps).join("\n"));
    println!("\nAdapters\n{}", adapter_lines(&app.adapters).join("\n"));
}

fn usage() -> String {
    "originlm-control --ipc ADDRESS --token TOKEN\n\
     originlm-control --token-file ABSOLUTE_PATH [--ipc ADDRESS]\n\
     Options: --once, --export ABSOLUTE_PATH, --refresh-seconds SECONDS\n\
     The control center uses the public authenticated broker API only."
        .into()
}

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut address: Option<String> = None;
    let mut token: Option<String> = None;
    let mut token_file: Option<PathBuf> = None;
    let mut once_mode = false;
    let mut export_path = default_export_path();
    let mut refresh_seconds = 5_u64;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--ipc" => address = args.next(),
            "--token" => token = args.next(),
            "--token-file" => token_file = args.next().map(PathBuf::from),
            "--once" => once_mode = true,
            "--export" => export_path = args.next().map(PathBuf::from).unwrap_or(export_path),
            "--refresh-seconds" => {
                refresh_seconds = args
                    .next()
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|value| (1..=300).contains(value))
                    .unwrap_or(5)
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return std::process::ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("Unknown argument: {flag}\n{}", usage());
                return std::process::ExitCode::from(2);
            }
        }
    }
    let client = match (token_file, address, token) {
        (Some(path), expected, _) => BrokerClient::from_token_file(&path, expected.as_deref()),
        (None, Some(address), Some(token)) => BrokerClient::connect(&address, &token),
        _ => Err("Provide --token-file or both --ipc and --token".into()),
    };
    let client = match client {
        Ok(client) => client,
        Err(error) => {
            eprintln!("{error}\n{}", usage());
            return std::process::ExitCode::from(2);
        }
    };
    let mut app = App {
        client,
        snapshot: Snapshot::empty(originlm_contracts::DataMode::Live),
        runtime: Value::Null,
        apps: Vec::new(),
        adapters: Vec::new(),
        tab: Tab::Overview,
        selected: 0,
        scroll: 0,
        status: String::new(),
        confirm_revoke: None,
        export_path,
        refresh_seconds,
    };
    if let Err(error) = load(&mut app) {
        eprintln!("{error}");
        return std::process::ExitCode::from(2);
    }
    if once_mode {
        once(&mut app);
        app.client.close();
        return std::process::ExitCode::SUCCESS;
    }
    let _screen = match Screen::enter() {
        Ok(screen) => screen,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::from(2);
        }
    };
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(_) => {
            eprintln!("Terminal is unavailable");
            return std::process::ExitCode::from(2);
        }
    };
    let mut last_refresh = std::time::Instant::now();
    loop {
        if terminal.draw(|frame| draw(frame, &mut app)).is_err() {
            break;
        }
        let timeout = Duration::from_secs(app.refresh_seconds);
        match event::poll(timeout) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    // While a destructive action is awaiting confirmation, Esc
                    // must reach the confirmation handler and cancel it rather
                    // than quitting the whole control center.
                    if app.confirm_revoke.is_none()
                        && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
                    {
                        break;
                    }
                    act(&mut app, key.code);
                }
                Ok(_) => {}
                Err(_) => break,
            },
            Ok(false) => {
                if last_refresh.elapsed() >= Duration::from_secs(app.refresh_seconds) {
                    if let Err(error) = load(&mut app) {
                        app.status = error;
                    }
                    last_refresh = std::time::Instant::now();
                }
            }
            Err(_) => break,
        }
    }
    drop(terminal);
    drop(_screen);
    app.client.close();
    std::process::ExitCode::SUCCESS
}
