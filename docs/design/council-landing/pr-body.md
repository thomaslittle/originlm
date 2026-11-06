**Draft — not for merge.** A three-round blind UI-council review of `apps/site`, rebuilt rather than polished. The branch is based on `origin/main` (`13ad3f1`); nothing is deployed or published. The only output is this PR.

## Summary

The landing page had structural visual problems, not just surface ones: the reduced-motion story repeated the same table four times, the "no server" section was a tiny diagram with a dead band, the mobile SDK/install code overflowed the viewport, the support matrix was a horizontally clipped table, the footer GitHub icon was rendered at 150px, and the ineligible story row was faded to ~2.4:1. The rebuild restores the page's editorial voice and makes the mobile page readable.

The blind panel's final verdict (Sol): **improved**. Its single most important remaining problem is mobile reading load: the support matrix, long code samples and footer are still a long evaluation journey.

## Calibration

The calibration image is `docs/design/council-landing/calibration.png` (1440×920), copied from `ui-council/test-run`. The three planted flaws were: the header control collision, the overprinted faint "Local cache only" line, and the stray overflowing text at the right edge. Both attempts per model are listed.

| Seat | Model | Attempt | Result | Flaws named with location | What it missed |
|---|---|---|---|---|---|
| Gate A | claudeAgent / claude-haiku-5-5 (low) | 1 | pass | 3/3: "Updated now" vs Refresh/search; "Local cache only" overprint; "OpenCode linked account allowance rema…" at the right edge | — |
| Gate A | claudeAgent / claude-haiku-5-5 (low) | 2 | pass | 3/3, same locations | — |
| Gate B | codex / gpt-6-luna | 1 | pass | 2/3: header collision; right-edge strip | Did not name the faint overprinted "Local cache only" line as a defect |
| Gate B | codex / gpt-6-luna | 2 | pass | 2/3: same two | Same omission |
| Panel | codex / gpt-6.1-terra (medium) | 1 | pass | 3/3: collision, overprint, right-edge strip | — |
| Panel | codex / gpt-6.1-terra (medium) | 2 | pass | 3/3, same | — |

Terra passed twice, so the reserve (`claudeAgent / claude-sonnet-5-5`) did not run. No seat failed or was substituted.

## What changed, by section

- **Hero.** The scene stills were regenerated without burned-in labels, and the live node labels now wait until the story begins, so they no longer print through "your users' own". A left scrim puts the display type on a darker field. On a 390px phone the headline sets in three lines, not five, and the install command plus both buttons are in the first view; hero height 1024 → 844.
- **Story.** The static/reduced-motion story is now four chapters with only their own evidence: one snapshot table (Discover), one chosen route (Select), one host approval (Grant), one event stream and reply (Run). The four repeated tables and the dead left columns are gone; 4,840 → ~3,100px at 1440. The live pinned story is untouched.
- **How it fits.** Removed the tiny still-as-background and the empty band. The statement, request-path label, five-node flow, machine boundary and control-center callout are one readable composition, with a mobile layout that no longer depends on a 260px diagram.
- **SDK.** Fixed a real mobile overflow: the code and tabs were 625px wide in a 356px column. Code now wraps with a hanging indent; `$ npm` has its space back.
- **Grants.** The desktop manifest column is wider, so the `tokenFile` path no longer wraps. On mobile the three-column permission table becomes labelled cards; code wraps.
- **Sessions / integrations.** Mobile code fits and wraps; comments are no longer clipped.
- **Support matrix.** Desktop headers align over centred marks, marks are 17px, "Limits" gets a +/− affordance, the legend moves above the table, and rows are tighter. On mobile the table becomes a four-column capability grid per provider — all eight operations visible, no sideways clipping.
- **Control center / usage.** The readable meter list comes before the full-window screenshot; a "Percent used · reset countdown" legend removes the used/left ambiguity; the screenshot is a link to the full-size image. Desktop reset labels no longer wrap.
- **Security.** The list starts under its rule, removing the orphaned rule.
- **Install.** Mobile tabs wrap, the command and Copy fit, and the alpha footnote is no longer clipped.
- **Close / footer.** Fixed `.footer-brand svg`, which was sizing the GitHub icon at 150px. The mobile CTA gets a scrim so the headline does not sit on the bright ring.
- **Contrast.** Removed the 0.38-opacity ineligible row, raised past-code and unshown-event opacity, brightened scene labels, and regenerated the stills without low-contrast labels.

## Before / after captures

Every image is pinned to this branch. Full page: [before, 1440](https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-full.png) · [after, 1440](https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-full.png) · [before, 390](https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-full.png) · [after, 390](https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-full.png).

### Desktop, 1440×900

| Before | After |
|---|---|
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-01-hero.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-01-hero.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-02-story-p1.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-02-story-p1.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-03-how.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-03-how.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-05-grants.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-05-grants.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-08-providers-p1.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-08-providers-p1.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-09-usage-p1.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-09-usage-p1.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-11-install.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-11-install.png" width="640"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/desktop-12-close.png" width="640"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/desktop-12-close.png" width="640"> |

### Mobile, 390×844

| Before | After |
|---|---|
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-01-hero.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-01-hero.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-02-story-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-02-story-p1.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-03-how-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-03-how-p1.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-04-sdk-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-04-sdk-p1.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-05-grants-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-05-grants-p1.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-08-providers-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-08-providers-p1.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-09-usage-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-09-usage-p1.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-11-install.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-11-install.png" width="260"> |
| <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-0/mobile-12-close-p1.png" width="260"> | <img src="https://raw.githubusercontent.com/thomaslittle/originlm/ui-council/landing/docs/design/council-landing/round-3/mobile-12-close-p1.png" width="260"> |

## Council review

