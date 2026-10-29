# ADR-002: Dependency-first distribution

- **Status:** accepted; phased implementation
- **Date:** 2026-10-27
- **Related:** [ADR-001](ADR-001-shared-runtime.md), [browser extension](../integration/browser-extension.md), [pairing](../integration/pairing.md), [shared broker](../integration/shared-broker.md)

## Context

OriginLM today ships as a runtime, a CLI/broker, a desktop app and a
developer showcase. An application that wants local AI must find the helper,
install or bundle it, start it, and keep it current. That is a deployment
problem disguised as an integration problem: every consumer re-invents
discovery, startup, token handling and upgrades, and gets it slightly wrong.

The standalone app, the broker and the control center are correct for people who
want them. They must keep working exactly as they do when run on their own. The
change is at the *edge*: installing a dependency should be enough for an app to
use local AI, with no separate install and no service required.

Two constraints shape the design:

1. Provider work (discovery, probes, native execution) must stay out of the
   app's render loop, and a crashing provider client must not take the app down.
2. Safety rules do not change because distribution did: imports never scan or
   read credentials, discovery is staged, grants are explicit, secrets are never
   exported, and transports stay owner-scoped or loopback-only.

## Positioning

The product has two halves of equal weight, and every surface leads with both.

**For everyone who uses AI tools.** OriginLM is the local control center for
every AI client, model, account and subscription on the machine: Claude Code,
Codex, Cursor, Grok, Gemini, OpenCode, Copilot, Ollama, LM Studio and more. It
shows usage and quotas with reset times and pace, which models are actually
reachable through which route (subscription or metered), history and trends, and
the issues that need attention — a signed-out provider, a nearly full quota. It
is fully local, with no telemetry and no exported secrets.

**For developers.** One dependency puts the user's own AI in an app. The same
runtime the dashboard reads is the one applications plug into, so installing it
for the dashboard makes every app built on it work.

> Everyone is building one-off usage widgets. This is the only one that is also
> the developer platform underneath.

Marketing, the install page and the desktop app treat these as peers; neither
half is a footnote to the other. Claims stay factual: competitors are not named
unless a claim about them is verifiable.

## Decision

### D1. The dependency is the product boundary

Every supported ecosystem ships a package that contains (or resolves) the helper
binary for its platform:

| Ecosystem | Package | Shape |
| --- | --- | --- |
| Rust | `originlm` (facade over `originlm-kit`) | crate, links the runtime |
| npm | `originlm` + `originlm-cli-<platform>` | optional platform packages |
| Python | `originlm` wheel | per-platform wheels |

The standalone app, service and control center stay as they are. A dependency
never tries to install them, and never requires them to exist.

### D2. Resolution order: shared broker, then private helper

`LocalAi::start()` (Rust), `createLocalAI()` (npm) and `Client.auto()` (Python)
resolve in this order:

1. **Shared broker.** If a live broker is reachable through the owner-only
   descriptor/socket, connect and use it. This is the mode where several apps
   share one scanner, one cache and one grant table.
2. **Private helper.** Otherwise start a helper owned by this process. In Rust
   that is a bundled or developer-supplied binary via `LAI_HELPER_PATH`;
   in npm/Python it is the platform binary from the package. The helper speaks
   the same stdio JSON-RPC surface, is tied to the parent's lifetime, and never
   publishes a reusable token file.
3. **In-process runtime (Rust only).** If no helper binary exists at all, run
   `originlm-kit` on a dedicated background thread inside the app process. This keeps
   the promise that a missing standalone install is never an error, at the cost
   of sharing the app's process for broker scheduling and cache work.

The rule is: **prefer process isolation; degrade to in-process only when there is
no binary to isolate.** The mode is observable (`Mode::{Shared, Child,
InProcess}`) so an app can show it, log it, or refuse to run.

Why not in-process first? Provider clients are already child processes, but the
broker also owns SQLite, timers, backoff and notification delivery. A panic, a
blocking adapter or an OOM in that surface should not be able to kill the host
app. The stdio child costs one short-lived process and gives crash containment,
clean shutdown and a stable protocol boundary. In-process exists for the case
where the package cannot carry a binary (for example a `no-default-features`
build), not as the default.

A shared broker that exists but rejects the token is **not** silently replaced by
a private helper: that is a configuration error the app must surface. Only
"nothing is listening" falls through.

### D3. npm mirrors esbuild and Biome

`originlm` declares `optionalDependencies` on:

- `originlm-cli-linux-x64`
- `originlm-cli-linux-arm64`
- `originlm-cli-darwin-arm64`
- `originlm-cli-darwin-x64`
- `originlm-cli-win32-x64`

Musl/Alpine and Windows ARM64 packages are not prepared or implied by this
release; they require their own qualified runtime builds.

