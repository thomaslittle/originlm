# Conversation handoff across models

OriginLM owns a provider-independent transcript for every host conversation. A
host can change model or provider mid-chat without losing the earlier
conversation, and without re-sending history to a provider session that already
keeps it.

## Why a canonical transcript

Native clients (Codex, Claude Code, OpenCode, Muse, ACP profiles) keep history
inside their own session. HTTP providers keep it in the adapter's message array.
Neither store survives a switch to a different model or provider. A host-side
copy is also wrong: every app would need to reproduce the same budgeting,
redaction and provider-shape rules, and apps that re-send their own transcript
duplicate history for the same model.

OriginLM therefore stores the canonical transcript itself:

- one entry per completed turn: user text, assistant text, and compact notes for
  tool calls and produced artifacts;
- credentials are redacted before storage;
- the transcript is scoped to the application identity and the host-provided
  `conversationId`;
- it is separate from any native session and is never presented as provider
  resumption.

`sessions.resume` still reattaches a provider's own conversation. The transcript
is only used when a **new** provider session is opened for an existing
conversation.

## API

Create the first session with a stable conversation id and optional app
instructions:

```json
{
  "method": "sessions.create",
  "params": {
    "selection": { "routeId": "route-1", "modelId": "model-a", "revision": "1" },
    "workspaceId": "scratch",
    "conversationId": "thread-42",
    "instructions": "Current home: three lights are on; the front door is locked."
  }
}
```

The returned `Session` always carries `conversationId`. If the host omits it,
OriginLM generates one. After each completed turn the transcript is updated.

Switch model or provider with:

```json
{
  "method": "sessions.switchModel",
  "params": {
    "sessionId": "session-1",
    "selection": { "routeId": "route-2", "modelId": "model-b", "revision": "1" }
  }
}
```

`switchModel`:

1. requires the previous session to be idle or completed;
2. validates the target selection against the current catalog and grant;
3. opens a fresh provider session for the target;
4. seeds it with the canonical transcript, budgeted for the target model;
5. closes the previous provider session;
6. returns the replacement `Session` with a `history` summary.

`instructions` and `allowPaid` are inherited when omitted. `persistent` is
inherited only when the target route can persist a native conversation; it can
be set explicitly. The old session is closed only after the new one opens.

A host may also call `sessions.create` again with an existing `conversationId`
(for example after closing a session or restarting). It is seeded the same way.
A resumed native session (`sessions.resume`) is **never** seeded: the provider
already has the history.

`Session.history` reports what was carried:

```json
{
  "turns": 8,
  "omittedTurns": 3,
  "estimatedTokens": 5120,
  "trimmed": true
}
```

`trimmed` is true whenever any turn was dropped, either by the target budget or
by the stored-transcript cap. Trimming is deterministic and is also stated in
the seeded system/prelude text.

## Seeding rules

| Target | How history is delivered |
| --- | --- |
| HTTP providers (Ollama, LM Studio, OpenAI, Anthropic, Gemini, xAI, compatible) | Real role-tagged messages. A system message carries app instructions and the trim notice; completed turns are user/assistant messages. Anthropic and Gemini receive the system layer in their provider-specific field. |
| Native CLIs (Codex, Claude Code, OpenCode, Muse, ACP/native profiles) | One clearly delimited `[OriginLM conversation handoff]` block on the first turn only. Subsequent turns go to the native session unchanged. |
| Native resume | No seed. The provider session already has the history. |
| Same session, next turn | No seed. The HTTP adapter or native client already retains it. |

This removes the double-send failure mode where an app re-sends its own
transcript to a session that already has the same history.

## Budgeting

The target model's reported `contextWindow` is used when available; otherwise
OriginLM assumes 32,000 tokens. OriginLM reserves room for output and app
instructions, keeps the newest whole turns verbatim, and drops the oldest whole
turns. Seeds are also bounded to 20 turns and 512 KiB so a fresh HTTP session
fits the adapter's message and frame limits. App instructions are never trimmed.

## Attachments

Text, tool-call summaries and artifact references carry forward. Image bytes do
not: the canonical transcript stores a compact reference such as
`image image/png 1024x768`, not the payload. A new provider is never sent a
stale image payload; the host must re-attach an image on the turn that needs it.
Produced artifacts remain readable through `artifacts.read` while the session
is alive, but a switch does not copy their bytes into the transcript.

## What cannot carry over

- Native-client-only state: pending approvals, working-directory sandbox state,
  live tool availability, and the provider's internal reasoning.
- Attachment bytes (images and artifact content). Only references survive.
- Provider-specific hidden reasoning or cache state.
- A conversation exactly as the old provider tokenised it. The new model gets
  the canonical text, which is the meaningful part.

## Secrets

Transcripts never intentionally store credentials. Before storage, OriginLM
redacts bearer/basic values, common key assignments (`api_key`, `token`,
`password`, `client_secret`, and similar), private-key blocks and common token
prefixes (`sk-`, `ghp_`, `github_pat_`, `xox*`, `AKIA`, JWT-shaped values).
Host MCP bearer tokens are separate and continue to be re-supplied on resume.

## Compatibility

The API additions are optional and additive: `conversationId` and `instructions`
on `sessions.create`, the `sessions.switchModel` method, and `conversationId` /
`history` on `Session`. Existing hosts that omit them get exactly the previous
behavior. The runtime advertises `session.conversation.handoff` in
`runtime.handshake`.
