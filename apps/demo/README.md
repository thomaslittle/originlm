# OriginLM · Developer showcase

The OriginLM React application that consumes only the public TypeScript SDK and the selector core. By default it pairs with the native helper and shows what is actually detected on your machine; clearly labeled example data is only a fallback when no runtime is paired. The app itself never reads a credential, keyring, or provider client; all of that stays in the native helper.

## Run it

From the repository root with Node 22.16+ (CI uses 22.20):

```sh
npm install --ignore-scripts
npm run dev          # native helper + UI on Vite hot reload -> http://127.0.0.1:5173/
```

It starts the **native helper**, so the demo shows what is actually detected on this machine (passive discovery, then metadata-only provider checks, no prompts). Choose **Show current pairing code**, then **Pair**; pairing is an explicit action. Imports never start a listener.

**Real data first, labeled example data as the fallback.** With a paired runtime everything on screen is real. If no native runtime is reachable, or you choose **Explore example data** on the pairing screen, the demo shows clearly labeled synthetic data ("Synthetic showcase" banner) with a **Pair a runtime** button back to the real thing. Example data is never mixed with real results, and a failed explicit connection clears it.

For tests and scripted scenarios only, `node apps/demo/server.mjs --fixture` serves the built UI with a loopback fixture service (`/#demo-fixture&staleAuth=1&...`).

## Brand

The shell uses the OriginLM treatment: the app icon (graphite symbol on the
amber `#EBA31A` tile) in the sidebar header, the graphite/amber/off-white palette
in both themes, and Red Hat Display (variable TTF, OFL 1.1) as the interface
face. Sources and licences: `src/assets/brand/README.md` and
`src/assets/fonts/README.md`.

## Stack

| Concern | Choice |
| --- | --- |
| UI | React 19 with the React Compiler (no manual `memo`/`useMemo` needed) |
| Build | Vite 8 (Rolldown), TypeScript 7, separate `index.html` / `assets/*` files |
| Styling | Tailwind CSS v4, shadcn/ui **base-mira** on Base UI, Phosphor icons, Red Hat Display |
| Routing | TanStack Router, real path routes (see below) |
| State | zustand vanilla store, components subscribe to slices |
| Charts | shadcn charts on Recharts, lazy-loaded |
| Quality gate | React Doctor, required on every pull request |

The bundle is emitted as separate files, never inlined, so the fixture server and the native helper can serve it under `script-src 'self'; style-src 'self'; font-src 'self'`.

### Routes

`/overview`, `/connections`, `/models`, `/usage`, `/selector-lab` (the landing page), `/playground`, `/diagnostics`. `/` redirects to `/selector-lab`. The fixture server and the native helper serve `index.html` for exactly these paths and nothing else; add a route in `src/lib/pages.ts`, `src/router.tsx`, `server.mjs` and `crates/originlm-transports/src/http/routes.rs` together.

### Architecture notes

- `src/state/store.ts` is a framework-free store. It owns the SDK client, the event stream, epoch bookkeeping, and every action. No component effect boots or tears down the client. Text deltas are coalesced to about one commit per frame and reduced by the pure `reduceEvents`.
- Components read state with `useDemo(selector)`; the resolved catalog is cached per (catalog, billing consent) so every consumer in a render shares one list. Search input is deferred with `useDeferredValue` so a 5,000-model catalog stays responsive.
- Pages other than the landing page, the palette, the connect dialog, and the chart chunks are lazy. Route chunks are warmed after first render.
- `src/components/ui/` is shadcn CLI output (`components.json`). Refresh it with `npx shadcn@latest add <name> --cwd apps/demo`; do not hand-copy components. The CLI detects pnpm from `pnpm-workspace.yaml`, so move that file aside for the command.
- The model picker (`src/components/model-picker.tsx`) is the provider-rail popover used by the Compact selector and the Playground composer. Provider marks are real brand icons (`src/assets/providers`, provenance in its README); they identify a provider and grant no trademark rights.

## What it shows

Overview, connections/account evidence, models (side-by-side comparison), per-pool usage with charts, selector lab, session playground, and safe diagnostics/integration snippets.

All eight selectors share the same selection state: vertical provider rail (the default), compact popover picker, keyboard command palette, capability-first filter, comparison table, allowance-aware groups, task presets, and mobile drill-down. Exact route/account identity is retained. Options are rendered from the provider's descriptors, including missing and future effort choices; sensitive options require a visible confirmation. Favorites distinguish separate routes to identical model names.

The Playground follows the [shadcn chatbot-template](https://github.com/shadcn-ui/chatbot-template) structure (see the reuse ledger in `THIRD_PARTY_NOTICES.md`): an empty state with suggestions, a message thread with a pending indicator, and a prompt composer with an inline model control. It exercises public SDK grants, selection validation, session creation, text events, approval denial/acceptance, and cancellation through the same permission gates as before — every turn still needs an explicit route, a consent checkbox, and a Run click. No inference occurs. Synthetic quota pools preserve shared identity and missing totals. A failed explicit live connection clears all synthetic data; live execution is disabled in this preview UI.

Large lists are bounded to 50 matching rows with search rather than full scrolling virtualization. Favorites are in-memory only; the theme choice is the only thing persisted (`localStorage`). Persistent recents, production pairing, live provider integration, and the remaining architecture tasks are still pending.

## Why some files are still `.mjs`

`server.mjs`, `fixture/*.mjs`, `lib/*.mjs`, and the `*.test.mjs` files are plain Node ESM on purpose: they run on the repository's Node 22.16 baseline with no build step and no type-stripping flag, and the same helpers are imported by the browser bundle and by dependency-free Node tests. Their public shapes are typed by the adjacent `.d.mts` files, and `tsc` checks every consumer.

## Tests

| Command | What it covers |
| --- | --- |
| `node --test apps/demo/tests/*.test.mjs` | Fixture service and real local HTTP/SSE, including the SPA route allowlist and font MIME. |
| `python scripts/test_demo.py` | 16 Playwright interactions against the real UI and public SDK, bundled through the same Vite pipeline (React Compiler included) into one offline script. Run `npm run build:demo` first for the stylesheet. Set `CHROMIUM_EXECUTABLE` to an approved Chromium when the default browser is unavailable. |
| `node --test scripts/test_accessibility.mjs` | Automated WCAG A/AA checks (axe) on every selector variant and page, plus connection-state assertions, against the fixture server. Scripted flags travel in the hash fragment (`#demo-fixture&staleAuth=1&extraProfiles=1&mode=live&probeResults=1&bulkProbeHook=1`); they never reach the server. |
| `npm run doctor` | React Doctor (offline: no telemetry, no score upload). Must report no warnings or errors. |

These are fixture-tested; the browser suites are not a claim of live-runtime qualification.
