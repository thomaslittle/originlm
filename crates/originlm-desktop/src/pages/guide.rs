use crate::app::Desktop;
use crate::ui::{self, Tone};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    progress::Progress,
};
use gpui_kit::*;

impl Desktop {
    pub fn page_guide(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let methods: Vec<(&str, &str, &str)> = vec![
            (
                "runtime.handshake",
                "Negotiate protocol 1.0 and list the methods this broker supports.",
                r#"{"jsonrpc":"2.0","id":1,"method":"runtime.handshake","params":{"protocolVersion":"1.0","clientName":"my-app"}}"#,
            ),
            (
                "snapshot.get",
                "Read the committed inventory: providers, connections, routes, models, quota pools and issues.",
                r#"{"jsonrpc":"2.0","id":2,"method":"snapshot.get","params":{}}"#,
            ),
            (
                "discovery.refresh",
                "Ask the broker for a passive metadata scan. Requires a discovery grant.",
                r#"{"jsonrpc":"2.0","id":3,"method":"discovery.refresh","params":{"level":"passive"}}"#,
            ),
            (
                "connections.probe",
                "Refresh one provider's metadata and usage. Requires a probe grant.",
                r#"{"jsonrpc":"2.0","id":4,"method":"connections.probe","params":{"connectionId":"connection_…"}}"#,
            ),
            (
                "models.page",
                "Page the provider-native model catalog with route, options and evidence.",
                r#"{"jsonrpc":"2.0","id":5,"method":"models.page","params":{"limit":50}}"#,
            ),
            (
                "selection.resolve",
                "Resolve a route and model by requirements without guessing capabilities.",
                r#"{"jsonrpc":"2.0","id":6,"method":"selection.resolve","params":{"requirements":{"requiredCapabilities":["generation.text"]}}}"#,
            ),
            (
                "usage.read",
                "Read provider-reported allowance windows. Percentages are never summed across pools.",
                r#"{"jsonrpc":"2.0","id":7,"method":"usage.read","params":{}}"#,
            ),
            (
                "apps.list / apps.revoke",
                "List live broker clients or revoke an app-scoped grant. Admin credential only.",
                r#"{"jsonrpc":"2.0","id":8,"method":"apps.list","params":{}}"#,
            ),
            (
                "sessions.create → turns.start → events.read",
                "Run one explicit turn. Requires an execution grant and a workspace.",
                r#"{"jsonrpc":"2.0","id":9,"method":"sessions.create","params":{"selection":{…},"workspaceId":"default","permissionMode":"read_only"}}"#,
            ),
        ];

        let entity = cx.entity();

        div()
            .v_flex()
            .w_full()
            .px(px(ui::PAGE_PAD))
            .pb(px(36.))
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .child(ui::section_title("Install", &theme))
                            .child(
                                Button::new("open-install-docs")
                                    .secondary()
                                    .label("Install guide")
                                    .on_click(|_, _, cx| {
                                        cx.open_url(
                                            "https://github.com/thomaslittle/originlm/blob/main/docs/integration/install.md",
                                        );
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child("Two ways in, one runtime: run the dashboard for every client and quota, or add the dependency to your app. Installing it for the dashboard makes every app built on it work."),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_start()
                            .gap_3()
                            .child(track(
                                "Get the app",
                                "The local control center: usage, quotas, models and issues for every AI client on this machine.",
                                &[
                                    "curl -fsSL https://<site>/install.sh | sh",
                                    "irm https://<site>/install.ps1 | iex",
                                ],
                                &theme,
                            ))
                            .child(track(
                                "Add to your project",
                                "One dependency. The SDK finds a running runtime or starts its own.",
                                &[
                                    "npm install originlm",
                                    "cargo add originlm",
                                    "pip install originlm",
                                ],
                                &theme,
                            )),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Three ways to use the kit"),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.muted_foreground)
                            .child("The Rust broker is authoritative. This desktop app, the terminal control center and your own applications all read it through the same authenticated API."),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .children(
                                [
                                    ("1", "Start the broker", "One owner-only Unix socket. It owns discovery, probes and grants."),
                                    ("2", "Connect a client", "Authenticate with an app-scoped descriptor, then handshake."),
                                    ("3", "Read or act", "Snapshot, catalog and usage are read-only. Execution needs its own grant."),
                                ]
                                .into_iter()
                                .map(|(number, title, body)| {
                                    div()
                                        .flex_1()
                                        .v_flex()
                                        .gap_1()
                                        .p_3()
                                        .rounded(px(8.))
                                        .bg(theme.muted)
                                        .child(
                                            div()
                                                .h_flex()
                                                .gap_2()
                                                .child(ui::mono(number, &theme))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(title),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(theme.muted_foreground)
                                                .child(body),
                                        )
                                        .into_any_element()
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .child(ui::section_title("Broker API", &theme))
                            .child({
                                let entity = entity.clone();
                                Button::new("open-broker-docs")
                                    .secondary()
                                    .label("Open API guide")
                                    .on_click(move |_, _, cx| {
                                        cx.open_url(
                                            "https://github.com/thomaslittle/originlm/blob/main/docs/integration/shared-broker.md",
                                        );
                                        let _ = &entity;
                                    })
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child("Newline-delimited JSON-RPC over the local socket. The first frame authenticates with the token from your descriptor; never put that token on a command line."),
                    )
                    .child(code_block(
                        &[
                            "{",
                            "  \"jsonrpc\": \"2.0\", \"id\": 0,",
                            "  \"method\": \"broker.authenticate\",",
                            "  \"params\": { \"token\": \"<from tokenFile>\" }",
                            "}",
                        ],
                        &theme,
                    ))
                    .children(methods.into_iter().map(|(name, description, example)| {
                        let copy = example.to_string();
                        div()
                            .v_flex()
                            .gap_1()
                            .py_3()
                            .border_t_1()
                            .border_color(theme.border.opacity(0.6))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_family(theme.mono_font_family.clone())
                                            .text_size(px(11.))
                                            .text_color(theme.primary)
                                            .child(name),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("copy-{name}")))
                                            .text_size(px(10.))
                                            .text_color(theme.muted_foreground)
                                            .cursor_pointer()
                                            .hover(|style| style.text_color(theme.foreground))
                                            .child("Copy example")
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy.clone(),
                                                ));
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.muted_foreground)
                                    .child(description),
                            )
                            .child(code_block(&[example], &theme))
                            .into_any_element()
                    })),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(ui::section_title("Use it from TypeScript", &theme))
                    .child(code_block(
                        &[
                            "import {createLocalAI} from 'originlm';",
                            "import {openSharedBrokerTransport} from 'originlm/node';",
                            "",
                            "const ai = await createLocalAI({",
                            "  transport: await openSharedBrokerTransport({",
                            "    executable: '/path/to/originlm',",
                            "    address: '/path/to/broker.sock',",
                            "    tokenFile: '/path/to/app-token.json',",
                            "    startArgs: ['--allow-passive-discovery'],",
                            "  }),",
                            "});",
                            "await ai.discovery.refresh({level: 'passive'});",
                            "const env = await ai.snapshot.get();",
                        ],
                        &theme,
                    ))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.muted_foreground)
                            .child("The transport starts a broker only when none is reachable. Your app's grant still decides what it can read or run."),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(ui::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.list)
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .child(ui::section_title("Building this interface", &theme))
                            .child({
                                Button::new("open-kit-docs")
                                    .secondary()
                                    .label("GPUI Kit docs")
                                    .on_click(|_, _, cx| cx.open_url("https://gpui-kit.com/docs"))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child("Everything here is native Rust: GPUI Kit 0.8 on GPUI Fast. Buttons, inputs, menus, tables and charts come from the kit; the theme is one palette, not a second component system."),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(Button::new("g-primary").primary().label("Primary"))
                            .child(Button::new("g-secondary").secondary().label("Secondary"))
                            .child(Button::new("g-danger").danger().label("Destructive"))
                            .child(Button::new("g-ghost").ghost().label("Ghost")),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(Button::new("open-components").secondary().label("Component gallery").on_click(
                                |_, _, cx| cx.open_url("https://gpui-kit.com/component"),
                            ))
                            .child(Button::new("open-charts").secondary().label("Charts").on_click(
                                |_, _, cx| cx.open_url("https://gpui-kit.com/component/chart"),
                            ))
                            .child(Button::new("open-docsrs").secondary().label("docs.rs").on_click(
                                |_, _, cx| cx.open_url("https://docs.rs/gpui-kit"),
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.muted_foreground)
                            .child("The gallery opens in your browser. An embedded WebView is not available in this Linux Wayland build: GPUI's WebView needs WebKitGTK/GTK development packages and X11 or XWayland, and this session is Wayland-native."),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .children([32u32, 58, 84].into_iter().map(|value| {
                                div()
                                    .flex_1()
                                    .v_flex()
                                    .gap_1()
                                    .child(ui::mono(format!("{value}%"), &theme))
                                    .child(
                                        Progress::new(("gallery-progress", value))
                                            .value(value as f32)
                                            .color(theme.primary),
                                    )
                                    .into_any_element()
                            })),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(ui::chip("Connected", Tone::Green, &theme))
                            .child(ui::chip("Stale", Tone::Amber, &theme))
                            .child(ui::chip("Unavailable", Tone::Red, &theme))
                            .child(ui::chip("Local cache", Tone::Neutral, &theme)),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .items_center()
                            .children(
                                [
                                    ("codex", "Codex"),
                                    ("claude_code", "Claude Code"),
                                    ("opencode", "OpenCode"),
                                    ("cursor", "Cursor"),
                                    ("gemini_cli", "Gemini"),
                                    ("ollama", "Ollama"),
                                ]
                                .into_iter()
                                .map(|(id, label)| {
                                    div()
                                        .h_flex()
                                        .gap_1()
                                        .children(crate::icons::provider_mark(
                                            id,
                                            14.,
                                            theme.muted_foreground,
                                        ))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.muted_foreground)
                                                .child(label),
                                        )
                                        .into_any_element()
                                }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn track(
    title: &'static str,
    body: &'static str,
    commands: &[&str],
    theme: &gpui_kit::component::Theme,
) -> AnyElement {
    div()
        .flex_1()
        .min_w(px(0.))
        .v_flex()
        .gap_2()
        .p_3()
        .rounded(px(8.))
        .bg(theme.muted)
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(10.5))
                .text_color(theme.muted_foreground)
                .child(body),
        )
        .child(code_block(commands, theme))
        .into_any_element()
}

fn code_block(lines: &[&str], theme: &gpui_kit::component::Theme) -> AnyElement {
    let id = SharedString::from(format!(
        "code-{}",
        lines
            .first()
            .unwrap_or(&"")
            .chars()
            .take(28)
            .collect::<String>()
    ));
    div()
        .id(id)
        .overflow_x_scroll()
        .v_flex()
        .gap(px(1.))
        .p_3()
        .rounded(px(7.))
        .bg(theme.muted)
        .border_1()
        .border_color(theme.border.opacity(0.7))
        .children(lines.iter().map(|line| {
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(px(10.))
                .text_color(theme.foreground)
                .child(line.to_string())
        }))
        .into_any_element()
}
