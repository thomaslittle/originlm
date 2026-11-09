# Image generation tool for the OriginLM agent — spec v1

Owner-approved; launched 2026-11-09. Codex engineering lane.

## Problem and goal
OriginLM agents have no image-generation tool. A user asking any composer model for scene imagery should receive a real image inline, at the caller's canvas size, without leaving the app. Hosts decide scene placement.

## Tool
`generate_image` takes required `prompt`, optional `size`/`aspect` (default caller canvas, e.g. 1920×1080), optional `style`, optional `reference_image` for edits/variations, and `count` (default 1, maximum 4). The caller supplies the asset directory. Results contain file references and provider/model/dimension metadata. Errors distinguish no configured provider, quota/billing, content refusal, and provider failure; no silent fallback.

## Providers and credentials
Pluggable adapters: OpenAI Images 2.6: `gpt-image-2.6-flare` (interactive default) and `gpt-image-2.6-sunburst` (fidelity/edit consistency), using `/v1/images/generations` and `/v1/images/edits`, reference edits, per-token billing and resolutions up to 4K. Google: `gemini-nano-banana-2.1` (default, fastest/cheapest) and `gemini-3-pro-image` (hero images and exact text), including the owner's Google/OpenRouter routes. Model IDs remain configuration. Follow explicit user configuration and existing credential machinery. Keys belong to the user; no hosted or billed-by-us path.

## Agent behavior
The tool description tells every calling model to use it for scene backdrops, overlays and thumbnails. Include scene purpose/name, canvas aspect and collection art direction in the prompt. Present the result inline and offer variations instead of silently dumping files.

## Host integration
OriginLM desktop must show ask → image inline. Crntly composer renders results inline; “add to scene” inserts the generated file as a background/media source at canvas size in the named scene. Only this placement layer is Crntly-specific. Implement if contained, otherwise document a scoped follow-up in the PR.

## Build order and acceptance
1. Commit this specification first.
2. Implement tool, adapter interface, both providers, mocked-provider tests.
3. One live generation if a genuine provider credential exists; include image in PR. If none, state that plainly and use mocks. Never print or commit credentials or hunt repeatedly for keys.
4. Prove desktop ask → image inline where feasible.
5. Crntly integration or scoped follow-up brief.

Implement first. Exactly one final local build/test verification pass, then hard stop; leftovers go in the PR. Ignore CI. Open one draft PR against main; never merge.

## Non-goals
Hosted billing, image-editor UI, batch asset packs, video generation.

## Owner correction — 2026-11-09
The original OpenAI gpt-image-1 reference is superseded: owner reports shutdown on 2026-10-23. Target the Images 2.6 pair shipped 2026-11-04, default Flare. Add Nano Banana 2.1 (GA 2026-10-06), default Google route; retain stable Pro. This correction is authoritative for implementation.

