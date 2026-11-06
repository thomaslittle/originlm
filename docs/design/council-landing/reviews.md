# Landing page council run — record

Target: `apps/site`. Branch `ui-council/landing` off `origin/main` (`13ad3f1`).
Machine: DESKTOP-AKHJDM7 (Windows x64). Captures: headless Chrome 1248-series,
SwiftShader, 1440×900 and 390×844, device scale 1, reduced motion, animations and
transitions frozen. Full page plus one image per section, tall sections split into
≤1400px chunks.

## Determinism

- Round 0 (pre-change): two captures of the same config, 56/56 PNGs byte-identical.
- Final build: two captures of the same config, 52/52 PNGs byte-identical.
- Byte compare was SHA-256 per file. The static (reduced-motion / no-WebGL) layout is used
  because the live WebGL scene is not byte-stable between runs; the static layout is the
  page's real reduced-motion and no-JavaScript presentation and contains every section.

## Calibration

Image: `docs/design/council-landing/calibration.png` (1440×920, copied from
`ui-council/test-run`). Planted flaws: header control collision, overprinted faint
“Local cache only” line, stray overflowing text at the right edge.

| Model | Attempt | Result | Flaws named with location | What it missed |
|---|---|---|---|---|
| claudeAgent / claude-haiku-5-5 (low) | 1 | pass | 3/3: “Updated now” vs Refresh/search; “Local cache only” overprint; “OpenCode linked account allowance rema…” at right edge | — |
| claudeAgent / claude-haiku-5-5 (low) | 2 | pass | 3/3, same locations | — |
| codex / gpt-6-luna | 1 | pass | 2/3: header collision, right-edge strip | Did not name the faint overprinted cache line as a separate defect |
| codex / gpt-6-luna | 2 | pass | 2/3: header collision, right-edge strip | Same omission |
| codex / gpt-6.1-terra (medium) | 1 | pass | 3/3, all planted flaws | — |
| codex / gpt-6.1-terra (medium) | 2 | pass | 3/3, all planted flaws | — |

Reserve `claudeAgent / claude-sonnet-5-5` was not called: Terra passed twice. No model
failed calibration and no seat was substituted.

## Scores

Each cell is 1–10. `no score` means the model did not give one (Luna omitted Craft in
Round 0 and Round 1; interaction and light/dark were not scoreable from stills). Averages
use the scores that exist. The three lowest Round 0 categories were Accessibility 5.00,
Craft and polish 5.50, Iconography 6.00; those were the revision targets, with the contrast
hard gate also fixed.

### Round 0 (baseline)

| Category | Haiku | Luna | Terra | Avg |
|---|---|---|---|---|
| Typography | 7 | 8 | 7 | 7.33 |
| Color and contrast | 6 | 7 | 6 | 6.33 |
| Spacing and rhythm | 6 | 7 | 7 | 6.67 |
| Alignment | 6 | 8 | 7 | 7.00 |
| Iconography | 5 | 7 | 6 | 6.00 |
| Hierarchy and density | 7 | 7 | 7 | 7.00 |
| Consistency | 6 | 8 | 8 | 7.33 |
| Interaction states | no score | no score | no score | — |
| Craft and polish | 5 | no score | 6 | 5.50 |
| Light/dark parity | no score | no score | no score | — |
| Accessibility | 5 | 5 | 5 | 5.00 |
| Copy and voice | 8 | 8 | 8 | 8.00 |
| Distinctiveness | 8 | 8 | 8 | 8.00 |

### Round 3 (final)

| Category | Haiku | Luna | Terra | Avg | Δ vs R0 |
|---|---|---|---|---|---|
| Typography | 8 | 8 | 7 | 7.67 | +0.34 |
| Color and contrast | 8 | 8 | 7 | 7.67 | +1.34 |
| Spacing and rhythm | 7 | 8 | 7 | 7.33 | +0.66 |
| Alignment | 8 | 9 | 8 | 8.33 | +1.33 |
| Iconography | 6 | 8 | 7 | 7.00 | +1.00 |
| Hierarchy and density | 7 | 8 | 7 | 7.33 | +0.33 |
| Consistency | 8 | 9 | 8 | 8.33 | +1.00 |
| Interaction states | no score | no score | no score | — | — |
| Craft and polish | 7 | 8 | 7 | 7.33 | +1.83 |
| Light/dark parity | no score | no score | no score | — | — |
| Accessibility | 6 | 7 | 6 | 6.33 | +1.33 |
| Copy and voice | 8 | 8 | 8 | 8.00 | 0 |
| Distinctiveness | 8 | 8 | 8 | 8.00 | 0 |

