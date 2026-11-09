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


## Embedding and configuration

`originlm_kit::images` exposes the reusable Rust API (also available as
`originlm_images`). Hosts select a `RouteConfig` with an existing OriginLM
`EndpointConfig` and its `keyEnv`; credentials are read only on explicit connect.
`RouteConfig::standard` supplies route-picker defaults; it never selects a route
based on which keys happen to exist. `model` overrides the default, including
`gpt-image-2.6-sunburst` and `gemini-3-pro-image`. OpenRouter uses its qualified
`google/…` model ID. Native chat subscriptions/OAuth are not image API credits.

After paid-operation approval, construct `ImageTool` with an absolute asset
directory and `Canvas`, then `mcp::ImageServer::start`. Attach its `descriptor`
to the session's `hostMcpServers` and keep the server and Tokio runtime alive
until the session closes. The tool is advertised to Codex/Claude/ACP through
OriginLM's existing host MCP path. Consume `results` for trusted file metadata;
MCP responses include preview images and full-resolution file references.
Reference images must be relative paths inside that same asset directory.

Provider responses are decoded with allocation/dimension limits, resized with a
centered cover crop to the exact requested canvas, and written as PNGs in a
unique subdirectory. Metadata records provider, model, original dimensions and
`fit: cover_center`. Errors never include upstream response prose. Failed
multi-image calls do not publish partial files (provider charges for earlier
completed requests can still apply). There are no provider/model retries.

### Desktop

Set `ORIGINLM_IMAGE_ROUTE` to an absolute JSON route-config file and enable the
existing “Allow paid or unknown-billing routes” switch for the turn. The desktop
attaches the tool only to those explicitly paid turns; successful files render
inline from the trusted result channel. With no image route configured the tool
returns `no_provider`, rather than inventing images or borrowing chat credentials.
The desktop's default image canvas is 1920×1080. Image-route billing is separate
from the selected text model.

Example config (contains a credential variable name, never a key):

```json
{
  "provider": "openai",
  "endpoint": {
    "id": "images", "label": "My OpenAI image route", "kind": "openai",
    "baseUrl": "https://api.openai.com/v1/", "keyEnv": "OPENAI_API_KEY"
  },
  "model": "gpt-image-2.6-flare"
}
```

### Crntly follow-up scope

In crntly-desktop, reuse the kit image server; supply collection canvas and a
collection-specific asset root, pass scene name/purpose and art direction in
session instructions, and render the trusted result metadata inline. Add an
“Add to scene” action next to each image. Its handler inserts the local PNG as
a media/background source into the named scene at canvas size using Crntly's
existing source mutation/undo path. Do not infer placement from model prose.
Acceptance: any supported composer model → generate_image → visible preview →
Add to scene → persisted source in the chosen scene; missing key, refusal and
quota errors shown inline; no scene mutation before the user selects placement.

Live smoke (one request, only after confirming the selected key exists):
`cargo run -p originlm-images --example smoke -- openai ABSOLUTE_ASSET_DIR`.
Use `gemini` or `openrouter` for that explicitly selected route instead. The
example prints only file metadata. Attach the resulting PNG to the draft PR;
never attach configuration containing credentials. This example does not retry.

Direct text-only HTTP conversation routes currently do not advertise host MCP
and therefore cannot call this tool yet. The desktop preserves their existing
text execution instead of attaching an unsupported server. Native host-tool
integrations (including Codex and Claude) use the image server independently of
their text-model credentials. Extending HTTP chat tool-call loops is a separate
follow-up; this is a remaining gap against the “any model” goal.

## Desktop follow-up proof — 2026-11-09

Fixed the failing image assertion by adding the image container's actual accessibility label, consistent with the app's other observed elements. Local desktop build and all 25 desktop tests pass.

The running native Windows app also completed a self-driving smoke run through its real composer submission method, broker worker, native Codex session, authenticated MCP image tool and local mock HTTP image provider. The app's transcript received the 1920×1080 PNG and the assistant's inline-image response / offer of a variation. See `docs/evidence/image-generation/result.json` and the explicitly labeled mock PNG. This proves the integrated app data path with a mock image provider; it does not qualify a live billed image provider or independently assert compositor pixel correctness.

