# OriginLM · Desktop

OriginLM Desktop is a native desktop application for OriginLM: a native GPUI manager for the AI
access already configured on this machine. It reads the shared broker through
its public authenticated JSON-RPC API and never opens the broker database, reads
provider credentials or starts discovery on its own.

The product is branded **OriginLM** (graphite `#231F20`, amber `#EBA31A`,
off-white `#FAF9F5`, Red Hat Display SemiBold). The crate is `originlm-desktop` and the binary is `originlm-desktop`.
See the [migration guide](../../docs/integration/migration.md) before connecting an existing runtime.

## Brand assets

The sidebar and window icon use the supplied full-color GPUI PNG. Horizontal light/dark lockups and standalone amber/ink/white marks remain available for exports. [Provenance](assets/brand/PROVENANCE.md) records the replacement source. The Windows executable embeds `brand/native/windows/originlm.ico` through `build.rs`, so Explorer, shortcuts and the taskbar use the OriginLM mark instead of a generic binary icon; release builds also use the Windows GUI subsystem. The window sets the `originlm-desktop` application id for Linux grouping, and `brand/native/linux/` carries the hicolor icons and desktop entry. macOS `OriginLM.icns` is ready for the app bundle; installing/bundling remains a release task. Tray resources remain under `brand/native/tray/`. Existing Red Hat Display and provider icon notices are preserved. The in-app inner opening follows the pointer while the outer icon stays fixed; reduced motion keeps the original static image. Set `ORIGINLM_REDUCED_MOTION=1` to disable decorative motion, or `0` to override the startup OS preference.