Intermediate averages (R1, R2) are in the PR body. The panel did not move on Accessibility
after Round 1 (6.00), so Round 2 and 3 targeted the mobile matrix, the usage screenshot,
code wrapping, the close-CTA contrast, and the security rule.

### Sol final verdict

Single call, packet-only, no repo browsing, 10-minute timeout, medium effort. Verdict:
**improved**. “The current page resolves conspicuous mobile clipping in the grants, support
matrix, SDK and install sections, brings the hero’s quick-start action into the first mobile
view, and corrects the oversized GitHub icon.” Single most important remaining problem:
mobile reading load (the expanded support matrix, long code samples and spaced footer).

| Category | Sol |
|---|---|
| Typography | 8 |
| Color and contrast | 7 |
| Spacing and rhythm | 6 |
| Alignment | 8 |
| Iconography | 8 |
| Hierarchy and density | 6 |
| Consistency | 8 |
| Interaction states | no score |
| Craft and polish | 8 |
| Light/dark parity | no score |
| Accessibility | 7 |
| Copy and voice | 8 |
| Distinctiveness | 9 |

## Hard gates

| Gate | Result | Evidence |
|---|---|---|
| WCAG text/non-text contrast | pass | `node scripts/check_contrast.mjs` — 47/47 pairs. All text pairs ≥4.5:1; focus ring 11.2:1; non-text marks ≥3:1. Reviewers’ Round 0 “fail” was an estimate from a still; the measured pairs pass. |
| Visible keyboard focus | pass | 14-stop keyboard smoke at 1440; every stop shows the 2px `--color-focus` ring at 3px offset; selected tabs and primary buttons also show the browser’s contrasting inner ring. |
| Touch targets | pass | `scripts/test_site.mjs`: every standalone control ≥44px at 320–960, ≥32px above; at 390 every standalone control measured ≥44. Haiku’s Round 3 estimate that the SDK tabs and Copy pill were ~30px was not borne out by measurement (Copy right edge 364 < 373; tabs 44px). |
| Horizontal overflow | pass | `scripts/test_site.mjs` over 320, 375, 414, 768, 1024, 1440, 1920; `documentElement.scrollWidth === 390` at 390. The only inner scroller is the provider table at ≥60rem; below 60rem it becomes cards. |
| No truncated headlines/CTAs | pass | Headlines wrap; code wraps with a hanging indent; the install command and Copy fit at 390. |
| Site build | pass | `npm run build` (tsc, Vite, prerender). |
| Rendered site tests | pass | `node --test scripts/test_site.mjs` — 9/9. |

## Performance

Measured with `apps/site/scripts/measure_perf.mjs` against the production preview, headless
Chrome, SwiftShader, 1440×900, 4× CPU throttle. The machine was under load from the
concurrent desktop council run, so the documented idle baseline in `landing-audit.md`
(LCP 0.33s / TBT 81ms) is not comparable. The base commit (`13ad3f1`) and this branch were
measured back-to-back under the same load:

| Build | LCP (4×) | TBT (4×) | CLS | Idle fps | Scrub fps |
|---|---|---|---|---|---|
| `13ad3f1` base | 1200 / 1936 ms | 1270 / 1249 ms | 0 | 25.6–26.8 | 20.7–21.0 |
| final branch | 1308 / 1276 ms | 890 / 1120 ms | 0 | 28.2–29.1 | 19.9–22.8 |

First-load JS 95.1 → 95.9 KB gzip (+0.8); CSS 10.0 → 10.8 KB gzip (+0.8); fonts unchanged.
No material regression: CLS 0, TBT lower in both runs, LCP within run-to-run noise, frame
rate comparable or better. The page height fell from 19,777 to 17,056px at 1440 and from
24,461 to 24,906px at 390 (the mobile increase is the 4-column provider cards replacing a
horizontally clipped table).

