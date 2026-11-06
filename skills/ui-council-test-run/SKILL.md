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

## Model assignments (exact IDs — resolve first, do not guess)
Implementer / main thread:
  - opencode / opencode-go/deepseek-v4.1-flash
    (abundant OpenCode Go usage; implements from structured text feedback,
     it is NOT a screenshot judge)

Gate reviewer (every round, delegate_task, role: review):
  - antigravity_2 / gemini-3.8-flash-high
  - Fallback, in order: antigravity_2 / gemini-3.8-flash-medium,
    then opencode / opencode-go/minimax-m3

Panel reviewers (Round 1 only, blind, independent, role: review):
  - opencode / opencode-go/kimi-k2.7-code
  - opencode / opencode-go/minimax-m3

Final reviewer (ONE pass, final screenshots vs Round 0 only):
  - codex / gpt-6.1-sol   (Tom's $20 Codex plan — never use it as the gate)

Optional tie-breaker if panel splits >2 points on a category:
  - antigravity_2 / claude-sonnet-5-5-medium
    (This is Antigravity quota, NOT Tom's Claude Max. It is the only
     Claude allowed anywhere in this test run, and only in that case.)

FORBIDDEN in this run:
  - The `claudeAgent` provider in any role — Tom's Claude Max is at 3%
    until Saturday. Do not burn it in a loop.
  - DeepSeek as a screenshot scorer. Standard DeepSeek V4.1 Flash / V4 Pro
    are text implementers here. `deepseek-v4-flash-vision-exp` must never
    give a final score: it downscales images to ~800x800, so type and
    alignment detail is lost. Coarse layout-only comment at most.
  - Grok in any role (usage exhausted).
  - Silent substitution. If an assigned model ID is missing or a delegate
    cannot accept images, log the exact failure and use only the fallback
    named above for that seat. If no fallback works, stop that seat and
    report it — do not quietly swap in Claude or Codex early.

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
  100%-scale crops (header / main / footer or equivalent regions) for
  the Go-model reviewers. Full shots alone are not enough.
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
Send it to the gate and both panel models, each blind:
  - Must name at least 2 of the 3 flaws with their locations, and
  - Must return the structured score block cleanly.
A model that fails does not score in this run. Record pass/fail per
model in the PR — this table is one of the test run's main deliverables.

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
1. Round 0: render, gate review, panel review, final-reviewer does NOT
   run yet. Synthesise: keep only points at least 2 reviewers agree on,
   or that the gate + one panel member both cite.
2. Implement fixes for the 3 lowest categories only. Log every change as
   feedback -> change -> expected category.
3. Re-render identically. Run cargo fmt, clippy, and the workspace tests
   that cover the desktop crate, plus a keyboard/focus smoke check.
4. Round 1: fresh blind gate + panel review (final reviewer now runs,
   once, on Round 1 vs Round 0).
5. Stop. Even if scores are below 9, this run ends here — reaching 9+
   is the full skill's job, not the test's. Only escalate mid-run if a
   hard gate fails or reviewers split >2 points (optional tie-breaker).

## 5. Output
- Save this skill in the repo where its other skills live (check for an
  existing skills convention first; if there is none, use
  `skills/ui-council-test-run/SKILL.md`). Do not overwrite or edit any
  existing `ui-council` skill.
- One branch / worktree, one DRAFT PR, do not merge, nothing public.
- PR description, before the model/harness line:
  - Calibration table (model x pass/fail + what it missed)
  - Before (Round 0) / after (Round 1) pairs per screen, numbered callouts
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
