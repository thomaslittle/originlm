//! Path-only ownership rules. Pure functions over `/`-separated, lowercased
//! paths so the same rule holds for Windows, macOS and Linux inputs.

/// `\` → `/` and lowercase, so a rule matches regardless of platform or case.
pub fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn slashes(path: &str) -> String {
    path.replace('\\', "/")
}

/// `<prefix>/lib/node_modules/<pkg>/...` → `<prefix>`. Rejects nested
/// `node_modules` (a project dependency) and mise tool layouts, which only look
/// like npm globals unless they sit under mise's own Node install.
pub fn npm_global_prefix(real_path: &str, package: &str) -> Option<String> {
    let slashed = slashes(real_path);
    // ASCII lowercasing keeps byte offsets, so `slashed` can be sliced by them.
    let normalized = slashed.to_ascii_lowercase();
    let segment = format!("/lib/node_modules/{}/", package.to_lowercase());
    let index = normalized.rfind(&segment)?;
    let before = &normalized[..index];
    if before.contains("/node_modules/") {
        return None;
    }
    if let Some(position) = before.find("/mise/installs/") {
        let tool = before[position + "/mise/installs/".len()..]
            .split('/')
            .next()
            .unwrap_or("");
        if tool != "node" {
            return None;
        }
    }
    Some(if index == 0 {
        "/".to_owned()
    } else {
        slashed[..index].to_owned()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Homebrew {
    pub cask: bool,
    pub name: String,
    pub prefix: String,
}

/// `<prefix>/Cellar/<formula>/<version>/...` or `<prefix>/Caskroom/<cask>/<version>/...`.
pub fn homebrew_keg(real_path: &str) -> Option<Homebrew> {
    let slashed = slashes(real_path);
    let lower = slashed.to_ascii_lowercase();
    for (marker, cask) in [("/cellar/", false), ("/caskroom/", true)] {
        if let Some(index) = lower.find(marker) {
            let rest = &slashed[index + marker.len()..];
            let mut parts = rest.split('/');
            let name = parts.next().filter(|name| !name.is_empty())?;
            // A version directory and something inside it.
            parts.next().filter(|version| !version.is_empty())?;
            parts.next()?;
            return Some(Homebrew {
                cask,
                name: name.to_owned(),
                prefix: slashed[..index].to_owned(),
            });
        }
    }
    None
}

/// Paths mise owns. npm globals under mise's own Node belong to npm instead.
pub fn is_mise_path(path: &str) -> bool {
    let normalized = normalize(path);
    (normalized.contains("/mise/installs/") && !normalized.contains("/mise/installs/node/"))
        || normalized.contains("/mise/shims/")
        || normalized.contains("/mise/command-wrappers/")
}

/// A mise shim or command wrapper is a link to the `mise` binary itself.
pub fn is_mise_executable(real_path: &str) -> bool {
    let normalized = normalize(real_path);
    let name = normalized.rsplit('/').next().unwrap_or("");
    name == "mise" || name == "mise.exe"
}

/// A `#!` script that runs `mise x` / `mise exec`.
pub fn is_mise_wrapper_script(script: &str) -> bool {
    if !script.starts_with("#!") {
        return false;
    }
    script.lines().any(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        words
            .windows(2)
            .any(|pair| pair[0].ends_with("mise") && matches!(pair[1], "x" | "exec"))
    })
}

/// The Nix store is read-only: no updater can replace a CLI that lives there.
pub fn is_nix_store(path: &str) -> bool {
    normalize(path).starts_with("/nix/store/")
}

pub fn is_winget_claude(path: &str) -> bool {
    let normalized = normalize(path);
    let Some(rest) = normalized.get(3..) else {
        return false;
    };
    if !(normalized.as_bytes().get(1) == Some(&b':') && normalized.as_bytes().get(2) == Some(&b'/'))
    {
        return false;
    }
    let roots = rest.starts_with("program files/winget/")
        || rest.starts_with("program files (x86)/winget/")
        || (rest.starts_with("users/") && rest.contains("/appdata/local/microsoft/winget/"));
    roots
        && normalized.ends_with("/claude.exe")
        && (normalized.contains("/winget/links/")
            || normalized.contains("/winget/packages/anthropic.claudecode_"))
}

/// Chocolatey's shim is not a symlink, so the package layout is matched directly.
pub fn is_chocolatey_claude(path: &str, chocolatey_install: Option<&str>) -> bool {
    let normalized = normalize(path);
    if normalized.ends_with("/chocolatey/bin/claude.exe")
        || normalized.ends_with("/chocolatey/lib/claude-code/tools/claude.exe")
    {
        return true;
    }
    let Some(root) = chocolatey_install.filter(|root| !root.trim().is_empty()) else {
        return false;
    };
    let prefix = normalize(root.trim_end_matches(['/', '\\']));
    normalized == format!("{prefix}/bin/claude.exe")
        || normalized == format!("{prefix}/lib/claude-code/tools/claude.exe")
}

pub fn is_pnpm_global(path: &str) -> bool {
    let normalized = normalize(path);
    [
        "/.local/share/pnpm/",
        "/library/pnpm/",
        "/local/share/pnpm/",
        "/appdata/local/pnpm/",
        "/pnpm/global/",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

pub fn is_bun_global(path: &str) -> bool {
    normalize(path).contains("/.bun/bin/")
}

pub fn is_yarn_global(path: &str) -> bool {
    let normalized = normalize(path);
    normalized.contains("/yarn/global/node_modules/")
        || normalized.contains("/yarn/data/global/node_modules/")
}

/// Vite+ shims live in `<VP_HOME>/bin`.
pub fn is_vite_plus_global(path: &str) -> bool {
    let normalized = normalize(path);
    [
        "/.vite-plus/bin/",
        "/.local/share/vite-plus/bin/",
        "/appdata/local/vite-plus/bin/",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

pub fn is_volta_shim(real_path: &str) -> bool {
    let normalized = normalize(real_path);
    let name = normalized.rsplit('/').next().unwrap_or("");
    name == "volta-shim" || name == "volta-shim.exe"
}

/// Claude Code's native installer layout: `~/.local/bin/claude.exe` on
/// Windows, or a link into `${XDG_DATA_HOME:-~/.local/share}/claude/versions`.
/// A plain `~/.local/bin/claude` file on Unix is a wrapper `claude update` cannot see.
pub fn is_claude_native(path: &str) -> bool {
    let normalized = normalize(path);
    normalized.ends_with("/.local/bin/claude.exe")
        || normalized.contains("/.local/share/claude/")
        || normalized.contains("/claude/versions/")
}

/// Codex's standalone installer home: everything before `/packages/standalone/`.
pub fn codex_standalone_home(path: &str) -> Option<String> {
    // Find in a slash-normalized copy, slice the original: separators stay native.
    let index = slashes(path)
        .to_ascii_lowercase()
        .find("/packages/standalone/")?;
    let home = &path[..index];
    // Keep a root separator: `C:` alone is drive-relative.
    let is_root = home.is_empty() || (home.len() == 2 && home.as_bytes()[1] == b':');
    Some(if is_root {
        path[..index + 1].to_owned()
    } else {
        home.to_owned()
    })
}

/// OpenCode's curl installer binary: `<HOME>/.opencode/bin/opencode[.exe]`.
pub fn opencode_native_home(path: &str) -> Option<String> {
    let lower = slashes(path).to_ascii_lowercase();
    let suffix = if lower.ends_with("/.opencode/bin/opencode") {
        "/.opencode/bin/opencode".len()
    } else if lower.ends_with("/.opencode/bin/opencode.exe") {
        "/.opencode/bin/opencode.exe".len()
    } else {
        return None;
    };
    let home = &path[..path.len() - suffix];
    Some(if home.is_empty() {
        "/".into()
    } else {
        home.to_owned()
    })
}

/// The OpenCode desktop app bundles its own CLI; only the app updates it.
pub fn is_opencode_desktop_bundle(path: &str) -> bool {
    let normalized = normalize(path);
    normalized.contains("/@opencode-aidesktop/resources/")
        || normalized.contains("/opencode.app/contents/")
}

/// T3 Code's managed tool directory. T3 installs and updates these itself.
pub fn is_t3_managed(path: &str) -> bool {
    normalize(path).contains("/.t3/tools/")
}

/// Parse `choco search <id> --exact --limit-output`: `<id>|<version>` lines.
pub fn chocolatey_latest(output: &str, package: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut parts = line.trim().splitn(2, '|');
        let id = parts.next()?;
        let version = parts.next()?.trim();
        (id.eq_ignore_ascii_case(package) && !version.is_empty()).then(|| version.to_owned())
    })
}

/// Quote one word for display in the platform's usual shell.
pub fn quote(word: &str, windows: bool) -> String {
    let safe =
        |c: char| c.is_ascii_alphanumeric() || "_./:@=-".contains(c) || (windows && c == '\\');
    if !word.is_empty() && word.chars().all(safe) {
        word.to_owned()
    } else if windows {
        format!("'{}'", word.replace('\'', "''"))
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// A display command: PowerShell needs `&` before a quoted executable.
pub fn display_command(executable: &str, args: &[String], windows: bool) -> String {
    let program = quote(executable, windows);
    let program = if windows && program != executable {
        format!("& {program}")
    } else {
        program
    };
    std::iter::once(program)
        .chain(args.iter().map(|arg| quote(arg, windows)))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_prefix_is_proven_by_the_real_package_path() {
        assert_eq!(
            npm_global_prefix(
                "/usr/local/lib/node_modules/@openai/codex/bin/codex.js",
                "@openai/codex"
            )
            .as_deref(),
            Some("/usr/local")
        );
        assert_eq!(
            npm_global_prefix(
                "/home/u/.local/share/mise/installs/node/22.1.0/lib/node_modules/@openai/codex/bin/codex.js",
                "@openai/codex"
            )
            .as_deref(),
            Some("/home/u/.local/share/mise/installs/node/22.1.0")
        );
        // A project dependency is not a global install.
        assert_eq!(
            npm_global_prefix(
                "/repo/node_modules/x/lib/node_modules/@openai/codex/bin/codex.js",
                "@openai/codex"
            ),
            None
        );
        // A mise tool layout only resembles npm.
        assert_eq!(
            npm_global_prefix(
                "/home/u/.local/share/mise/installs/npm-openai-codex/1.0.0/lib/node_modules/@openai/codex/bin/codex.js",
                "@openai/codex"
            ),
            None
        );
        // Another package does not prove this one.
        assert_eq!(
            npm_global_prefix(
                "/usr/lib/node_modules/opencode-ai/bin/opencode",
                "@opencode/cli"
            ),
            None
        );
    }

    #[test]
    fn homebrew_kegs_and_casks_are_recognized() {
        assert_eq!(
            homebrew_keg("/opt/homebrew/Cellar/opencode/1.2.3/bin/opencode"),
            Some(Homebrew {
                cask: false,
                name: "opencode".into(),
                prefix: "/opt/homebrew".into()
            })
        );
        assert_eq!(
            homebrew_keg("/opt/homebrew/Caskroom/grok/0.9.0/grok").map(|keg| keg.cask),
            Some(true)
        );
        assert_eq!(homebrew_keg("/opt/homebrew/bin/opencode"), None);
    }

    #[test]
    fn mise_nix_and_package_manager_layouts() {
        assert!(is_mise_path(
            "/home/u/.local/share/mise/installs/claude/latest/bin/claude"
        ));
        assert!(!is_mise_path(
            "/home/u/.local/share/mise/installs/node/22/lib/node_modules/x"
        ));
        assert!(is_mise_path("/home/u/.local/share/mise/shims/codex"));
        assert!(is_mise_executable("/usr/local/bin/mise"));
        assert!(is_mise_wrapper_script(
            "#!/bin/sh\nexec mise x -- claude \"$@\"\n"
        ));
        assert!(!is_mise_wrapper_script("#!/bin/sh\nexec claude\n"));
        assert!(is_nix_store("/nix/store/abc-claude/bin/claude"));
        assert!(is_pnpm_global(r"C:\Users\u\AppData\Local\pnpm\codex.cmd"));
        assert!(is_bun_global("/home/u/.bun/bin/codex"));
        assert!(is_yarn_global(
            "/home/u/.config/yarn/global/node_modules/@openai/codex/bin/codex.js"
        ));
        assert!(is_vite_plus_global("/home/u/.vite-plus/bin/codex"));
        assert!(is_volta_shim(r"C:\Program Files\Volta\volta-shim.exe"));
    }

    #[test]
    fn windows_claude_installers_are_recognized() {
        assert!(is_winget_claude(
            r"C:\Users\u\AppData\Local\Microsoft\WinGet\Links\claude.exe"
        ));
        assert!(is_winget_claude(
            r"C:\Program Files\WinGet\Packages\Anthropic.ClaudeCode_abc\claude.exe"
        ));
        assert!(!is_winget_claude(r"C:\Users\u\.local\bin\claude.exe"));
        assert!(is_chocolatey_claude(
            r"C:\ProgramData\chocolatey\bin\claude.exe",
            None
        ));
        assert!(is_chocolatey_claude(
            r"D:\choco\lib\claude-code\tools\claude.exe",
            Some(r"D:\choco\")
        ));
        assert!(!is_chocolatey_claude(r"D:\choco\bin\claude.exe", None));
        assert_eq!(
            chocolatey_latest("claude-code|2.1.3\r\n", "claude-code").as_deref(),
            Some("2.1.3")
        );
        assert_eq!(chocolatey_latest("", "claude-code"), None);
    }

    #[test]
    fn native_layouts() {
        assert!(is_claude_native(r"C:\Users\u\.local\bin\claude.exe"));
        assert!(is_claude_native(
            "/home/u/.local/share/claude/versions/2.1.0"
        ));
        assert!(!is_claude_native("/home/u/.local/bin/claude"));
        assert_eq!(
            codex_standalone_home("/home/u/.codex/packages/standalone/current/bin/codex")
                .as_deref(),
            Some("/home/u/.codex")
        );
        assert_eq!(
            codex_standalone_home(r"C:\packages\standalone\codex.exe").as_deref(),
            Some(r"C:\")
        );
        assert_eq!(
            opencode_native_home(r"C:\Users\u\.opencode\bin\opencode.exe").as_deref(),
            Some(r"C:\Users\u")
        );
        assert_eq!(opencode_native_home("/usr/bin/opencode"), None);
        assert!(is_opencode_desktop_bundle(
            r"C:\Users\u\AppData\Local\Programs/@opencode-aidesktop/resources\opencode-cli.exe"
        ));
        assert!(is_t3_managed(
            r"C:\Users\u\.t3\tools\antigravity-acp\win32-x64\versions\h\agy_acp_server.exe"
        ));
    }

    #[test]
    fn display_commands_quote_for_the_shell() {
        assert_eq!(
            display_command("npm", &["install".into(), "-g".into()], false),
            "npm install -g"
        );
        assert_eq!(
            display_command(r"C:\Program Files\x\claude.exe", &["update".into()], true),
            r"& 'C:\Program Files\x\claude.exe' update"
        );
        assert_eq!(quote("it's", false), r"'it'\''s'");
    }
}
