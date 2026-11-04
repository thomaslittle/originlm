# Desktop control center: design audit

Scope: `crates/originlm-desktop` (GPUI via `gpui-kit 0.9`), run as
`cargo run -p originlm-desktop -- --example` on Windows 11 at 1440×920.
Screenshots are real captures of example mode (synthetic data, no accounts
read). The "before" set is in [`screens/desktop/before/`](screens/desktop/before/);
the "after" set is in [`screens/desktop/after/`](screens/desktop/after/).

Date: 2026-11-04. Two passes on branch `fix/opencode-wrapper-execution`: the
first rebuilt the shell, Overview and Developer; the second ported the other
eight pages, added keyboard access, states and contrast fixes, and verified
every page in both themes and three window sizes. The findings below are the
original audit; their status follows.

## Summary

The app is functionally careful. Its honesty rules are good: `data.rs` keeps
unknown, unavailable and stale apart, and pages say "reset not reported"
instead of guessing. The design problems sit above that layer:

1. **No design system.** `ui.rs` has a handful of helpers, but every page
   hand-builds its own cards with literal `px(10.)`, `px(11.5)`, `px(12.5)`,
   `px(14.)` values, and four different corner radii. There is no spacing
   scale, no type scale, and no shared panel or meter component, so each page
   drifts.
2. **The overview answers the wrong question first.** The first screen is a
   metrics strip ("Providers 6, Verified 6/8, Models 16, Broker clients 0"),
   then the three most constrained pools, then a full-width cost bar chart of
   example turns. The question users open a usage tool for (how much is left
   on each account, and when does it reset) is answered for at most three
   windows, with no per-account grouping. Five of eight connections have no
   visible usage state at all; they appear only in a sentence at the bottom.
3. **Developer features are buried and flat.** Connected apps, Sessions and
   Playground sit under "Operate"; the API guide is under "Support" as "How to
   use". Nothing on the overview tells a developer the runtime state, how many
   routes are runnable, or where to start.
4. **Windows shell bug.** The title bar shows two sets of minimize, maximize
   and close buttons (one from `gpui-kit`'s `TitleBar`, one drawn by the app
   because Windows reports server decorations).
5. **Small, low-contrast type.** Most secondary text is 10–11px muted on
   graphite. At 100% scaling on a 1440 window that is below comfortable reading
   size; on the Diagnostics and Usage pages most of the content is 10px.

## Findings by area (ranked inside each area)

### Navigation and information architecture

| # | Finding | Severity |
|---|---|---|
| N1 | Ten pages in three groups: *Monitor* (Overview, Providers & accounts, Models, Usage), *Operate* (Connected apps, Sessions, Playground), *Support* (How to use, Diagnostics, Settings). "Operate" is the developer surface but is not named as one; the API guide is filed under "Support". | major |
| N2 | Usage is the fourth item under Monitor, after Providers and Models, although it is the main reason a non-developer opens the app. | major |
| N3 | The selected nav item carries three signals at once: a filled background, an amber icon and an amber dot on the right. One is enough. | minor |
| N4 | The window title bar repeats the page title that the page header shows 40px below. | minor |
| N5 | The page header holds four controls (age chip, Reload, theme toggle, Command). Reload and the age chip say the same thing; the status bar says it a third time ("refreshed 4s ago"). | minor |
| N6 | The status bar's "Today" button opens the mini view. The label does not describe the action. | minor |

### Page layouts and hierarchy

