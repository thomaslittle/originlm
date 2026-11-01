# Integrating a prompt-driven desktop app

For a prompt-driven desktop app, use OriginLM to discover access, choose a route and model, and stream a text response. Keep output schemas, validation, asset handling, and rendering in the application.

## Host and renderer

For Tauri, embed `originlm-kit` in the Rust host and use the optional React selectors in the renderer. For a Node/Electron host, use the TypeScript SDK with the bundled helper over stdio. A normal hosted website cannot discover a visitor's installed clients without an explicitly paired local helper or browser extension.

The host registers endpoint/native-client configuration and grants. The renderer receives normalized public data and narrow application commands. Do not accept executable paths, arbitrary workspace paths, credentials, or unrestricted grant objects from renderer content.

## User flow

1. On the user's Connect action, enable passive discovery and scan. Imports and initial rendering stay inert.
2. Run explicit metadata checks on selected connections. `readCatalogSnapshot` and `createSnapshotStore` assemble paged catalogs. Authentication, billing, freshness, and capabilities remain separate facts.
3. Present the provider, connection, model, and its native options. Retain the complete `Selection`: `routeId`, `modelId`, `revision`, and `options`. Model names alone are not an execution identity.
4. Validate `generation.text` and the exact selection through the service. Local Ollama/LM Studio routes do not require an invented account login. Unknown billing is blocked until the host supplies a justified route classification; allowing metered access does not turn unknown billing into verified access.
5. Ask for the app's required workspace and execution scope, then register the host grant. For text-only structured output, use read-only mode and keep native tools disabled. Do not opt into an installed agent's native tool policy merely to generate JSON.
6. Create a session. Start one turn with a unique request ID only after the user submits their prompt. Creating a session never sends a prompt. A turn prompt may contain at most 512 KiB of UTF-8 text. The serialized `turns.start` params object has a separate limit of 1 MiB minus 8 KiB, reserving space for protocol envelopes. JSON escaping can make the serialized limit apply first. Transport and provider frame limits still apply, including Browser Native Messaging's 1 MiB whole-frame limit; unusually large provider-specific options or paths can consume the reserve.
7. Consume the event stream, filtered by session and turn ID. Subscribe with a saved cursor before starting, or use the native `events.read` cursor/tail cursor to replay the same turn. Handle `EVENT_GAP` explicitly. An ambiguous start failure must not trigger an automatic retry, new session, or provider fallback.
8. Treat `final` status as the result. Cancellation uses the returned turn ID. Close the session and application client on shutdown; this only releases that application's resources.

Use the [embedded Ollama integration test](../../crates/originlm-kit/tests/local_execution.rs) as a concrete host example. It exercises discovery, metadata, selection validation, denied execution before grants, session creation, streaming, and cleanup through the public embedded API against a loopback fixture. The [Tauri guide](../../examples/tauri/README.md) covers the host boundary. The live showroom's default launcher grants metadata only; its disabled execution controls are expected.

## Structured output and continuation

Require your own versioned output JSON schema and validate the completed text before previewing or saving it. Do not infer structured-output support merely because a provider advertises it. Display unsupported options honestly. Provider output must not become executable HTML or host commands automatically.

## Image input

A turn may carry up to four JPEG or PNG images, for example a rendered preview for a review pass. Check `handshake.capabilities` for `turn.input.image` and `handshake.methods` for `inputs.put` first; older helpers have neither.

1. Stage each image in an open session: `inputs.put { sessionId, mediaType: "image/jpeg" | "image/png", dataBase64, displayName? }` returns `{ inputId, byteLength, width, height }`. The helper decodes the base64 and reads dimensions from the PNG `IHDR` or JPEG `SOF` header; a payload that does not match its media type is `INVALID_REQUEST`. Limits: 4 MiB decoded (`FRAME_TOO_LARGE` above that), 2048 px on the longer edge, 8 unconsumed inputs per session and a per-application byte budget (`RATE_LIMITED`). `displayName` is validated and never sent to a provider. Payload bytes are never logged or echoed back.
2. Start the turn with `turns.start { sessionId, prompt, requestId, inputs: [{ inputId }] }`. Omitting `inputs` keeps the text-only behavior unchanged.
3. The helper fails the start with `CAPABILITY_UNSUPPORTED`, before any provider call, when the selected model does not report `input.image` as `supported` (unknown is not supported) or when the session's adapter cannot carry images. Images are never silently dropped. A rejected start does not consume the inputs or the request ID, so the host can retry text-only.
4. A started turn consumes its inputs. Inputs are scoped to the owning application connection and session; another application or session sees `NOT_FOUND`. They expire when the session is closed or suspended, when grants are revoked, and when the helper restarts.

Images belong to the turn that attached them. Native Claude Code and Codex sessions forward images to the client (Claude Code as base64 `image` content blocks, Codex as app-server `image` input items with a data URL); the client keeps them in its own conversation history. HTTP routes keep text-only conversation history, so follow-up turns do not resend or accumulate earlier images; attach the image again if a later turn needs it. Frame limits: native stdio and HTTP `/v1/rpc` accept 8 MiB request frames (the TypeScript and Python stdio SDK defaults match); the browser Native Messaging bridge stays at 1 MiB, which bounds images sent through it to roughly 750 KiB. The [support matrix](../adapters/SUPPORT-MATRIX.md) lists which adapters carry images.

Keep an open session for follow-up edits. Native suspend/resume is available only when the session reports `nativeResumption: true`; currently this is the shared ACP path when its peer advertises `loadSession`. Keep the opaque token in app memory and never render/log it. Tokens are owner-scoped, single-use, and invalid after revocation or helper restart. Failed resumption never becomes a fresh conversation. Other providers require a fresh session after restart; transcript import is a separate application feature.

## Readiness boundary

This is a developer alpha. Deterministic adapter and package tests establish mechanics, not a guarantee that every installed version or user's subscription accepts inference. Live metadata checks do not qualify generation. Before shipping your integration, qualify the app's actual host lifecycle, output validation, cancellation, and the specific provider versions you intend to support. See the [support matrix](../adapters/SUPPORT-MATRIX.md) for provider-specific limits.
