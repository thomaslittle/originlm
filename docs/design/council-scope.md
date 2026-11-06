# UI council — test run scope

Date: 2026-11-06. Machine: DESKTOP-AKHJDM7 (Windows x64), the only capture
machine for this run. App: `originlm-desktop` (GPUI), example mode.

This is a **validation pass**, not a full polish run. Success is proving
that deterministic captures, blind scoring and one fix round work, and
measuring the delta — not reaching 9+.

## Screens in scope (3 most used)

| # | Page (title) | Code | Why it is in scope |
|---|---|---|---|
| 1 | Overview | `Page::Overview` / `pages/overview.rs` | First screen and the daily "what is left?" answer. |
| 2 | Usage history | `Page::Usage` / `pages/usage.rs` | The over-time view behind the same question. |
| 3 | Accounts | `Page::Providers` / `pages/providers.rs` | Where a person checks which account is verified and what it can do. |

All other pages are out of scope for this run, including Settings,
Diagnostics, Models, Developer, Apps & grants, Sessions and Playground.

## Allowed changes

- Spacing and rhythm inside the existing scale.
- Type size and weight inside the existing families and scale.
- Colour and contrast of existing tokens (no new palette).
- Hierarchy and density of existing content.
- Existing states: loading, empty, unknown, stale, error.
- Copy where it is wrong, clipped or misleading.

## Test-run limits

- **Max 2 scoring rounds:** Round 0 (baseline) and Round 1 (after fixes).
- **Fix only the 3 lowest-scoring categories**, across these 3 screens.
- **Conservative changes only:** no palette overhaul, no typeface change,
  no layout rewrite. Direction questions go in the final report for Tom;
  do not block on them.
- **No new features, no new dependencies, no unlisted screens.**
- Stop after Round 1 even if scores are below 9. Escalate mid-run only if
  a hard gate fails or reviewers split more than 2 points on a category
  (then, and only then, the Antigravity `claude-sonnet-5-5-medium`
  tie-breaker may be used).

## Capture protocol

- Window 1440x920, 100% scale, example mode (synthetic data, no accounts
  read), dark and light.
- Reduced motion and a pinned clock (`ORIGINLM_CAPTURE_TIME`) so repeated
  captures are byte-identical.
- Each screen/theme produces one full PNG plus native-scale header, main
  and footer crops. Go-model reviewers get the crops as well as the full
  shots.
- Harness: `pwsh scripts/council_capture.ps1 -Round <n>`; output under
  `docs/design/council/round-<n>/` with a SHA-256 `manifest.json`.

## Rubric (13 categories, 1-10 each)

Typography, Color and contrast, Spacing and rhythm, Alignment,
Iconography, Hierarchy and density, Consistency, Interaction states,
Craft and polish, Light/dark parity, Accessibility, Copy and voice,
Distinctiveness.

Every score needs a one-line reason and a cited element/location. `no
score` where a capture cannot show the thing (for example motion).

## Hard gates (never averaged away)

- WCAG contrast for text and meaningful non-text.
- A visible focus indicator on keyboard-reachable controls.
- Minimum hit targets for primary controls.

## Reviewers

Gate every round: `antigravity_2 / gemini-3.8-flash-high` (fallbacks:
`gemini-3.8-flash-medium`, then `opencode-go/minimax-m3`).
Panel: `opencode-go/kimi-k2.7-code` and `opencode-go/minimax-m3`.
Final, once, after Round 1: `codex / gpt-6.1-sol`.

Forbidden: the `claudeAgent` provider, Grok, DeepSeek as a screenshot
scorer, and any silent substitution.