When the current platform has no packaged binary, the SDK fails with
`PLATFORM_UNSUPPORTED` and the install-page URL, never with a generic runtime
error. A caller-supplied transport or `LAI_HELPER_PATH` still works.

Each package contains exactly one binary and a small manifest. The SDK resolves
`require.resolve('originlm-cli-<platform>/package.json')`, verifies the
binary is executable, and spawns it with pipes. Installation is a normal
dependency update; nothing is downloaded at import time, and no postinstall
script runs. Works in Node, Electron's main process and Bun. Renderer processes
get the web bridge instead (Phase 5).

`LAI_HELPER_PATH` overrides resolution for kit development and for CI fixtures.
It is honoured before any packaged binary and is never read from the network.

### D4. Python wheels carry the binary

One wheel per platform (`manylinux`, `macosx`, `win_amd64`) includes the helper
under `originlm/_bin/`. `Client.auto()` uses the shared broker first, then
the bundled binary, then raises a single actionable error describing both
attempts. A source checkout may point at `LAI_HELPER_PATH`.

### D5. Publishing is a release-engineering task, not a side effect

Version-tag release workflow publishes crates, npm packages (including platform
packages), wheels and signed, checksummed GitHub bundles, with provenance
attestations. **Nothing is published until the maintainer has approved the
release and supplied the registry credentials.** The workflow is prepared and
dry-run first; a dry run uploads nothing and prints exactly what would be
published. Consumers get updates as ordinary dependency bumps; a Renovate or
Dependabot configuration is recommended per ecosystem rather than a custom
updater.

### D6. A one-line install exists for people who want the service

`curl -fsSL https://<site>/install.sh | sh` (macOS, Linux) and
`irm https://<site>/install.ps1 | iex` (Windows, plus a winget manifest) install
one binary into a user-level location and register a **user** service:

- launchd `LaunchAgent` on macOS,
- `systemd --user` unit on Linux,
- Task Scheduler logon task on Windows.

No admin rights, no system-wide paths. The service idles near zero CPU, restarts
on crash, starts on demand, and self-updates in the background with checksum and
signature verification. Each installer ships a matching uninstall command that
stops and removes the unit, its binary and its state descriptor.

### D7. Status stays out of the way

Four surfaces, one source of truth:

1. **SDK status:** `ai.status()` returns mode, providers and connected apps;
   sites render a small badge ("Local AI connected · Claude, Ollama" /
   "Not detected" with an install link).
2. **Tray/menu bar:** on by default, easy to disable; lists connected apps and
   models, Pause and Open Control Center. No popups.
3. **Notifications:** only for decisions the user must make (new app approval, a
   provider signed out).
4. **`originlm status`:** the same information in the terminal.

### D8. Plain websites use the extension first

The primary bridge for web apps is the existing browser extension plus Native
Messaging host: no port is opened, each origin needs a one-time approval stored
as a revocable grant, and the page receives results only, never credentials.

Loopback HTTP with pairing remains the fallback: strict origin allowlist, a
one-time pairing code, short-lived scoped tokens, and Chrome Private Network
Access handled explicitly. The web SDK detects the extension first, then
pairing, then shows the "Connect your local AI" flow.

### D9. Version compatibility is negotiated, not assumed

The SDK and the broker exchange `runtime.handshake` and compare protocol
versions. If a running shared broker is older or newer than this SDK can talk
to, the SDK closes that connection, falls back to its private bundled copy, and
records `FALLBACK_PROTOCOL_MISMATCH` with both versions in the status surface.
A version mismatch is never a fatal error and never steals the shared broker's
work. A shared broker that rejects the *grant* is still fatal (D2).

### D10. Host packaging gotchas

- **Electron:** the helper must be listed in `asarUnpack`; a binary inside an
  asar cannot execute. The SDK documents the exact glob and fails with a
  readable error when it detects an asar path without an unpacked sibling.
- **Tauri:** both routes are documented — link the crate (preferred: in-process
  or child through the facade) or ship the helper as an `externalBin` sidecar.
  The sidecar route must use the target-triple suffix Tauri expects.
- **Flatpak, Snap and the Mac App Store:** these sandboxes cannot start host CLIs
  or read provider metadata outside the sandbox. The client detects the sandbox
  (env markers, bundle paths, missing host socket) and reports an explicit
  `sandboxed` status, never "no providers found". It does not attempt to escape
  the sandbox.

### D11. Signing and provenance

