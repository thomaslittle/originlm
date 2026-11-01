# OriginLM · Desktop

OriginLM Desktop is a native desktop application for OriginLM: a native GPUI manager for the AI
access already configured on this machine. It reads the shared broker through
its public authenticated JSON-RPC API and never opens the broker database, reads
provider credentials or starts discovery on its own.

The product is branded **OriginLM** (graphite `#231F20`, amber `#EBA31A`,
off-white `#FAF9F5`, Red Hat Display SemiBold). The crate is `originlm-desktop` and the binary is `originlm-desktop`.
See the [migration guide](../../docs/integration/migration.md) before connecting an existing runtime.

## Brand assets

The sidebar and window icon use the supplied full-color GPUI PNG. Horizontal light/dark lockups and standalone amber/ink/white marks remain available for exports. [Provenance](assets/brand/PROVENANCE.md) records the replacement source. Platform ICO, ICNS, Linux and tray resources are under `brand/native/`; wiring platform installers remains a release task. Existing Red Hat Display and provider icon notices are preserved. The in-app inner opening follows the pointer while the outer icon stays fixed; reduced motion keeps the original static image. Set `ORIGINLM_REDUCED_MOTION=1` to disable decorative motion, or `0` to override the startup OS preference.

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
| `--refresh SECONDS` | Snapshot read cadence (5–600, default 30) |
| `LAI_DESKTOP_THEME=light\|dark` | Start in a specific theme |
| `LAI_DESKTOP_PAGE=<title>` | Open a page for screenshots or testing |
| `LAI_DESKTOP_WINDOW=1280x820` | Window size for layout checks |
| `LAI_DESKTOP_STATE_DIR` | History and preference directory |

## What works

- **Overview** — access summary, the most constrained allowance windows, next
  reset, locally collected usage history, per-provider authentication and open
  issues.
- **Providers & accounts** — discovered installations, profiles, plans, auth
  evidence, diagnostics, routes and billing, with copyable identifiers and a
  metadata probe action.
- **Models** — search, favorites, provider-native IDs, capabilities, context
  evidence and option descriptors. Selection always shows its route and billing.
- **Usage** — 24h/7d/30d ranges, per-pool windows, remaining-share line with
  gaps left as gaps, reset countdowns, freshness and a tabular alternative.
- **Connected apps** — live broker clients, permissions as the broker reports
  them, and app-scoped revocation with an explicit confirmation.
- **Sessions** — sessions this app started, their status, usage and cancellation.
  External sessions are explicitly described as unobservable.
- **Playground** — one explicit, read-only turn through
  `sessions.create` → `turns.start` → `events.read`, streamed into the window.
  Paid and unknown-billing routes require the per-run consent switch.
- **Diagnostics** — health, plain-language issues with evidence and remediation,
  revision copy and a redacted diagnostic export.
- **Settings** — theme, refresh cadence, the socket and credential *paths* (never
  the token), and where local history lives.
- **Command palette** — `Ctrl/⌘ K` for pages and models; `Ctrl/⌘ R` refreshes;
  `Ctrl/⌘ T` switches theme; `Esc` closes overlays.

Preferences (theme, page, refresh cadence, favorites) and the bounded usage
history live in `~/.local/state/originlm/desktop/`. History is collected
only from observations the broker actually reports and is capped and pruned; it
never fabricates earlier points.

## Design

Graphite surfaces, warm off-white text and a restrained amber accent, with a
matching paper-light theme; one type scale, tabular figures, a consistent 2/4/8
spacing rhythm and Lucide icons from the component library. Charts are GPUI
Kit's native plots, with hover values, readable units, and a table alternative
for the usage series.

## Known limits

- Verified interactively on Linux/X11 (Xvfb, software rendering) and launched on
  GNOME Wayland. Windows and macOS were not built or run here.
- The Playground is deliberately read-only. Paid and unknown-billing routes stay
  off until the user turns on the per-run consent switch, and the private runtime
  grants execution only in its dedicated playground workspace. Provider-native
  option selection is not exposed yet, though models show the supported options.
- The broker's `apps.list` does not report execution scope, so the Connected
  apps page says so rather than guessing.
- Admin credentials cannot reveal account identity in alpha.18; the app shows
  "identity hidden" instead of implying it can see it.
- Usage charts plot the share of each window **used**, not remaining
  percentage, and replace a flat line with the current figure when nothing
  changed in the range. Opening the app makes one bounded metadata probe per
  authenticated connection whose allowance is missing or stale, so usage
  returns after a broker restart without polling.
- Cursor is probed through the installed `cursor-agent` CLI: signed-in state,
  plan tier and the native model catalog. Included-usage percentages come from
  Cursor's dashboard service only when the broker is started with
  `--allow-usage cursor`; without that grant the app shows the local account
  cache and says the allowance is not read. The session token stays in memory
  for that one request and is never logged, serialized or exported.
- OpenCode 2 is read from its running local service (owner-only descriptor,
  loopback API): authentication is marked observed only when the service
  returns authenticated workspace limits, and its rolling/weekly/monthly
  allowance is shown. OpenCode 2 text execution is not wired, so its models are
  catalogued but not offered in the Playground.
- Detail rails (Providers, Models) keep a fixed comfortable width; only the
  sidebar is drag-resizable today. Model lists are search-filtered and capped,
  not virtualized.
- The command palette does not yet move its selection with arrow keys; Enter
  activates the best match. Menus/dialogs are inline rather than modal.
- Custom rows and navigation are pointer-accessible GPUI elements; they do not
  yet carry full accessibility roles, and there is no tray integration.