For repeatable testing without OS mouse/keyboard automation, set `ORIGINLM_DESKTOP_IMAGE_SMOKE` to an absolute JSON config path before launching the desktop. It submits one turn, leaves the window open, and writes the report after the turn ends (or 180 seconds). Config fields: `prompt`, `providerId` (native conversation provider, e.g. `codex`), `allowPaid` (explicit approval for this test turn), and absolute `report` path. Normal app startup is unchanged when this opt-in variable is absent.

`scripts/image_generation_mock.py` is a local-only fixture server (requires Pillow), bound to 127.0.0.1:18763. It writes a clearly labeled synthetic PNG and captures the request beside the script. For a mock image route, use the standard OpenAI endpoint config with `baseUrl` set to `http://127.0.0.1:18763/v1/` and a dedicated `keyEnv` bound to a non-secret placeholder. Never supply a real API credential to the mock. Configure `ORIGINLM_IMAGE_ROUTE` normally; no production route or credential is altered by this harness.

### Generated-image file actions

Each inline result offers **Save image…** (native save dialog; copies the original PNG), **Open image** (system default viewer), and **Show in folder** (system file manager). Cancelling Save leaves the original untouched; dialog/copy failures show a toast. The native GPUI test clicks Save, chooses a temporary destination, verifies byte-for-byte equality with the generated asset and exercises cancellation. Latest desktop build and all 25 desktop tests pass. The updated running app completed the self-driving Codex/local-mock flow again with one 1920×1080 image and no error.


## Subscription-first correction — owner follow-up 2026-11-09

The desktop now defaults to the installed Codex client's native image generation using its existing login. No image API key is needed for this route. The conversation model remains independent: a Claude conversation can call the host image tool backed by Codex. This does not claim a separate native Claude image engine. An explicit `ORIGINLM_IMAGE_ROUTE` still selects the BYOK adapters above; failure on that explicit route never falls back to another provider.

`NativeCodexProvider::new(executable, codex_home)` is available to other hosts. It launches the trusted native Codex executable with image generation enabled and shell/exec/browser/computer/apps/plugins/hooks/multi-agent tools disabled, in a temporary workspace. It uses the specified Codex profile's own credentials, never extracts OAuth tokens, and imports only the PNG bound to the emitted native thread ID. The desktop selects `CODEX_HOME` or the default `.codex` profile, never an arbitrary discovered secondary profile. The native backend model ID is not exposed by this CLI path, so metadata explicitly says `codex-native-image-generation` rather than inventing an API model ID.

A Windows EOF bug in the existing bounded subprocess capture helper was fixed: stdin must be dropped, not merely flushed, so `codex exec` does not wait indefinitely for piped input. A Python child regression test verifies EOF before output; the 14 host MCP tests still pass.

Live evidence is now **real subscription generation**, not only the earlier mock: `docs/evidence/image-generation/subscription-result.json` records the actual desktop ask → host tool → native Codex generation → normalized 1920×1080 image → assistant response, with no API route or key. `subscription-backdrop.png` shows the blue mountains, valley mist and amber horizon. A direct native CLI probe and the actual app generation both succeeded. Earlier mock artifacts remain explicitly labeled test fixtures and are not representative generated artwork.

The running app has compact icon-only Save, Open and Show in folder controls with tooltips and accessible names. Provider/model implementation text is hidden from the visual image footer; only dimensions remain. All 25 desktop tests pass, including native icon/action labels and save-copy behavior; all 8 image tests pass (7 adapter/tool tests plus native artifact binding). The app was relaunched without mock configuration and only one test instance is left open.

Remaining native limitations: the CLI capture operation is bounded to 60 seconds per image; native quota/auth failures currently use a generic provider-failure message rather than a dedicated native error classification. Real subscription reference-edit and count>1 qualification remain unproven.


## Resumed stdin-EOF checkpoint acceptance

The final resumed pass passed formatting, the combined desktop/images/kit build, 9 image tests (including exact wire IDs for all four corrected models), the stdin-EOF regression, 14 host-MCP tests, 25 desktop tests and 3 kit tests: 52 passed, 0 failed. `docs/evidence/image-generation/resume-verification.json` records the commands, results, credential status and leftovers. The existing real subscription-backed desktop image/report remains the live ask-to-inline proof; it was not regenerated. No supported image API key or explicit API route was present in the bounded environment check. BYOK live qualification remains unavailable rather than implied by the native subscription proof. Terra read-only review found no new blocking defect. Draft PR #71 stays unmerged; no CI was inspected or used.