| # | Finding | Severity |
|---|---|---|
| L1 | Overview: no per-account view. Allowance cards show at most three pools; the "Next reset" card repeats one of them. Accounts without readings are reduced to "No allowance reported for Grok and OpenCode and Local runtime." | critical |
| L2 | Overview: the cost trend (example estimates) takes the largest area on the first screen. It is a secondary signal and the bars are unlabeled. | major |
| L3 | Overview: "Usage over time" duplicates the Usage page. | minor |
| L4 | Usage: the series picker is a row of tab pills whose labels are full window names ("Antigravity Count without a published total"), so they read as buttons of unequal weight. The reset schedule lists one row in a large card. | major |
| L5 | Providers & accounts: each connection is a 100px-tall card with a 52px tile and three chips; eight connections need a scroll. A table or compact list fits the data. | major |
| L6 | Models: rows are 70px tall, each repeats provider and account under the name. The favourite star is a 12px target. | minor |
| L7 | Connected apps and Diagnostics are mostly empty space in example mode, with the explanatory text in a small banner. | minor |
| L8 | How to use: install commands are stale (`curl -fsSL https://<site>/install.sh`, `npm install originlm` without the alpha tag) and say "Unix socket" while the app runs on Windows. | major |
| L9 | Settings: the first card is a six-field price editor, before Appearance and Data source. Advanced tools should come last. | minor |

### Components and consistency

| # | Finding | Severity |
|---|---|---|
| C1 | Card, panel and banner styles are re-implemented on every page (`rounded(px(ui::CARD_RADIUS))`, `rounded(px(9.))`, `rounded(px(8.))`, `rounded(px(7.))`, `rounded(px(6.))`, `rounded(px(5.))`). | major |
| C2 | Three progress styles: `ui::bar` (5px pill), the overview's coloured bar, and a separate "used" bar list. None is segmented or labeled for screen readers. | major |
| C3 | Chip colours mean different things on different pages (amber is "default model", "subscription", "cache-only, stale" and "warning"). | major |
| C4 | Section titles use 10–11px semibold muted text with no rule or spacing standard. | minor |

### Typography

| # | Finding | Severity |
|---|---|---|
| T1 | The mono family is `"Adwaita Mono"`, which does not exist on Windows or macOS, so numbers fall back to an arbitrary system font. | major |
| T2 | No type scale: sizes used include 10, 10.5, 11, 11.5, 12, 12.5, 13, 18, 19, 20, 26 and 34px. | major |
| T3 | Body copy defaults to 10–11.5px for secondary text. | major |

### Colour and theming

| # | Finding | Severity |
|---|---|---|
| K1 | Brand tokens are generated (`brand_tokens.rs`), but `theme.rs` adds about 60 literal hex values next to them. Surface steps (background, card, raised) are not exposed to pages, so pages use `theme.colors.list` as a card colour. | major |
| K2 | Success green carries "fresh", "authenticated" and the 75% bar. Amber carries brand, warning and "default". Meaning should come from state, not decoration. | minor |

### States (empty, loading, error, unknown)

| # | Finding | Severity |
|---|---|---|
| S1 | Unknown readings show "Unknown" in the mini view but are hidden on the overview. Unknown should look like a first-class state wherever a meter would appear. | major |
| S2 | Diagnostics shows "Unknown, 0 open, checked not checked", a generated sentence that reads as broken. | minor |
| S3 | Empty states are well written (Sessions, Usage) but centred in large cards with no next step except Sessions. | minor |

### Accessibility and keyboard

| # | Finding | Severity |
|---|---|---|
| A1 | Shortcuts exist (Ctrl+K, Ctrl+R, Ctrl+T, Ctrl+B, Ctrl+Shift+M) and the palette works. Nav items are mouse targets only; there is no focus ring on sidebar items or cards. | major |
| A2 | Hit targets: rail items 34px, nav rows about 28px, favourite star about 12px, "Check usage" text links about 14px. | minor |
| A3 | Status is often colour-only (green or red dots in connection lists and the status bar). | minor |

### Windows behaviour

| # | Finding | Severity |
|---|---|---|
| W1 | Duplicate caption buttons (see summary). | critical |
| W2 | Mono font fallback (T1). | major |
| W3 | Guide copy assumes a Unix socket. | minor |

### Performance

No measurable problem in example mode: the UI polls every 250ms and only
re-renders on change. `app.rs` is 2,035 lines and owns all state; that limits
future work more than it costs frames.

## Status of every finding

Second pass, 2026-11-04. "Done" means implemented and checked in a capture or
test listed under Verification.

