define_clients!(
    Codex = 0 => {
        id: "codex",
        display: "Codex CLI",
        logo: None,
        root: PathRoot::EnvVar {
            var: "CODEX_HOME",
            fallback_relative: ".codex",
        },
        relative: "sessions",
        pattern: "*.jsonl",
        headless: true,
        parse_local: true,
        submit_default: true
    },
    Cursor = 1 => {
        id: "cursor",
        display: "Cursor",
        logo: None,
        root: PathRoot::Home,
        relative: ".config/tokscale/cursor-cache",
        pattern: "usage*.json",
        headless: false,
        parse_local: false,
        submit_default: true
    }
);
