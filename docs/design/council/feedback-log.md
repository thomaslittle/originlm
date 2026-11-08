# Round 0 feedback -> change log

Round 0 reviewers: gate `antigravity_2/gemini-3.8-flash-high` (completed
late), panel `opencode-go/kimi-k2.7-code` and `opencode-go/minimax-m3`.
Consensus lowest three categories (mean of the three scores):

| Category | Gate | Kimi | Minimax | Mean |
|---|---|---|---|---|
| Craft and polish | 6 | 5 | 5 | 5.33 |
| Accessibility | 6 | 5 | 5 | 5.33 |
| Consistency | 6 | 6 | 5 | 5.67 |

Only points cited by at least two reviewers, or by the gate plus one panel
member, were kept. Changes are conservative: no palette, typeface, layout
or feature changes.

## Craft and polish

| Feedback (source) | Change | Expected category |
|---|---|---|
| Right-rail alert clips mid-word ("not verifie") — gate, Kimi, Minimax | Alert text is `flex_1` + `min_w(0)` so it wraps inside the 320px rail | Craft, Copy |
| Routes value clips at the right rail edge — gate, Kimi, Minimax | Value wraps instead of overflowing | Craft |
| Allowance labels/figures truncate ("Count witho…", "17 re…") — gate, Kimi, Minimax | Count windows read "Count only"; label 68→78px, figure 42→72px, meter 96→76px | Craft, Typography |
| Usage history repeats the provider name and identifier — Minimax | Mono identifier only renders when it differs from the display name | Craft, Hierarchy |
| "Models" action in the account inspector reads as a label — Minimax | Ghost action gains the CPU icon | Craft |
| Selected-row amber edge is clipped by `overflow_hidden` — gate | Edge moves from `left:-2` to `left:0` | Craft, Interaction |

## Accessibility

| Feedback (source) | Change | Expected category |
|---|---|---|
| Light-theme chips lose their fill — gate (parity), Kimi, Minimax | Chip tint 8%→14%, border 40%→55% opacity | Accessibility, Parity |
| "No local transcript history" is small and faint — Kimi, Minimax | Empty-state line uses body size instead of caption | Accessibility |
| Overview provider rows lack a focus ring — gate | Row gets `role(Button)` and `focus_ring` | Accessibility |
| Mini view toggle is 26px, below the 28px target — gate | Height uses `ds::TARGET` (28px) | Accessibility |

## Consistency

| Feedback (source) | Change | Expected category |
|---|---|---|
| "stale" vs "Stale" — Kimi | Title case in the allowance line | Consistency |
| "plan unknown" vs title-case plan names — Kimi | "Plan unknown" | Consistency |
| Route billing chip is lowercase next to title-case status chips — Kimi | Billing labels are title case | Consistency |
| Table head padding 14px vs row 12px — gate | Table head uses `sp::MD` (12px) | Consistency, Alignment |

## Left for Tom / later (not changed)

- Accounts has a large dead area below the table at 1440. Filling it means a
  layout change or new content, outside this run.
- The Accounts "Models" column is 2 for every row in example data. Removing
  or replacing it is an information-architecture decision.
- The observed-cost chart has no axis or scale; adding one is a feature.
- Status vocabulary still differs by screen ("No reading", "No quota
  window", "Sign-in required"). Normalising it is a product decision.
- Session-token metrics in Usage history are not column-aligned.
- Sidebar footer caption uses a one-off 13px indent.