| # | Status | How |
|---|---|---|
| N1 | Done | Groups are Usage, Inventory, Build, System; the API guide became the Developer page under Build. |
| N2 | Done | Usage is the first group; Overview answers "what is left". |
| N3 | Done | One selection signal: a raised row with an amber icon. The trailing dot is gone. |
| N4 | Done | The title bar says "OriginLM"; the page title appears once. |
| N5 | Done | One refresh control that states the age ("Updated 4s", "Last read 7m" when stale, "Not connected" when offline). |
| N6 | Done | "Mini view", focusable, with an accessible name naming its shortcut. |
| L1 | Done | One card per account with every window it reports; unknown and "not checked" are their own rows. |
| L2 | Done | Cost trend sits below the account grid, with a real empty state instead of a "max $0.0000" figure. |
| L3 | Done | "Usage over time" removed from Overview. |
| L4 | Done | Usage history: a list of windows with meters; range and Chart/Table are segmented tabs. |
| L5 | Done | Accounts: one compact table (44px rows) plus an inspector. |
| L6 | Done | Models: 44px rows; the star is a 32px labelled button with a tooltip. |
| L7 | Done | Apps & grants and Diagnostics have real empty and error states and fill the page with useful structure (how access is granted; health summary). |
| L8 | Done | Install commands verified against npm, crates.io and PyPI on 2026-11-04; no Unix-socket wording. |
| L9 | Done | Settings order: Appearance, Data source, Refresh, Layout, Local model prices (marked Advanced), About. |
| C1 | Done | Every page composes `ds::panel`, `panel_head`, `panel_row`, `table_head`/`th`, `state`, `banner`. No page sets a radius or a font size literal. |
| C2 | Done | One meter (`ds::meter`, 20 cells) everywhere; `ui::bar` deleted. |
| C3 | Done | Chips are neutral unless they carry state: green verified, amber cached/stale/billing-sensitive, red blocked. "Default" and billing are neutral. |
| C4 | Done | `ds::section_header` and `panel_head` replace the 10–11px muted titles. |
| T1 | Done | Mono family resolved per platform at startup (see D below). |
| T2 | Done | Five sizes: 11, 12, 13, 15, 22px (`ds::ty`). A grep for `text_size(px(<number>` outside `ds.rs` finds nothing. |
| T3 | Done | Secondary text is 12px; 11px is used only for captions, chips and table headings. |
| K1 | Done | Pages use `ds::surface` steps; light status colours and a light amber-for-text moved into `brand/tokens.json` (regenerated). |
| K2 | Done | Green means verified/fresh only; the 75% meter is amber (brand), low is amber then red. |
| S1 | Done | Unknown draws hollow cells plus "Unknown", "Not reported" or "Not checked yet" with the reason. |
| S2 | Done | Diagnostics says "Not checked yet. The runtime has not run a health check" or "Degraded · 11 open issues · last checked Nov 04, 06:28". |
| S3 | Done | Every empty state names what was looked for and the next step, left-aligned in its panel, with an action where one exists. |
| A1 | Done | Tab/Shift+Tab traversal, focus ring on every interactive element, Enter/Space activation (verified by sending keys, see Verification). |
| A2 | Done | Rail 36px, nav rows 34px, list rows 44px, links and tabs 28px minimum, icon buttons 32px. |
| A3 | Done | Every dot is paired with text (`ds::status`); state chips carry words. |
| W1 | Done | One set of caption buttons on Windows (visible in every capture). |
| W2 | Done | See T1. |
| W3 | Done | See L8. |

## Ranked punch list

