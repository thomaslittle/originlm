# OriginLM promo — source and delivery

1920 × 1080 · 30 fps · 26.67 s (800 frames) · H.264 High / yuv420p · silent by design.

The film follows one loop: the per-client glue pile → one local inventory → one granted
streaming session inside a host app → the safety boundary → the real wordmark.

## Files

- `originlm-promo.mp4` — master, encoded from 800 raea PNG frames with x264 CRF 14.
- `evidence/mid-frame-14s.png` — a frame pulled from the MP4 with ffmpeg (visually checked).
- `evidence/safety-20.5s.png`, `evidence/end-24s.png` — later frames pulled from the MP4.
- `evidence/signalstats-14s.txt` — ffmpeg signalstats for the mid-frame (YAVG 137.1, YMAX 238).
- `evidence/ffprobe.txt`, `evidence/decode-check.txt` — stream facts and a clean decode pass.
- `evidence/contact-sheet.png` — 5×4 contact sheet of the whole cut.
- `evidence/gpu-path.txt` — what the GPU path did on this machine, including the failed forced-ICD attempt.
- `check.log`, `check.json` — `raea check` on the export. Final state: 0 P0, 0 P1; five P2 reading notices.
- `edit.tsx`, `film/`, `brand/`, `media/`, `styles.gen.ts` — the composition and its real assets.

## Regenerate (inside WSL, from an ext4 copy)

```sh
raea render edit.tsx --at "$(seq -s, 0 799)" -o frames/ --lite --manifest render.json
ffmpeg -framerate 30 -i frames/frame-%05d.png \
  -c:v libx264 -preset slow -crf 14 -pix_fmt yuv420p -profile:v high \
  -movflags +faststart -color_primaries bt709 -color_trc bt709 -colorspace bt709 \
  originlm-promo.mp4
raea check originlm-promo.mp4 --manifest render.json -o check.json
```

`raea render -o frames/` is the reliable path for this raea canary on WSL: the direct
MP4 path is cheaper but its page readback produced trails/stale pixels, and forcing
`VK_ICD_FILENAMES` produced all-green frames. Rendering PNG frames and encoding with
x264 gave clean, repeatable pixels.

## Claims, and where they come from

| On screen | Source |
|---|---|
| Nine clients: Codex, Claude Code, Gemini CLI, Grok, OpenCode, Cursor, GitHub Copilot, Ollama, LM Studio | all nine have passive-detection rows in `docs/adapters/SUPPORT-MATRIX.md` |
| "One local control center." | README introduction |
| "Finds what is installed. Keeps the evidence." | README: passive discovery, evidence-backed inventory |
| "Passive discovery · metadata only · no secrets read · no network calls" | README discovery section (scoped to discovery, not to granted sessions) |
| "Unknowns are never filled in." | README: unknowns are never filled in with guesses |
| Provider / installation / profile / account / connection / route / model kept distinct | README: evidence-backed inventory |
| "Grants are explicit. Revocable." / grant dialog | README safety model: metadata, execution, workspace access, tools and paid options are separate, revocable, per-app grants |
| "Discovery is not permission." | README: a discovery signal is never permission to execute |
| "Credentials never reach your app." | README/ADR-003: credentials are never returned to consuming apps |
| "No telemetry. No hosted relay." | ADR-003 privacy boundary: no OriginLM telemetry or hosted relay; history and grants stay local |
| "Claude Code · Haiku" | support matrix: Claude Code + Haiku is live-proven (September 28) |
| "alpha · MIT" | README badges |

No npm/install line is shown: the repo's distribution status says package names are
provisional and source packages remain private/unpublishable, so `npm install originlm`
is not a safe claim today.

No benchmarks, customer names, prices, GA claims, provider logos or generated imagery
are used. Provider names are plain type. The only logo is the supplied OriginLM SVG.