Built on [GPUI Kit](https://gpui-kit.com) with the
[GPUI Fast](https://github.com/longbridge/gpui-fast) backend: one dependency
graph, one component system, one theme.

## Run

```sh
cargo run -p originlm-desktop
```

No token file is required for a first run. On launch the app looks for an
existing shared broker credential, most privileged first:

1. `$LAI_DESKTOP_TOKEN_FILE`, then `$LAI_BROKER_TOKEN_FILE`
2. the platform state directory (`%LOCALAPPDATA%\originlm` on Windows,
   `~/.local/state/originlm` on Linux, `~/Library/Application Support/originlm`
   on macOS): `broker-admin-token.json`, then `broker-token.json`
3. `~/.originlm/broker-admin-token.json`, then `~/.originlm/broker-token.json`
4. the documented control-center path
   (`~/.local/state/originlm/fedora-shared/control-admin-token.json`)

Each candidate is probed. When none answers, the app hosts a private broker
in-process and mints an in-memory administrative credential for this window; no
credential is written to disk and the runtime ends with the app. Passive
discovery plus read-only metadata probes for the discovered providers are
enabled so the window fills in and can verify sign-in state. The Playground gets
a read-only execution grant for a dedicated empty workspace; paid or
unknown-billing routes still require the per-run consent switch, and account
identity reveal stays off. An explicit `--token-file` always fails closed
instead: it is never replaced by a local runtime.

An admin descriptor enables the Connected apps view. An application descriptor
still works for everything else; apps management then explains why it is
unavailable. Credentials are never displayed, logged or exported.

Useful flags and environment overrides:

| Option | Purpose |
| --- | --- |
| `--example` | Labelled synthetic data; reads no accounts and runs nothing |
| `--refresh SECONDS` | Snapshot read cadence (5–600, default 30) |
| `LAI_DESKTOP_THEME=light\|dark` | Start in a specific theme |
| `LAI_DESKTOP_PAGE=<title>` | Open a page for screenshots or testing |
| `LAI_DESKTOP_WINDOW=1280x820` | Window size for layout checks (minimum 1000×660) |
| `LAI_DESKTOP_STATE_DIR` | History and preference directory |
| `LAI_DESKTOP_MINI_VIEW=1` | Example mode only: open with the mini view showing |
| `LAI_DESKTOP_EXAMPLE_STATE=loading\|offline\|stale\|empty` | Example mode only: preview the shell's loading, offline, stale and empty states |

## What works

Navigation is grouped as **Usage** (Overview, Usage history), **Inventory**
(Accounts, Models), **Build** (Developer, Apps & grants, Sessions, Playground)
and **System** (Diagnostics, Settings). Earlier page titles still work for
`LAI_DESKTOP_PAGE` and saved preferences.

- **Overview**: a runtime strip, then one card per account with a segmented
  meter for every allowance window it reports, percent left, reset countdown and
  freshness. Accounts that report nothing say so (or say they were not checked
  yet) and offer a metadata probe; unknown readings stay unknown. Below: what
  needs attention, a developer summary and observed turn cost.
- **Usage history**: every allowance window as a list with meters; the selected
  window's recorded readings (percent left) over 24h/7d/30d as a chart or table,
  with gaps left as gaps; the reset schedule.
- **Accounts**: one compact table of connections (plan, sign-in, stale flag,
  model count) with an inspector: identity, discovery diagnostics, routes with
  their supported capabilities and billing, and a metadata probe.
- **Models**: search, an All/Favorites filter, 44px rows with a labelled star,
  and an inspector with copyable native and route IDs, capabilities and every
  provider option.
- **Developer**: the permission model (discover, select, grant, run), SDK quick
  starts in TypeScript, Rust and Python with install commands, a protocol
  reference, every route on this machine with its supported capabilities,
  sign-in state and a copyable route ID, recent turns from the runtime's cost
  evidence (tokens, cost, basis), apps with access, runtime facts and guides.
- **Apps & grants**: apps connected right now with their scopes, app-scoped
  revocation with an explicit confirmation, and how an app gets access (grants
  file, `Broker::authorize`, `grants.request`). This window does not approve
  requests; approval belongs to the host or the broker owner.
- **Sessions**: sessions this window started, with status, usage and cancel.
  Sessions opened elsewhere are described as not visible here.
- **Playground**: one explicit, read-only turn through `sessions.create` →
  `turns.start` → events, streamed into the window. Paid and unknown-billing
  routes require the per-run consent switch.
- **Diagnostics**: health in plain language, issues with evidence and numbered
  remediation, diagnostic lines, and the redacted report (copy to clipboard or
  save to a file).
- **Settings**: appearance, data source (runtime, endpoint, credential *kind*,
  never the token; state folder), refresh cadence, layout, local model prices
  (advanced, last) and about.
- **Mini view**: `Ctrl+Shift+M` shows up to four allowance windows over any page.
- **Command palette**: `Ctrl/⌘ K` for pages and models; `Ctrl/⌘ R` refreshes;
  `Ctrl/⌘ T` switches theme; `Ctrl/⌘ B` toggles the sidebar; `Esc` closes
  overlays.

**States.** Pages that need a snapshot show a loading state until the first
one arrives and an offline state (with the error and a Retry button) if none
can be read. If the runtime stops answering after a good read, the last
snapshot stays on screen under a "Stale: showing the last snapshot, read N ago"
banner, the header says "Last read N", and the status bar says the runtime is
not answering. Empty states say what was looked for and what to do next.

**Keyboard and accessibility.** Tab and Shift+Tab move through every control in
reading order (title bar, sidebar, header, page); Enter or Space activates.
Keyboard focus draws a 2px amber ring (`focus_visible`, so pointer clicks do
not show it). Sidebar items, tabs, rows, copy buttons and icon-only buttons
carry accessibility roles and labels; icon-only buttons also have tooltips.
Text and meter colours meet WCAG AA in both themes; `cargo test` checks the
contrast pairs (`theme::tests::theme_pairs_meet_wcag`).

Shared layout and components (spacing, type and radius scales, panels, state
blocks, banners, tabs, selectable rows, tables, meters, chips, copyable values,
icon buttons, code samples, the focus ring) live in `src/ds.rs`; `src/ui.rs`
keeps only small primitives and formatters. The design audit, decisions and
screenshots are in
[`docs/design/desktop-audit.md`](../../docs/design/desktop-audit.md). On
Windows, `scripts/capture_desktop_example.ps1` refreshes every page in both
themes, the mini view, the example states and other window sizes.

Preferences (theme, page, refresh cadence, favorites) and the bounded usage
history live in the platform state directory under `desktop/`. History is
collected only from observations the broker actually reports and is capped and
pruned; it never fabricates earlier points.

## Design

Graphite surfaces, warm off-white text and a restrained amber accent, with a
paper-light theme. Type scale: 11px captions, 12px secondary text, 13px body,
15px titles, 22px page headings; nothing smaller. 4px spacing scale, 4px control
and 6px panel radii, hairline borders. The UI face is the bundled Red Hat
Display; the mono face is the first installed of Cascadia Mono or Consolas
(Windows), SF Mono or Menlo (macOS), JetBrains Mono, DejaVu Sans Mono or Adwaita
Mono (Linux), falling back to the platform `monospace`. Charts are GPUI Kit's
native plots with a table alternative.

## Known limits

- Verified interactively on Windows 11 (example mode, every page in both themes
  at 1000×660, 1440×920 and 2560×1440, plus live mode against this machine's
  runtime) and earlier on Linux/X11 (Xvfb) and GNOME Wayland. macOS has not been
  run.
- The Playground is deliberately read-only. Paid and unknown-billing routes stay
  off until the user turns on the per-run consent switch, and the private runtime
  grants execution only in its dedicated playground workspace. Provider-native
  option selection is not exposed yet, though models show the supported options.
- The broker's `apps.list` does not report execution scope, so the Connected
  apps page says so rather than guessing.
- Admin credentials cannot reveal account identity in alpha.18; the app shows
  "identity hidden" instead of implying it can see it.
- Usage charts plot the share of each window **left**, matching the meters,
  and replace a flat line with the current figure when nothing changed in the
  range. A shared broker is asked for one bounded metadata probe per
  authenticated connection whose allowance is missing or stale, so usage
  returns after a broker restart without polling; the private runtime lets
  its background lanes refresh instead.
- Passive discovery can see several command shims, profile roots or bundled copies of one client. The desktop collapses them to one account per provider unless the service reports distinct account ids. Overview is one provider list, not a summary plus a second allowance grid: every account always appears with its meter, its explicit no-quota state, or its local token totals. Quota evidence read from a folded duplicate stays attached to the visible account. Discovery installation and profile indices are removed from user-facing names, while genuinely distinct account ids keep an account suffix.
- Codex allowance windows are named from the provider duration (`5 hr`, `Weekly`) rather than the raw `primary`/`secondary` bucket keys. OpenCode models carry their plan (`Free`, `Zen`, `Go`, or `Local`) next to the model name.
- `cargo test -p originlm-desktop` includes native GPUI end-to-end tests. They render the real Overview, Models and Playground pages in the deterministic test window and drive them with GPUI pointer events, following Zed's `#[gpui::test]`/`TestAppContext` pattern. They never use Windows UI Automation, SendKeys or OS focus. The Playground test runs two in-process OriginLM sessions against a model-switch conversation and asserts the second model receives the first turn through the real handoff: `cargo test -p originlm-desktop native_playground_model_switch_hands_off_the_conversation`. The Playground itself uses a Twinhaus-inspired chat surface: right-aligned user bubbles, full-width assistant prose, a live status pulse, staggered message entry, and composer controls below the input for image attachments, model selection with an inline picker, reasoning effort, and Send that becomes an animated Stop.

## Automated end-to-end gate

Run the desktop UI, handoff and HTTP request-shaping suites together:

```sh
python scripts/test_desktop_e2e.py
```

This runs the GPUI Playground tests plus the service model-switch and HTTP adapter execution suites. It covers two-turn model switching, prior-context seeding, transcript retention, no double seeding, resume after switch, target-budget trimming, free-model billing consent, and large-image downscaling before `inputs.put`.
- Availability meters are green at 50% or more remaining, amber below 50%, and red below 20%. The Claude burst and OpenAI marks are used for Claude Code and Codex.
- Startup renders the cached snapshot first; metadata and usage refresh run in the background rather than queuing one probe behind another on the broker worker. The private runtime keeps its own `desktop-metadata.db`, hydrates the last observations, and reverifies them in the background; it never shares the CLI broker database.
- Cursor is probed through the installed `cursor-agent` CLI: signed-in state,
  plan tier, the native model catalog and included-usage percentages. The
  desktop app's private runtime opts in to Cursor's dashboard allowance read;
  a separately started CLI broker still needs `--allow-usage cursor`. The
  session token stays in memory for that one request and is never logged,
  serialized or exported.
- OpenCode 2 is read from its running local service (owner-only descriptor,
  loopback API): authentication is marked observed only when the service
  returns authenticated workspace limits, and its rolling/weekly/monthly
  allowance is shown. OpenCode and OpenCode Go text sessions go through the
  installed client's transport and are offered in the Playground when the
  runtime grants execution. OpenCode 2 sessions are text-only and do not
  support native resumption.
- Inspectors (Accounts, Models) keep a fixed width and stack under the list
  below about 1000px of content width; only the sidebar is drag-resizable. Model
  lists are search-filtered and capped at 300 rows, not virtualized.
- The command palette does not yet move its selection with arrow keys; Enter
  activates the best match. Menus/dialogs are inline rather than modal.
- Accessibility roles and labels are set through GPUI's AccessKit integration;
  screen-reader output (Narrator, VoiceOver, Orca) has not been verified. There
  is no tray integration.
