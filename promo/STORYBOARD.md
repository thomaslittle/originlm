# OriginLM — promotional film storyboard

Format: 1920×1080, 30 fps, 26.7 s (800 frames), no narration, silent-viewable.
Every beat is carried by type and drawn UI; no stock, generated imagery or footage.

| # | Scene | On screen | Narration (none) | Payoff |
|---|---|---|---|---|
| 1 | 0.0–4.6 s | A pile of nine client tiles drifts in a graphite void: Codex, Claude Code, Gemini CLI, Grok, OpenCode, Cursor, GitHub Copilot, Ollama, LM Studio. Dashed "glue" strands connect every pair, each tagged with the work an app re-implements: paths, metadata, auth probe, catalog, stream, retries. Title: "Nine clients. Nine integrations." | — | The viewer recognises their own integration wall. |
| 2 | 4.6–10.2 s | The strands retract into one amber core. The pile resolves into an evidence-backed inventory: each client becomes a row with state chips (found, profile, unknown) and a source tag. Camera keeps pushing in. Text: "One local control center. Evidence, not guesses." | — | Belief step 1: one thing sees them all, locally. |
| 3 | 10.0–16.7 s | A host app window ("Atlas" review tool) is the frame; OriginLM is a thin strip. A grant dialog appears: "Atlas requests → session · stream · revocable". Grant lands, tokens stream into the app's own composer. Text: "Your app stays your app." | — | Belief step 2: the app uses AI without adopting a provider SDK. |
| 4 | 15.9–21.5 s | Safety beat: four rules cut in on moving grid rails — "Local control plane", "Discovery is not permission", "Explicit, revocable grants", "Credentials never returned to your app". A fifth line resolves: "No telemetry. No hosted relay." | — | Belief step 3: safe by construction, not by promise. |
| 5 | 21.5–26.7 s | The real OriginLM wordmark settles over the grid; tagline "See your AI. Build with it." and "alpha · MIT". Amber rule draws under the mark; camera keeps drifting. | — | Name, promise, status. |

## Claims map (all from README, ADR-003, support matrix)

- Nine named clients are all in `docs/adapters/SUPPORT-MATRIX.md` passive-detection rows: Codex, Claude Code, Gemini CLI, Grok, OpenCode, Cursor, GitHub Copilot, Ollama, LM Studio.
- "Passive discovery … from metadata only. No scanning at import time, no secrets read, no network calls." (README)
- "Evidence-backed inventory … Unknown and unsupported are different states" (README)
- "Credentials are never returned to consuming apps." (README / ADR-003)
- "No telemetry or hosted relay. History and grants stay on your machine." (ADR-003)
- "a discovery signal is never permission to execute" and grants are "separate, revocable, per-application grants" (README safety model)
- "alpha", "MIT" (README badges)
- No install command on the end card: README's distribution status says names are provisional and source packages remain private/unpublishable, so an `npm install` line would not be accurate today.
