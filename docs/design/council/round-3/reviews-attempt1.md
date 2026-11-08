# Pass 3 scoring — attempt 1 (invalidated by a packet path defect)

Three blind review delegates were run with 29 image paths. Two Set A
allowance-crop paths pointed at `round-2/crops/`, which does not exist; the
before-crops live in `round-3/crops/`. Haiku reported the two failed reads,
Luna reported "Set A: none seen", and Terra did not open the image paths at
all and returned `no score` for every category.

These runs are recorded for honesty and are **not** used for the pass-3
scores. The packet was corrected and all three seats were re-run as new
blind delegates with a read-receipt requirement.

## Haiku (gate A)

1) CLIPPED OR TRUNCATED TEXT

Set A (before):
- Overview, Providers > Allowance, Antigravity row: "17 request..." is cut mid-word.
- Overview, Providers > Allowance: "Example sessi..." on the Codex Personal and OpenCode Linked rows, and "Example week..." on the Codex Work row, are ellipsized mid-word.
- Overview, bottom edge: "Recorded session tokens" sits at the viewport edge with its body cut off; same in both sets.

Set B (after):
- Overview, Providers > Allowance: none seen. "Count only" / "17 requests used", "Example session window" / "75% 1h 0m" and "Example weekly window" / "30% Stale" all fit in all captures.
- Usage history, Accounts: none seen.
- Note: two of the 29 Read calls returned "File does not exist"; it could not tell which.

Scores: Typography 7, Color 6, Spacing 7, Alignment 7, Iconography 7,
Hierarchy 8, Consistency 7, Interaction 7, Craft 7, Parity 7, Accessibility 5,
Copy 8, Distinctiveness 5. Delta: alignment, hierarchy, consistency,
interaction, craft, accessibility and copy improved; the rest unchanged.
Text contrast and non-text contrast UNVERIFIABLE from pixels; focus PASS
(dark); hit targets UNVERIFIABLE.

## Luna (gate B)

Set A: "none seen." Set B: "none seen." Scores: Typography 8, Color 7,
Spacing 8, Alignment 8, Iconography 7, Hierarchy 8, Consistency 8,
Interaction 8, Craft 8, Parity 8, Accessibility 6, Copy 8, Distinctiveness 7.
All affected categories "improved". (Set A's allowance truncation is
visible in the full shots, so this read of Set A is not trustworthy.)

## Terra (panel)

Returned `no score` for all 13 categories: "the images were provided as file
paths and are not visible in the conversation." No images read.