Models actually used: Haiku ×2 and Luna ×2 and Terra ×2 in calibration; Haiku, Luna and Terra in Round 0 and in each of the three revision rounds; Sol once for the final verdict. No timeouts, no errors, no substitutions. Luna omitted Craft and polish in Round 0 and Round 1; that is recorded as `no score`, not invented. No Opus, no Gemini/Antigravity, no Kimi, no MiniMax, no image-generation model; DeepSeek implemented and did not score.

### Round 0 — per reviewer

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

### Round 3 — per reviewer

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

The three lowest Round 0 categories were Accessibility (5.00), Craft and polish (5.50) and Iconography (6.00); those were the revision targets. The contrast hard gate was also fixed. Intermediate averages: Round 1 — typography 7.33, colour 7.33, spacing 6.67, alignment 7.00, iconography 7.00, hierarchy 7.00, consistency 7.67, craft 6.50, accessibility 6.00; Round 2 — 7.33, 7.00, 6.67, 7.67, 6.67, 6.67, 7.67, 7.00, 6.00. The panel plateaued on Accessibility after Round 1, which is why Rounds 2–3 targeted the mobile matrix, the usage screenshot, code wrapping and the close-CTA contrast.

### Sol — final verdict

One call, packet-only, no repo browsing, 10-minute timeout, medium effort: **improved**. Scores: typography 8, colour 7, spacing 6, alignment 8, iconography 8, hierarchy 6, consistency 8, craft 8, accessibility 7, copy 8, distinctiveness 9. Single most important remaining problem: mobile reading load.

### Hard gates

| Gate | Result | Evidence |
|---|---|---|
| WCAG contrast | pass | `node scripts/check_contrast.mjs` — 47/47 pairs; all text ≥4.5:1, focus ring 11.2:1, non-text marks ≥3:1. The reviewers' Round 0 "fail" was an estimate from a still. |
| Visible focus | pass | 14-stop keyboard smoke at 1440; every stop shows the 2px `--color-focus` ring at 3px offset. |
| Touch targets | pass | `scripts/test_site.mjs`: every standalone control ≥44px below 960, ≥32px above. At 390 the SDK tabs and Copy are 44px; Copy's right edge is 364 < 373. Haiku's Round 3 estimate that they were ~30px was not borne out by measurement. |
| Horizontal overflow | pass | `scripts/test_site.mjs` at 320, 375, 414, 768, 1024, 1440, 1920; `documentElement.scrollWidth === 390` at 390. |
| No truncated headlines/CTAs | pass | Headlines wrap; code wraps with a hanging indent; install command and Copy fit at 390. |
| Site build | pass | `npm run build` (tsc, Vite, prerender). |
| Rendered tests | pass | `node --test scripts/test_site.mjs` — 9/9. |
| Capture determinism | pass | Two captures of the same config, 52/52 PNGs byte-identical (SHA-256). Round 0 was 56/56 before the change. |

## Performance

`apps/site/scripts/measure_perf.mjs`, production preview, headless Chrome, SwiftShader, 1440×900, 4× CPU. The machine was under load from the concurrent desktop council run, so the idle baseline in `landing-audit.md` (LCP 0.33s / TBT 81ms) is not comparable. The base commit and this branch were measured back-to-back under the same load:

| Build | LCP (4×) | TBT (4×) | CLS | Idle fps | Scrub fps |
|---|---|---|---|---|---|
| `13ad3f1` base | 1200 / 1936 ms | 1270 / 1249 ms | 0 | 25.6–26.8 | 20.7–21.0 |
| final branch | 1308 / 1276 ms | 890 / 1120 ms | 0 | 28.2–29.1 | 19.9–22.8 |

First-load JS 95.1 → 95.9 KB gzip (+0.8); CSS 10.0 → 10.8 KB gzip (+0.8); fonts unchanged. No material regression: CLS 0, TBT lower in both runs, LCP within run-to-run noise. Page height 19,777 → 17,056px at 1440; 24,461 → 24,906px at 390 (the mobile rise is the provider cards replacing a clipped table).

## For Tom

- **Headline scope.** Luna suggested "Ship AI features on your users' own subscriptions" may read as subscription-only, while the page also covers local Ollama and metered API routes. I kept it because it is the site's existing claim and the lead widens the scope immediately. If you want the broader claim, the README's "Build on the AI your users already have" fits the same slot. Direction call, not a bug.
- **Story stills.** The four chapter illustrations are variations on the same amber eye. They show the progression, but Sol called the repetition part of the mobile reading load. One wide scene or no stills is a taste call.
- **Support matrix length.** 18 providers × 8 operations. The desktop table and the mobile cards are both complete; shortening it means cutting evidence, which I did not do.
- **Advice overruled.** Haiku's Round 3 "targets fail" and the Round 0 contrast "fail" were visual estimates from stills. The measured checks pass and are cited above; I did not shrink type or hide content to chase them.

## Later (out of scope)

- A mobile crop of the control-center account cards (the app screenshot truncates its own card text, so a crop would not add readable data).
- A sticky in-page section index for the 17k page.
- A shorter mobile footer with two columns.
- A light theme; that is a direction decision, not parity work.
- A live-motion determinism harness (the WebGL scene is not byte-stable; these captures are the static fallback).

## Reproduce

```sh
cd apps/site && npm ci --ignore-scripts && npm run build
npm run preview -- --port 5178
CHROME_PATH=/path/to/chrome node scripts/capture_council.mjs http://127.0.0.1:5178/ ../../docs/design/council-landing/round-3
node scripts/check_contrast.mjs
cd ../.. && ORIGINLM_SITE_URL=http://127.0.0.1:5178/ node --test scripts/test_site.mjs
```

The review record and per-round notes are in `docs/design/council-landing/reviews.md`.