| Platform | Artifact signing | Secrets needed (never purchased here) |
| --- | --- | --- |
| macOS | Developer ID Application signing + notarization for the helper in npm/wheel/bundle packages and for the installer | `APPLE_CERTIFICATE_P12`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID` |
| Windows | Authenticode signing of the helper and installer | `WINDOWS_CERTIFICATE_PFX`, `WINDOWS_CERTIFICATE_PASSWORD`, timestamp server URL |
| Linux | SHA-256 checksums plus minisign or cosign signatures | `MINISIGN_SECRET_KEY` (or `COSIGN_PRIVATE_KEY`) and its password |
| npm | `--provenance` with GitHub OIDC | `NPM_TOKEN` only for the first publish/2FA fallback |
| PyPI | Trusted Publishing (OIDC) preferred, token fallback | `PYPI_API_TOKEN` only if OIDC is unavailable |
| crates.io | `cargo publish` with a scoped token | `CARGO_REGISTRY_TOKEN` |

Gatekeeper and SmartScreen block unsigned binaries, so an unsigned release is a
dry-run artifact, not a published one. This ADR lists the certificates and
secrets; it does not buy or generate them.

### D12. Several private copies at once

Without a shared broker, each application may run its own private helper. That
is intentional isolation, not a bug: each copy carries its own grant file keyed
by app identity, and no copy can see another's accounts or grants. Duplicate
discovery work is bounded by the fact that only the first launch of a given app
probes; the rest read its state. The install page explains that installing the
background service converges these copies under one control center, with one
grant table and one set of provider probes.

### D13. Clean uninstall

The uninstall command removes, in order: the user service (launchd agent,
systemd user unit or Task Scheduler task), tray autostart, Native Messaging
manifests for every browser it installed, and the helper binaries. User data
(state database, history, grants) is kept by default and removed only with an
explicit `--purge` (Windows: `-Purge`). Uninstall is idempotent, never requires
admin rights, and never touches another user's files.

### D14. Privacy statement

The install page and the README carry the same statement as SECURITY.md: no
telemetry or hosted relay; history and grants stay local, while explicitly
authorized operations may contact the selected provider. Provider credentials
are never returned to consuming apps;
grants are per app and revocable; the SDK reports the same facts in-app. The
statement is generated from one source so they cannot drift.

### D15. Size budget

The helper ships inside every consumer's installer, so its size is a product
constraint:

- release builds use `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`,
  `panic = "abort"` where the platform allows it;
- the release workflow records the stripped binary size per target and fails the
  dry run if a platform exceeds the agreed budget (initial target: 40 MB per
  desktop target, 60 MB for musl static);
- the measured sizes are reported in the release notes and on the install page.

### D16. Tests for resolution

Phase 1 needs end-to-end coverage of each resolution path, not only unit tests:

1. shared broker present and accepting the grant;
2. shared broker absent, bundled helper starts;
3. shared broker present but protocol-incompatible, fallback recorded;
4. `LAI_HELPER_PATH` overrides the packaged binary and a bad override fails;
5. sandboxed environment reports `sandboxed`, not "no providers".

These run headless against fixture helpers. They must never take over the
developer's mouse, keyboard, window focus or an interactive session; no test may
raise a real window.

### D17. Consumers migrate in their own repositories

crntly-desktop, remotif and xhw_haiwire are migrated by their own maintainers in
their own repos, using the published crate and auto mode. This repository
prepares a short migration note per app; it does not edit sibling repositories.

## Phases

| Phase | Deliverable | Exit criteria |
| --- | --- | --- |
| 1 | Rust facade, npm platform packages, Python wheels, `LAI_HELPER_PATH`, `Mode` | A fixture helper proves shared → child → in-process resolution; packaged binary resolution is tested without downloading anything |
| 2 | Release workflow, provenance, dry run, dependency-bump guidance | Dry run lists every artifact and uploads nothing; credentials documented |
| 3 | Two-track install page (docs site and native app) | **Get the app**: one-line installer and desktop download. **Add to your project**: npm, pnpm, bun, cargo, pip tabs. Both tracks are present without scrolling past the other |
| 4 | Install/uninstall scripts, user services, tray, notifications, `status` | Install, enable, crash-restart, self-update verification and uninstall are exercised on each platform or explicitly marked unqualified |
| 5 | Extension bridge, pairing fallback, web SDK detection | Origin approval is revocable; no open port on the extension path; PNA behaviour documented |
| 6 | Migration notes for crntly, remotif, haiware | Each note names the file to replace and the auto-mode call |
| 7 | Landing page and product video | Lighthouse 90+ performance/accessibility, reduced-motion honored, hero fallback, video renders committed or linked |

Phase 3 is one page with two tracks, not a developer page with a download
footnote. **Get the app** leads with the one-line installer (curl/PowerShell),
the desktop download and what the dashboard shows; **Add to your project** leads
with the npm, pnpm, bun, cargo and pip tabs and the shortest working snippet for
each. The same page exists in the docs site and inside the native app's Help
view, and both tracks use the OriginLM brand and screenshots of the real app.

Phase 7 is the public home. It is a new `apps/site` (Vite + React +
TypeScript, static output, separate from `apps/demo`) with a premium dark
surface, crisp type, generous spacing and subtle motion, the OriginLM logo and
styles throughout, and a Lighthouse target of 90+ for performance and
accessibility. It leads with both halves:

- **Hero.** Headline and subhead cover the control center *and* the dependency.
  Copy-ready install tabs for npm, pnpm, bun, yarn, cargo, pip and the
  curl/PowerShell service install sit beside a desktop screenshot.
- **Two equal paths.** Directly under the 3D hero: "See your AI" (screenshots or
  GIFs of the OriginLM desktop and control center showing usage, quotas, models
  and issues) and "Build with it" (install tabs and code). Equal visual weight;
  no path is a footer link to the other.
- **Comparison.** A factual table of "single-provider usage widgets" versus
  this product: all providers, local by default, plus a developer platform.
  No competitor is named unless a claim about it is verifiable.
- **How it works.** Scroll-driven "discover → grant → session" steps with short
  code snippets.
- **Integrations.** Tauri, Electron, GPUI, Node, Python and plain websites
  (extension or pairing), with the real minimal code for each.
- **Providers.** Generated from `docs/adapters/SUPPORT-MATRIX.md`, honest about
  per-operation support.
- **Security and privacy.** No secrets exported, loopback only, per-app
  revocable grants, no telemetry; link the threat model.
- **Status and tray mock**, the product video, docs/GitHub/changelog links and a
  footer.

The hero is a procedural React Three Fiber scene (with drei and postprocessing,
light bloom and depth of field): a single stacked "device" that comes apart into
labeled glass layers — **the app and a usage/analytics dashboard side by side at
the top**, then SDK, transport, runtime, adapters and provider nodes — with light
packets traveling down and back up. The dashboard layer makes the shared runtime
visible: the same layer feeds both the screen and the dependency. Hovering a
layer explains it.
Reduced motion and missing WebGL get a static poster. The 3D bundle stays under
about 300 KB gzipped plus models, and the hero text paints before it loads.

The product video lives in `media/video` as an fframes project (Rust + SVG, GPU
render): 45–60 s at 1080p and 4K split roughly 50/50 — first "know your AI usage
across everything" (dashboard, quotas, models, issues), then "and build on it"
(the dependency and the same exploded-layer story) — plus a 6–10 s seamless loop
for the hero fallback and 16:9/1:1/9:16 social cuts. Export
MP4 (H.264), WebM and a poster; commit the source and put oversized renders in
releases or LFS. Music or voiceover must be licensed or omitted. Nothing is
posted publicly, no domain or paid hosting is bought, and previews use free
tiers only with the maintainer's approval.

Phases ship as separate branches and pull requests, each merged only after CI is
green. Phase 2 never publishes on its own; publication is a maintainer action.

## Product polish checklist

The desktop app and control center are a headline feature, not a demo of the
SDK. These are release gates for the dashboard half:

1. **Clean first-run.** A new install opens on a readable state — no token file
   hunting, no empty panes — and says what it found and what still needs a grant.
2. **Usage at a glance.** The default view answers "what can I use right now and
   what is nearly gone" without scrolling: per-provider remaining share, reset
   time and pace.
3. **Per-provider quotas and resets.** Each account shows its provider-reported
   windows and reset times, with unknown values marked unknown, never zero.
4. **Models with routes.** Every model shows its native id, route, and whether
   that route is subscription or metered; unsupported operations are stated.
5. **Optional tray mini-view.** Off unless the user turns it on: today's usage
   at a glance, with Pause and Open Control Center. No popups.

Gaps found while checking these go on this list rather than into a vague roadmap.
The desktop already covers the overview, quotas, routes and issues; the tray
mini-view is new and stays opt-in.

## Consequences

- Applications stop shipping their own client, token plumbing and startup
  policy; they declare a dependency and read a mode.
- The broker gains a second supported lifetime (private stdio child) that
  behaves like the existing parent-owned mode, with no reusable descriptor.
- Two binaries exist on disk in some installs (the app's copy and the service's).
  They are independent by design: the shared broker is only used when it is
  already running, never installed on demand.
- In-process mode is the weakest isolation and is documented as such; it is the
  fallback that makes "no separate install" true, not the recommended mode.
- Publishing becomes a gated, auditable release step with provenance, not a
  developer convenience.

## Open questions

- Whether the shared broker address should be a well-known socket name
  (`originlm-user`) or the existing descriptor path; the facade will accept
  both, preferring the descriptor when it exists.
- Whether npm platform packages ship a single binary or a signed archive; a bare
  binary keeps esbuild-style resolution simple, a signed archive would need a
  verification step at runtime.
- Which service manager owns self-update: the helper or the tray process. The
  current lean is the helper, with the tray only reporting.
