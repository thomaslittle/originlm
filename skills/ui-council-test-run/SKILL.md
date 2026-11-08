---
name: ui-council-test-run
description: TEST RUN variant of ui-council for Tom's exact model quotas and the OriginLM GPUI desktop app on his Windows PC. Use only for this first validation pass. The generic ui-council skill is unchanged and remains the default for real runs.
---

# UI Council — Test Run (OriginLM, Tom's setup)

Purpose of THIS skill is different from the original: not "reach 9+",
but validate the pipeline end-to-end and measure whether it works at all.
Success = deterministic screenshots work, the assigned models can actually
see and score them, scores are stable enough to act on, and one fix round
moves the lowest categories. Report honestly if any of that fails.

OriginLM is unannounced. Never name it publicly, post about this run,
or put this experiment in any public description.

## Verified setup (checked 2026-11-06 via orchestrator_capabilities)
- Environment: DESKTOP-AKHJDM7 (Windows x64) — the canonical and ONLY
  capture machine for this run. Do not mix in Linux/WSL captures.
- Project: `originlm` — desktop app is `cargo run -p originlm-desktop`.
- Main thread model pattern for this project: provider `opencode`,
  model `opencode-go/deepseek-v4.1-flash`, runtime full-access.

## Model assignments — v3 (2026-11-06, after pass 2)
Run 1 evidence behind the v2 lineup: Gemini 3.8 Flash was an unstable
gate (passed calibration once, then read a PNG as binary, truncated, and
timed out 30+ min); MiniMax M3's Round 1 scores contradicted its own
prose; Kimi K2.7 Code was the most reliable scorer. Tom's economy rule:
the two near-free vision models (Haiku, Luna) do the frequent work; the
two flagships (Sol, Opus) get only very small single runs at brief
occasions.
V3 CHANGE (Tom, 2026-11-06): Kimi is REMOVED. In pass 2 it scored 0
delta on nearly every affected category while both gates and Sol moved
+1 — it was passing calibration but not discriminating on real screens.
The panel seat was VACANT pending a model bake-off (single test calls
to 9 reviewer candidates, T3 thread
`mcp:4e33f3ab-b918-4c5c-8293-0a8ae0df5226`).
APPOINTED (Tom, 2026-11-06, from the bake-off results): Panel =
codex / gpt-6.1-terra (bake-off: 3/3 planted flaws, 0 false positives,
23s — fastest clean run, purpose-built for visual reasoning, $1/$5).
Reserve panel = claudeAgent / claude-sonnet-5-5, under the reserve
rule in the panel section below. The bake-off also produced a
ground-truth correction: the committed `calibration.png` is an
Overview capture whose planted flaws are the header control collision,
the overprinted faint "Local cache only" line, and the stray
overflowing text at the right edge — not a settings dialog.

Exact IDs — resolve first, do not guess.

Implementer / main thread (unchanged):
  - opencode / opencode-go/deepseek-v4.1-flash
    (abundant OpenCode Go usage; implements from structured text feedback,
     it is NOT a screenshot judge)