## Models and rounds actually used

- Calibration: Haiku ×2, Luna ×2, Terra ×2. All passed; Terra passed twice, so the reserve
  (Sonnet) did not run.
- Round 0: Haiku, Luna, Terra.
- Revision rounds 1, 2, 3: Haiku, Luna, Terra each time.
- Final: Sol ×1.
- No timeouts, no errored delegates, no substitutions. Luna omitted Craft in R0 and R1; those
  are recorded as `no score`, not invented. No Opus, no Gemini/Antigravity, no Kimi, no
  MiniMax, no image-generation model, and DeepSeek did not score.

## What changed, by section

- **Hero:** removed the burned-in scene labels from the stills; the live node labels now wait
  until the story starts, so they no longer print through the headline; added a left scrim
  so the display type sits on a darker field. Mobile headline now sets in three lines with
  the command and both buttons above the fold; hero height 1024 → 844 at 390.
- **Story:** rebuilt the static/reduced-motion story as four chapters (2×2) with only their
  own evidence: one snapshot table, one route result, one host approval, one event stream.
  Removed the four repeated tables and the dead left columns. The live pinned story is
  unchanged. Static story height 4,840 → ~3,100 at 1440.
- **How it fits:** removed the tiny still-as-background and the empty band; the statement,
  request-path label, five-node flow and control-center callout are now one readable
  composition. Adds a clear “on one machine” boundary.
- **SDK:** fixed the mobile grid overflow (code/tabs were 625px wide in a 356px column);
  code wraps with a hanging indent; `$ npm` spacing corrected.
- **Grants:** widened the manifest column on desktop so the token path no longer wraps;
  mobile permission rows become labelled cards; code wraps.
- **Sessions / integrations:** mobile code fits and wraps; no clipped comments.
- **Support matrix:** desktop headers now align over centred marks, marks 17px, “Limits”
  gets a +/− affordance, legend moved above the table, rows tightened. Mobile becomes a
  four-column capability grid per provider instead of a horizontally clipped table.
- **Control center / usage:** readable meter list comes before the full-window screenshot;
  added “Percent used · reset countdown”; desktop reset labels no longer wrap; the full
  screenshot is a link to the full-size image and the caption says so.
- **Security:** list starts under its rule (no orphan rule).
- **Install:** mobile tabs wrap, command and Copy fit, footnote no longer clipped.
- **Close/footer:** fixed `.footer-brand svg` sizing the GitHub icon at 150px; mobile CTA
  gets a scrim so the headline does not sit on the bright ring; eye is positioned for phone.
- **Contrast:** removed the 0.38 opacity on ineligible rows, raised past-code and unshown-event
  opacity, brightened scene labels, regenerated stills without low-contrast burned-in labels.

## For Tom

- **Headline scope:** Luna suggested “Ship AI features on your users’ own subscriptions” may
  read as subscription-only, while the page also covers local Ollama and metered API routes.
  I kept it because it is the site’s existing claim and the lead immediately widens the
  scope. If you want the broader claim, “Build on the AI your users already have” is the
  README’s own line and would fit the same slot. This is a direction call, not a bug.
- **Story stills:** the four chapter illustrations are variations on the same amber eye.
  They show the progression, but Sol called the repetition part of the mobile reading load.
  Replacing them with one wide scene or with no stills is a taste call for you.
- **Support matrix length:** it is 18 providers × 8 operations. The desktop table and the
  mobile cards are both honest and complete; shortening it means cutting evidence, which I
  did not do.
- **Advice overruled:** Haiku’s Round 3 “targets fail” and the Round 0 contrast “fail” were
  visual estimates from stills. The measured checks pass and are cited above. I did not
  shrink the type or hide content to chase those estimates.

## Later (out of scope)

- A real mobile crop of the control-center account cards (the app screenshot truncates its
  own card text, so a crop would not add readable data).
- A sticky in-page section index for the 17k page.
- A shorter mobile footer with two columns.
- Restoring a light theme is a direction decision, not parity work.
- A live-motion determinism harness (the WebGL scene is not byte-stable; current captures are
  the static fallback).