1. Fix the duplicate Windows caption buttons. **Done.**
2. Design-system layer with spacing, type and radius scales and shared components. **Done** (`src/ds.rs`; second pass added focus ring, page container, state blocks, banners, tabs, selectable rows, tables, copyable values, icon buttons).
3. Overview as one card per account. **Done.**
4. Navigation regrouped. **Done.**
5. Developer page. **Done**, then extended (see E).
6. Platform mono family. **Done.**
7. Raise the type floor on every page. **Done** for all ten pages, the shell, palette, mini view and toast.
8. Keyboard focus ring and traversal of the sidebar. **Done**: every sidebar item, rail item and control is a tab stop with a ring.
9. Cost trend below the grid; no "Usage over time" on Overview. **Done.**
10. Accounts as a compact table; Models rows at 44px. **Done.**
11. Usage page: a list with meters instead of tab pills. **Done.**
12. Settings order. **Done.**
13. Split `app.rs` state into per-page view models. **Deferred.** It is a refactor with no user-visible effect; it touches every page's data access and the broker message handling at once, so it belongs in its own change with its own tests rather than inside a design pass. `app.rs` grew by about 200 lines (data gate, stale handling, focus) and is still the single owner of state.

## Checklist A–G (second pass)

**A. Port every page.** Done. Accounts, Models, Usage history, Apps & grants,
Sessions, Playground, Diagnostics and Settings were rewritten on `ds.rs`, plus
the cost and price panels. Every page uses the same header (title, example
chip, subtitle, refresh, command field), `ds::page()` container (28px gutter,
24px between sections), panel anatomy (head row with title and caption or link,
hairline rows, 16px inset) and the five-size type scale. States:

| State | Where | Copy (example) |
|---|---|---|
| Loading | Overview, Usage history, Accounts, Models, Apps & grants, Diagnostics (shared data gate) | "Connecting to the runtime. Looking for this user's shared broker, or starting a private runtime…" |
| Offline (error, nothing cached) | same pages | "The runtime is not answering. {error} No snapshot has been read in this session…" + Retry now |
| Stale (error after a good read) | every page, shell banner | "Stale: showing the last snapshot, read 7m ago. The runtime stopped answering (…). Nothing below is live." + Retry now |
| Empty | every page | e.g. "No accounts discovered yet", "No allowance windows reported… That is not the same as zero usage", "No connected apps in example mode", "No sessions yet", "No runnable model", "No cost evidence yet" |
| Error (per feature) | Apps & grants, Playground, prices | "Listing apps needs the administrator credential", "The turn did not run", "Prices unavailable" |

Stale is real, not cosmetic: a failed read used to arrive as an empty snapshot
and blank every page. `Desktop::drain` now keeps the last good snapshot when a
read fails after one succeeded, sets `stale`, and keeps the true read time.
Unused `ui.rs` components (`section_title`, `eyebrow`, `mono`, `numeric`,
`chip`, `empty_state`, `divider`, `bar`, `tab_pill`, `PAGE_PAD`, `CARD_RADIUS`,
`Tone::Blue`) and two unused `ds` items were deleted; clippy runs with
`-D warnings` and reports no dead code.

**B. Accessibility and keyboard.** Done. `ds::Focusable::focus_ring` gives an
element a tab stop and a 2px ring that shows only for keyboard focus
(`focus_visible`). The root handles Tab/Shift+Tab with `focus_next`/`focus_prev`;
GPUI fires `on_click` for Enter and Space on a focused element. Roles and
labels: sidebar and rail items are tabs, list rows are list items, links are
links, copy values and icon buttons are buttons with "Copy {value}" or action
names; icon-only controls (rail, star, mini-view close) also have tooltips.
Escape closes the palette and the mini view. Contrast: every pair below passes
WCAG AA in both themes after three changes (light amber text `#8F5D00`, light
status colours, meter fill on `chart_1`), and `theme::tests::theme_pairs_meet_wcag`
fails the build if a token regresses.

**C. Themes and window sizes.** Done. Every page captured in dark and light, the
mini view in both, states in dark (stale also in light), and Overview,
Accounts, Models, Usage history, Developer and Playground at the 1000×660
minimum in both themes, plus Overview and Developer at 2560×1440. Responsive
rules: the account grid is 3/2/1 columns; inspectors stack below the list under
1000px of content width; Developer pairs stack under 1100px and the permission
steps go 4→2 columns under 900px. One defect found and fixed at the minimum
height: the sidebar status block overlapped the last nav items; the nav list now
scrolls and the header and footer blocks do not shrink.