PROVIDER ROUTING IS FIXED (Tom, 2026-11-06 — do not reroute):
  - Claude account (`claudeAgent`): Haiku and Opus, plus Sonnet ONLY
    as reserve panel. Nothing else.
  - Codex account (`codex`): Luna, Sol, and Terra (panel). Nothing
    else.
  - OpenCode (`opencode`): DeepSeek ONLY. No other OpenCode model may
    be used in any seat. (Kimi removed 2026-11-06 — see v3 note.)
  - Gemini / Antigravity: dropped entirely. No seat, no fallback,
    no tie-break. (Run 1 instability + Tom's call.)
  - Other bake-off candidates (Astra, Fable, Grok, Muse Wonder, Qwen,
    GLM) were TESTED via single calls only and hold NO council seat.
    Do not use them in a scoring run.
  - Image-generation models in ANY role — including any render-target
    or direction-generation step — are out entirely (Tom, 2026-11-06).
    The council reviews real captures only.

CHEAP SEATS — run freely, every round (delegate_task, role: review):
  Gate A: claudeAgent / claude-haiku-5-5, low effort
    (Tom's Claude account — NOT OpenCode. Haiku is the baseline weight
     on that account, so a small single review packet is negligible
     even on a low remaining pool. No fallback route: if this delegate
     fails, log it and run the round on Gate B + panel alone.)
  Gate B: codex / gpt-6-luna
    (Tom's Codex account — NOT OpenCode. No fallback route, same rule.)
  Two cheap gates from two families cross-check each other every round.
  If Gate A and Gate B split >2 points on a category, that category is
  flagged — see the flagship occasions below.

Panel reviewer (blind, independent, role: review):
  - codex / gpt-6.1-terra, medium reasoning effort (appointed by Tom
    2026-11-06 from bake-off results; route: Tom's Codex account —
    NOT OpenCode). Terra must pass calibration TWICE consecutively
    before its first scoring round, same standard as the gates; its
    bake-off call was evidence, not a calibration pass.
  Reserve panel: claudeAgent / claude-sonnet-5-5. The reserve steps in
  ONLY if Terra fails its double calibration, or errors/times out in
  a round. If called, Sonnet must pass calibration twice itself
  before scoring. Otherwise Sonnet does not run — it is not a fourth
  reviewer.
  A contested category (gates + panel split >2) goes to a flagship
  tie-break (occasion 2 below), not to the reserve.
  MiniMax M3 stays out: its run-1 scores contradicted its own prose.

FLAGSHIP SEATS — Sol and Opus, rationed to brief occasions ONLY.
  - codex / gpt-6.1-sol
  - claudeAgent / claude-opus-5-5
  The ONLY permitted occasions (one flagship per occasion, never both
  by default):
    1. Final verdict — ONE of them, once, on the final round vs Round 0.
       Default: Sol (it did this well in run 1: +2/+1/+1, "improved").
    2. Tie-break — the cheap gates + panel split >2 points on a category
       that decides a pass/fail, or a hard-gate result is disputed.
    3. Pre-merge taste check — the OTHER flagship gets one look at the
       final PR screenshots before Tom merges. Once per PR, not per round.
  Every flagship run obeys the brief-run contract:
    - Single delegate call, packet = brief + shots/crops for the screens
      in scope + Round 0 shots. Nothing else.
    - NO repo browsing, NO file exploration, NO tool use beyond reading
      the packet. One structured score block (§3) and out.
    - Effort medium at most. Hard timeout 10 minutes: on timeout, record
      `no score`, do not retry, and the run moves on without it.
    - Opus occasions: while Claude Max is at ~3% before the Saturday
      reset, Opus does not run at all except a tie-break Tom approved.
      After the reset, its occasion is the pre-merge taste check.

FORBIDDEN in this run:
  - Gemini / Antigravity in ANY seat, fallback, or tie-break — dropped
    entirely. Kimi K2.7 Code in any seat — removed by Tom. Grok in any
    role (usage exhausted; bake-off test call excepted). Haiku routed
    through OpenCode, Luna routed through OpenCode, and any OpenCode
    model other than DeepSeek (implementer). Any flagship use
    outside the 3 occasions and the brief-run contract above.
  - DeepSeek as a screenshot scorer. Standard DeepSeek V4.1 Flash / V4 Pro
    are text implementers here. `deepseek-v4-flash-vision-exp` must never
    give a final score: it downscales images to ~800x800, so type and
    alignment detail is lost. Coarse layout-only comment at most.
  - Silent substitution. If an assigned model ID is missing or a delegate
    cannot accept images, log the exact failure and follow the seat's
    stated rule (gates: continue without it; panel/implementer: stop and
    report). Never reroute a model to a different provider than the one
    named for it, and do not quietly promote a flagship into a cheap seat.

All reviewers are read-only: instruct every delegate in writing to edit
nothing and return only the structured score block (§3).

## 0. Preconditions — test-run deviations from the original skill
- Screens: discover the desktop app's main screens, then pick the 3 most
  used. Only those 3 are in this run. List them in the scope file.
- Screenshot harness: if a deterministic screenshot command does not
  already exist, build a MINIMAL one for those 3 screens only (fixed size,
  theme, scale, seeded fake data, frozen time/animations/caret, PNG +
  manifest.json), as its own commit, and CONTINUE — do not stop for
  review (deviation: Tom authorised this pass to run unattended).
  If after 2 attempts the same commit does not produce pixel-identical
  output on this machine, stop the run and report that as the result.
  A failed harness is a valid test result; fake scores are not.
- Captures per screen, per round: light + dark at default size, plus
  100%-scale crops (header / main / footer or equivalent regions) for the
  Go-model reviewers. Full shots alone are not enough.
- Brief: if `docs/design/brief.md` does not exist, write a short one
  from what the repo itself shows (existing colors, fonts, density)
  and mark it DRAFT in the PR. Anti-goal for this run: do not turn the
  app into a generic Zed/Linear clone, and do not shift it grayscale.

## 1. Scope
Write `docs/design/council-scope.md` before touching UI code: the 3
screens, allowed changes (spacing, type scale/weight, color/contrast,
hierarchy, states, copy), and these test-run limits:
  - Max 2 scoring rounds: Round 0 (baseline) and Round 1 (after fixes)
  - Fix ONLY the 3 lowest-scoring categories, across those 3 screens
  - Conservative changes only: no palette overhaul, no typeface change,
    no layout rewrite. Direction questions go in the final report for
    Tom (a former designer, and the final arbiter) — do not block on them
  - No new features, no new dependencies, no unlisted screens

## 2. Calibration (before Round 0 scoring, no exceptions)
Make one calibration image from a real screenshot with 3 planted flaws:
low-contrast text, a misaligned element, clipped/overflowing text.
Send it to both gates and the panel model, each blind:
  - Must name at least 2 of the 3 flaws with their locations, and
  - Must return the structured score block cleanly.
Gate seats need TWO consecutive passes, not one — in run 1 Gemini
passed once and failed the retry, and a single pass let it through.
The same two-pass standard applies to the panel appointee (Terra)
and, if ever called, to the reserve (Sonnet).
A model that fails does not score in this run. Record pass/fail per
model (and per attempt for gates) in the PR — this table is one of the
test run's main deliverables.

## 3. Rubric (same 13 categories as the original skill)
Typography, Color and contrast, Spacing and rhythm, Alignment,
Iconography, Hierarchy and density, Consistency, Interaction states,
Craft and polish, Light/dark parity, Accessibility, Copy and voice,
Distinctiveness.
Each score: 1-10, one-line reason, and a specific element/location cited.
No citation = that point is discarded. Motion/resize: `no score` unless
a capture actually shows it — never guess from a still.
Hard gates regardless of averages: WCAG contrast, visible focus,
minimum hit targets. A hard-gate failure is reported as a failure,
not averaged away.
Reviewers get: brief, scope, this round's shots + crops, Round 0 shots,
and reference screenshots if the repo already has them.
Reviewers NEVER get: a target score, a round number, another reviewer's
scores, or the fact that a later round will happen.

## 4. Loop (test run)
1. Round 0: render, both gate reviews, panel review, NO flagship runs
   yet. Synthesise: keep only points at least 2 reviewers agree on, or
   that a gate + the panel member both cite.
2. Implement fixes for the 3 lowest categories only. Log every change as
   feedback -> change -> expected category.
3. Re-render identically. Run cargo fmt, clippy, and the workspace tests
   that cover the desktop crate, plus a keyboard/focus smoke check.
4. Round 1: fresh blind gate + panel reviews, then flagship occasion 1
   (final verdict, Sol by default, brief-run contract).
5. Stop. Even if scores are below 9, this run ends here — reaching 9+
   is the full skill's job, not the test's. Escalate mid-run only via
   flagship occasion 2 (tie-break) as defined above.

## 5. Output
- Save this skill in the repo where its other skills live (check for an
  existing skills convention first; if there is none, use
  `skills/ui-council-test-run/SKILL.md`). Do not overwrite or edit any
  existing `ui-council` skill.
- One branch / worktree, one DRAFT PR, do not merge, nothing public.
- PR description, before the model/harness line:
  - Calibration table (model x pass/fail + what it missed)
  - Before (Round 0) / after (Round 1) pairs per screen, numbered callouts
  - Every image in the PR body MUST be an absolute raw URL pinned to the
    PR's own branch:
    `https://raw.githubusercontent.com/<owner>/<repo>/<branch>/<path>`.
    Bare relative paths (`docs/...`) do NOT render in a PR description —
    they show as broken-image icons (learned on PR #63, pass 2). After
    writing the body, fetch each image URL and confirm it returns
    `200 image/png` before finishing.
  - `## Council review`: per-reviewer, per-category scores for both
    rounds, the delta on the 3 fixed categories, hard-gate results,
    rounds/models actually used, and any model that failed or was
    substituted (with the exact failure). Write `no score` where none
    was given; never invent one.
  - "For Tom" section: direction questions and any advice you overruled
  - "Later" list of out-of-scope ideas
- Final message in the thread: a short handoff Tom's Muse (outside T3)
  can verify — PR link, screenshot paths, scores headline, cost/time
  if the harness reports it, and what broke.
- No Co-authored-by: Cursor or any Cursor trailer; check git log first.
