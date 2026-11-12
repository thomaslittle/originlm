# Provider CLI updates

`originlm providers check` reports, for every discovered provider CLI binary,
the installed version, the latest version from the installer that owns it, and
one of four statuses. `originlm providers update` runs that owning installer,
and only when it is proven to own the binary. Both commands are for the local
operator. No broker, SDK, MCP or browser request can run them, and discovery
and refresh never run them either. See the [grant boundary](#grant-boundary).

These commands update the provider CLIs on this host (`claude`, `codex`,
`opencode`, `grok`, `pi`, ...). They do not update OriginLM (that is
`originlm-update`, which verifies Ed25519-signed releases), and they never
touch an account, login or subscription.

The rules are ported from T3 Code's provider maintenance
(`docs/internals/providers.md`, "Provider updates run only through the owning
installer") into Rust in `crates/originlm-cli/src/provider_updates/`.

## Check

```sh
originlm providers check [--json] [--provider PROVIDER]... [--home ABSOLUTE_DIRECTORY]
```

For each install:

- **Installed version**: `<executable> --version`, with the exact command and
  first output line kept as `versionEvidence`.
- **Latest version**: taken from the installer that owns the binary, and
  `latestSource` names where it came from:
  - npm and native installs that share npm's version train: `npm view
    <package>@latest version` through the npm on `PATH`. That uses npm's
    configured registry, auth and proxy, and a failure is not retried against
    the public registry.
  - Homebrew: `brew info --json=v2 <name>`, because casks can trail npm by hours.
  - mise: `mise latest <tool>`, because `minimum_release_age` can hold mise
    behind npm.
  - Chocolatey (Claude): `choco search claude-code --exact --limit-output`.
  - Claude Code native: `https://downloads.claude.ai/claude-code-releases/<channel>`,
    with the channel read from `autoUpdatesChannel` in the user settings or
    managed settings.
  - Codex standalone: `https://releases.openai.com/codex/channels/latest`.
  - Muse Code: `https://api.meta.ai/muse-code/channels/<MUSE_CHANNEL>`.
- **Status**:
  - `current`
  - `behind_latest`: newer exists and OriginLM can run the owning installer.
  - `manual_only`: newer exists, but no runnable installer is proven.
    `reason` and `manualCommand` say what to do.
  - `unknown`: either version could not be read. It is never guessed.

`check` contacts those registries and runs `--version`. It prints private
executable paths.

## Update

```sh
originlm providers update (--provider PROVIDER | --all) [--executable ABSOLUTE_PATH] [--dry-run] [--json]
```

- `--dry-run` prints the exact command and its ownership proof for every behind
  install, and changes nothing. It takes no lock and runs no installer.
- Only `behind_latest` installs run. `current` reports `already_current`.
  `manual_only` and `unknown` report `skipped` with their reason, and the manual
  command is never executed.
- One update at a time per install location: a lock file under
  `<state dir>/provider-updates/` is keyed by the installer's lock key
  (`npm-global:<prefix>`, `homebrew:<prefix>`, `mise:<mise>:<tool>`,
  `<provider>-native:<real path>`, ...).
- Immediately before running, ownership is resolved again and the version is
  probed again. The update is refused if the real path, lock key, command,
  environment or installed version changed since the status was computed.
- stdin is at EOF. The timeout is 30 minutes, because native updaters download
  hundreds of MB without showing progress. On timeout the whole process tree
  is killed.
- **Success** means the re-probed version is readable and at least the latest
  version. Exit code 0 alone is not success, so an updater that exits 0 without
  updating is reported as a failure. Any other failure reports the installer's
  last stderr line, or its last stdout line.
- Installs that share one owner and command, such as two profiles of one npm
  global, run the installer once.
- After a successful update, the provider's connections are discovered again,
  and their cached metadata (including model lists) in `state.db` is dropped.
  The next metadata probe then asks the new CLI, so a model that only exists on
  a newer CLI appears only after that CLI is installed. A running broker keeps
  its in-memory model list until its next metadata refresh.
- Running sessions are not restarted. They keep the old CLI until they end,
  and the new version applies to the next session. For OpenCode, restart the
  configured local OpenCode server to use the new version.

Exit status is 0 unless an update failed (1). Invalid arguments exit with 2.

## Installer ownership

An installer runs only when the binary's **real path** (with symlinks
followed) proves that installer owns it:

| Installer | Proof | Command |
|---|---|---|
| npm | `<prefix>/lib/node_modules/<pkg>/` in the real path. On Windows, the shim beside `node_modules/<pkg>/package.json`. A nested `node_modules` or a mise tool layout does not count | `npm install -g --prefix <prefix> --allow-scripts=<pkg> <pkg>@latest` (the prefix is pinned) |
| Homebrew | A versioned `Cellar/<formula>/` or `Caskroom/<cask>/` directory under a prefix that `brew --prefix` confirms | `brew upgrade [--cask] <name>` |
| mise | Checked first. `mise which <tool>` resolves the binary, and the binary is inside an active install of `<tool>` requested as `latest`, reached through a shim or the tool's `latest` link | `mise upgrade <tool>` |
| Volta | The real path is a `volta-shim` link and Volta holds `tools/image/packages/<pkg>` | `volta install <pkg>@latest` |
| pnpm / Bun / Yarn / Vite+ | The binary sits in that manager's global directory | `pnpm add -g <pkg>@<exact latest>` (an exact version, because `minimumReleaseAge` holds `@latest` back), `bun i -g`, `yarn global add`, `vp i -g` |
| Native updater | The provider's own layout: Claude `~/.local/bin/claude.exe` or `.../claude/versions/`, Codex `.../packages/standalone/`, OpenCode `~/.opencode/bin/opencode` | `claude update`, `codex update` (`CODEX_HOME` set to the install's home), `opencode upgrade --method curl` (`HOME` set to the install's home) |
| Self-detecting updater | Pi and Grok outside any proven package manager or Nix | `pi update --self`, `grok update` |

Each of these stays manual-only, with its reason shown:

- A pinned or version-directory mise install. OriginLM never edits mise config.
- An install in the Nix store, which is read-only.
- Chocolatey Claude. `choco upgrade claude-code` is shown but not run, because
  `claude update` through the shim would install a second copy.
- WinGet Claude, where `claude update` exits 0 without updating.
- Claude, Codex or OpenCode outside their native layout.
- A Grok cask whose Homebrew prefix cannot be verified, because `grok update`
  would install a second copy.
- An OpenCode 1.x install under mise, because mise's tool crosses into 2.x.

## Per-provider support

| Provider | Update support | Latest-version source | Notes |
|---|---|---|---|
| Claude Code | Installer when proven (native, npm, mise, Homebrew, ...). Manual for WinGet, Chocolatey, Nix and other layouts | Native: release channel. Chocolatey: `choco search`. Otherwise npm / brew / mise | Running sessions keep the old CLI |
| Codex | Installer when proven (standalone, npm, mise, Homebrew, ...). Manual otherwise | Standalone: release channel. Otherwise npm / brew / mise | |
| OpenCode | Installer when proven, within the installed generation (1.x `opencode-ai`, 2.x `@opencode/cli`). The desktop app's bundled CLI is manual | npm / brew / mise | Restart the local OpenCode server after an update |
| Gemini CLI | Installer when npm / pnpm / Bun / Yarn / Volta / Vite+ / Homebrew is proven. The T3-managed copy is manual | npm / brew | |
| Grok | mise or a verified Homebrew cask, otherwise `grok update`. Manual for Nix and unverified casks | npm `@xai-official/grok` / brew / mise | |
| Pi | Installer when proven, otherwise `pi update --self` (manual inside an unproven `node_modules`) | npm / mise | |
| Muse Code | mise, or the official launcher's synchronous refresh on macOS/Linux. Manual on Windows and for standalone binaries | Muse release channel / mise | |
| Cursor | Manual. OriginLM proves no installer and reads no release channel | None, so `unknown` | |
| Antigravity | Manual. T3 Code installs and updates its managed runtime under `~/.t3/tools`, and the Antigravity app updates its own | None, so `unknown` | |
| Kiro | Manual. Kiro updates itself in the background, and `kiro-cli update` only reports | None, so `unknown` | |
| fx | Manual. `fx upgrade` is shown, never run | None, so `unknown` | |
| Ollama, LM Studio | Unsupported. Their desktop apps update them | None, so `unknown` | |
| GitHub Copilot | Unsupported | None | |

## Grant boundary

`providers update` mutates the host: it runs a package manager or a provider's
own updater as the current user. It is an explicit, local-operator-only
operation:

- It never runs automatically, in the background, on discovery or refresh, or
  at broker startup.
- There is no broker JSON-RPC method, no SDK call and no MCP tool for it. An
  application connected to OriginLM cannot reach host mutation, and the
  ordinary app token is not widened. `providers check` is also CLI-only,
  because it executes `--version` and contacts registries.
- A future desktop Providers page must reach these operations through the
  administrator surface with an explicit grant, never through the normal
  application token. That integration is not built yet.