**D. Windows behaviour.** Done for what can be checked here. One set of caption
buttons (gpui-kit draws them on Windows; the app draws its own only on
server-decorated Linux). Fonts: Red Hat Display is bundled; the mono family is
the first installed of Cascadia Mono, Consolas (Windows), SF Mono, Menlo
(macOS), JetBrains Mono, DejaVu Sans Mono, Adwaita Mono (Linux), else
`monospace` (`theme::mono_family`); captures show Consolas-class glyphs on this
machine. No flash: the theme is installed before the window opens and again from
saved preferences inside `Desktop::new`, before the first frame. DPI: captures
are at 100% scaling; the layout uses logical pixels throughout, but 125–200%
scaling was not exercised because changing the system scale was out of scope.

**E. Developer page and Overview.** Done, using only data the runtime reports.
Added to Developer: supported capabilities per route (union of the route's
models' `Supported` capabilities), sign-in state per route, a "Recent turns"
panel from `costs.read` (model, observed time, input/output tokens, cost,
basis Reported/Estimated/Unknown, labelled "example data" in example mode), an
"Apps with access" panel from `apps.list`, install commands per SDK, and a
link to the shared-broker guide. Diagnostics gained "Copy redacted report"
(the same redacted JSON as the file export, via
`originlm_control::diagnostic_document`), and the saved path is shown
absolute. Overview: probe actions are honest in example mode ("Example mode
probes nothing"), unknown sign-in reads "Not checked yet. No allowance has been
read." instead of claiming the client reports nothing, and an empty account
list has its own state. Not added because the data model does not support it:
grant approval (no approval surface exists; see decision log), per-turn cost
for turns started by other apps (the runtime does not report them to this
window), and grant scopes beyond what `apps.list` returns.

**F. Verification.** See below.

**G. Live mode.** Exercised. Running without `--example` on this machine
attached to a runtime with administrator access (16 accounts, 12 providers, 315
models, 16 routes, 11 open issues). Overview, Developer, Apps & grants and
Diagnostics rendered real data: Claude Code and Codex windows with resets,
"Not checked" accounts, the app's own `ipc-host` administrator client,
"Degraded · 11 open issues". Startup made the app's normal bounded metadata
probes; nothing was executed. These captures show real accounts and were kept
out of the repository. Not exercised live: revoking an app (only the app's own
administrator client was connected), saving prices, a Playground turn (it would
spend the user's allowance), the stale banner against a runtime that really
stops (it is exercised through `LAI_DESKTOP_EXAMPLE_STATE=stale`, which drives
the same code path), and screen readers.

## Contrast

Computed from `theme.rs` and `brand_tokens.rs` (WCAG 2.x relative luminance).
Chip text is measured on its 8% tinted chip background over the panel. Text
needs 4.5:1; focus ring, dots and meter cells need 3:1.

**Dark**
| Pair | Foreground | Background | Ratio | Needs | |
|---|---|---|---|---|---|
| foreground on page | #faf9f5 | #1e1a1b | 16.36 | 4.5 | pass |
| muted text on page | #b0a8a1 | #1e1a1b | 7.35 | 4.5 | pass |
| foreground on panel | #faf9f5 | #262122 | 15.07 | 4.5 | pass |
| muted text on panel | #b0a8a1 | #262122 | 6.77 | 4.5 | pass |
| foreground on sidebar | #faf9f5 | #231f20 | 15.47 | 4.5 | pass |
| muted text on sidebar | #b0a8a1 | #231f20 | 6.96 | 4.5 | pass |
| foreground on row selected | #faf9f5 | #3A3233 | 11.84 | 4.5 | pass |
| muted text on row selected | #b0a8a1 | #3A3233 | 5.32 | 4.5 | pass |
| link/primary text on panel | #eba31a | #262122 | 7.40 | 4.5 | pass |
| link/primary text on page | #eba31a | #1e1a1b | 8.03 | 4.5 | pass |
| success chip text on its chip | #7cc08f | #2d2e2b | 6.37 | 4.5 | pass |
| success meter/dot vs panel (non-text) | #7cc08f | #262122 | 7.40 | 3.0 | pass |
| warning chip text on its chip | #d9a441 | #342b24 | 6.15 | 4.5 | pass |
| warning meter/dot vs panel (non-text) | #d9a441 | #262122 | 7.06 | 3.0 | pass |
| danger chip text on its chip | #e0776b | #352828 | 4.73 | 4.5 | pass |
| danger meter/dot vs panel (non-text) | #e0776b | #262122 | 5.31 | 3.0 | pass |
| primary chip text on its chip | #eba31a | #362b21 | 6.42 | 4.5 | pass |
| primary meter/dot vs panel (non-text) | #eba31a | #262122 | 7.40 | 3.0 | pass |
| muted chip text on panel | #b0a8a1 | #262122 | 6.77 | 4.5 | pass |
| focus ring vs page (non-text) | #eba31a | #1e1a1b | 8.03 | 3.0 | pass |
| focus ring vs panel (non-text) | #eba31a | #262122 | 7.40 | 3.0 | pass |
| focus ring vs sidebar (non-text) | #eba31a | #231f20 | 7.60 | 3.0 | pass |
| primary button text | #231f20 | #eba31a | 7.60 | 4.5 | pass |
| danger button text | #231f20 | #e0776b | 5.45 | 4.5 | pass |
| selected tab text | #faf9f5 | #3b3334 | 11.66 | 4.5 | pass |
| status bar caption | #b0a8a1 | #231f20 | 6.96 | 4.5 | pass |
| meter fill (chart_1) vs empty cell | #eba31a | #373233 | 5.87 | 3.0 | pass |

**Light**
| Pair | Foreground | Background | Ratio | Needs | |
|---|---|---|---|---|---|
| foreground on page | #231f20 | #faf9f5 | 15.47 | 4.5 | pass |
| muted text on page | #6b635c | #faf9f5 | 5.59 | 4.5 | pass |
| foreground on panel | #231f20 | #FFFFFF | 16.30 | 4.5 | pass |
| muted text on panel | #6b635c | #FFFFFF | 5.89 | 4.5 | pass |
| foreground on sidebar | #231f20 | #F3F0E9 | 14.32 | 4.5 | pass |
| muted text on sidebar | #6b635c | #F3F0E9 | 5.18 | 4.5 | pass |
| foreground on row selected | #231f20 | #F0E5D2 | 13.07 | 4.5 | pass |
| muted text on row selected | #6b635c | #F0E5D2 | 4.73 | 4.5 | pass |
| link/primary text on panel | #8f5d00 | #FFFFFF | 5.62 | 4.5 | pass |
| link/primary text on page | #8f5d00 | #faf9f5 | 5.34 | 4.5 | pass |
| success chip text on its chip | #4c6f58 | #f1f3f2 | 5.06 | 4.5 | pass |
| success meter/dot vs panel (non-text) | #4c6f58 | #FFFFFF | 5.64 | 3.0 | pass |
| warning chip text on its chip | #876012 | #f5f2ec | 5.06 | 4.5 | pass |
| warning meter/dot vs panel (non-text) | #876012 | #FFFFFF | 5.66 | 3.0 | pass |
| danger chip text on its chip | #a64b42 | #f8f1f0 | 5.08 | 4.5 | pass |
| danger meter/dot vs panel (non-text) | #a64b42 | #FFFFFF | 5.67 | 3.0 | pass |
| primary chip text on its chip | #8f5d00 | #f6f2eb | 5.04 | 4.5 | pass |
| primary meter/dot vs panel (non-text) | #8f5d00 | #FFFFFF | 5.62 | 3.0 | pass |
| muted chip text on panel | #6b635c | #FFFFFF | 5.89 | 4.5 | pass |
| focus ring vs page (non-text) | #8f5d00 | #faf9f5 | 5.34 | 3.0 | pass |
| focus ring vs panel (non-text) | #8f5d00 | #FFFFFF | 5.62 | 3.0 | pass |
| focus ring vs sidebar (non-text) | #8f5d00 | #F3F0E9 | 4.94 | 3.0 | pass |
| primary button text | #231f20 | #eba31a | 7.60 | 4.5 | pass |
| danger button text | #FFF8F6 | #a64b42 | 5.40 | 4.5 | pass |
| selected tab text | #231f20 | #EDE5D6 | 13.02 | 4.5 | pass |
| status bar caption | #6b635c | #F3F0E9 | 5.18 | 4.5 | pass |
| meter fill (chart_1) vs empty cell | #B47400 | #ededed | 3.30 | 3.0 | pass |

All 54 pairs pass.

## Before and after

Before: the original app (`screens/desktop/before/`). After: this pass
(`screens/desktop/after/`), example mode, 1440×920 unless noted.

| Page | Before | After, dark | After, light | Minimum size (1000×660) |
|---|---|---|---|---|
| Overview | ![](screens/desktop/before/overview.webp) | ![](screens/desktop/after/overview-dark.webp) | ![](screens/desktop/after/overview-light.webp) | ![](screens/desktop/after/overview-dark-min.webp) ![](screens/desktop/after/overview-light-min.webp) |
| Usage history | ![](screens/desktop/before/usage.webp) | ![](screens/desktop/after/usagehistory-dark.webp) | ![](screens/desktop/after/usagehistory-light.webp) | ![](screens/desktop/after/usagehistory-dark-min.webp) ![](screens/desktop/after/usagehistory-light-min.webp) |
| Accounts | ![](screens/desktop/before/providersaccounts.webp) | ![](screens/desktop/after/accounts-dark.webp) | ![](screens/desktop/after/accounts-light.webp) | ![](screens/desktop/after/accounts-dark-min.webp) ![](screens/desktop/after/accounts-light-min.webp) |
| Models | ![](screens/desktop/before/models.webp) | ![](screens/desktop/after/models-dark.webp) | ![](screens/desktop/after/models-light.webp) | ![](screens/desktop/after/models-dark-min.webp) ![](screens/desktop/after/models-light-min.webp) |
| Developer (was How to use) | ![](screens/desktop/before/howtouse.webp) | ![](screens/desktop/after/developer-dark.webp) | ![](screens/desktop/after/developer-light.webp) | ![](screens/desktop/after/developer-dark-min.webp) ![](screens/desktop/after/developer-light-min.webp) |
| Apps & grants (was Connected apps) | ![](screens/desktop/before/connectedapps.webp) | ![](screens/desktop/after/appsgrants-dark.webp) | ![](screens/desktop/after/appsgrants-light.webp) | |
| Sessions | ![](screens/desktop/before/sessions.webp) | ![](screens/desktop/after/sessions-dark.webp) | ![](screens/desktop/after/sessions-light.webp) | |
| Playground | ![](screens/desktop/before/playground.webp) | ![](screens/desktop/after/playground-dark.webp) | ![](screens/desktop/after/playground-light.webp) | ![](screens/desktop/after/playground-dark-min.webp) ![](screens/desktop/after/playground-light-min.webp) |
| Diagnostics | ![](screens/desktop/before/diagnostics.webp) | ![](screens/desktop/after/diagnostics-dark.webp) | ![](screens/desktop/after/diagnostics-light.webp) | |
| Settings | ![](screens/desktop/before/settings.webp) | ![](screens/desktop/after/settings-dark.webp) | ![](screens/desktop/after/settings-light.webp) | |
| Mini view | ![](screens/desktop/before/mini.webp) | ![](screens/desktop/after/mini-dark.webp) | ![](screens/desktop/after/mini-light.webp) | |

States (Overview, `LAI_DESKTOP_EXAMPLE_STATE`): loading
![](screens/desktop/after/overview-dark-loading.webp), offline
![](screens/desktop/after/overview-dark-offline.webp), stale
![](screens/desktop/after/overview-dark-stale.webp) and
![](screens/desktop/after/overview-light-stale.webp), empty
![](screens/desktop/after/overview-dark-empty.webp). Wide (2560×1440):
![](screens/desktop/after/overview-dark-wide.webp)
![](screens/desktop/after/developer-dark-wide.webp).

## Decision log

- **Squarer corners, hairlines.** 4px controls, 6px panels, matching the landing page.
- **Segmented meters, percent left.** The same 20-cell meter as the site; the meter fills with what remains, and the usage history chart now plots percent left too (it plotted percent used), so every surface uses one measure.
- **No invented values.** Accounts with no pool render "Not reported" or "Not checked yet"; unknown readings render an explicit unknown row with the snapshot's reason.
- **IA renames are persisted by title.** Old titles still resolve for `LAI_DESKTOP_PAGE` and saved preferences.
- 2026-11-04 **Keyboard focus is `focus_visible`.** A ring on every mouse click would be noise; keyboard users get it instantly, pointer users do not.
- 2026-11-04 **Stale keeps the last snapshot.** Showing old data under a dated banner is more useful and more honest than an empty page; the header switches to "Last read N" and nothing is labelled live.
- 2026-11-04 **States are previewable in example mode.** `LAI_DESKTOP_EXAMPLE_STATE` drives the real loading/offline/stale/empty paths with synthetic data, so they can be reviewed and captured without breaking a broker.
- 2026-11-04 **No approval prompt in this window (default decision 1).** The app has a real revoke surface but no approval surface; grants are approved through a grants file or `Broker::authorize`. Apps & grants explains exactly that, and the site's hero keeps its approval card labelled as the host app's UI.
- 2026-11-04 **Light theme amber.** Brand amber `#EBA31A` is 2.3:1 on paper, so light text and focus use `#8F5D00` (5.3:1) and meters use `#B47400` (3.3:1 against the empty cell). The brand amber stays on the primary button, where graphite text on it is 7.6:1.
- 2026-11-04 **Light status colours moved in the shared tokens.** `lightSuccess` `#4C6F58`, `lightWarning` `#876012`, `lightDanger` `#A64B42`, new `lightAmberText` `#8F5D00`, regenerated with `scripts/generate_brand_tokens.py` so the site and app stay in sync.
- 2026-11-04 **Inspectors stack instead of shrinking.** Under 1000px of content width the inspector moves below the list rather than squeezing both.
- 2026-11-04 **Deferred: per-page view models** (punch list 13), for the reason given above.

## Verification log

| Check | Result |
|---|---|
| `cargo build -p originlm-desktop` | OK |
| `cargo fmt -p originlm-desktop --check` | clean |
| `cargo clippy -p originlm-desktop --all-targets -- -D warnings` | clean |
| `cargo test -p originlm-desktop` | 9 passed (adds `theme_pairs_meet_wcag`) |
| `cargo check --workspace --all-targets --locked` | OK |
| `python scripts/verify.py --profile core` (workspace `cargo test`) | OK: 124 test binaries, 719 tests passed, 0 failed |
| `python scripts/generate_brand_tokens.py --check` | OK after regenerating |
| Captures | 41 images in `screens/desktop/after/`, all opened and inspected; defects found and fixed during review: chips stretching to column width, meters collapsing to zero height inside rows, header saying "Updated" while offline, Overview without an empty state, a fake "max $0.0000" with no records, sidebar overlap at 660px height, muddy light meters, "reports no allowance window" for unchecked accounts |
| Keyboard | Sent Tab/Enter to the running app (window activated by process id, captured with `PrintWindow`): 4th Tab focuses Accounts in the sidebar with a visible ring; Enter opens Accounts; 14 Tabs reach the first table row with the ring in dark, 15 the second row in light |
| Live mode | See G |

Captures use `PrintWindow` on the window handle of the process the script
started and discard a capture whose handle belongs to another process, so no
other application's pixels are recorded.

Not verified: macOS rendering; Linux after this pass; display scaling above
100%; screen-reader output; a live revoke, price save or Playground turn.
