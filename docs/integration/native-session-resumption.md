# Native session resumption

Completed turns remain suspendable. The demo continues the resumed session when the route, options, workspace, and billing consent are unchanged. A failed or timed-out suspend leaves that session unusable because the provider's conversation state is uncertain. Revocation and publication are synchronized across grant lifetimes; restoring the same grant cannot revive an older in-flight session or token.

OriginLM resumes only conversations the native client itself keeps. There is no transcript import or toolkit-owned conversation database disguised as provider resumption. A session resumes when its adapter reports `session.nativeResumption === true`:

| Integration | How the conversation is kept | How it is reattached |
| --- | --- | --- |
| Codex app-server | `sessions.create({ persistent: true })` starts a non-ephemeral thread (`ephemeral: false`) in the profile's `CODEX_HOME` | `thread/resume` by thread id, with the same read-only sandbox, approval policy and thread config. A missing or different thread is `RESUME_UNAVAILABLE`. |
| Claude Code | `persistent: true` drops `--no-session-persistence` and names the conversation with `--session-id <uuid>` in the profile's `CLAUDE_CONFIG_DIR` | `--resume <uuid>` (same id, never `--fork-session`), only after the stored `projects/*/<uuid>.jsonl` is found. |
| OpenCode (kit-managed server or configured endpoint) | `persistent: true` keeps the native session on suspend instead of deleting it | A new server looks the session up by id in the same directory and checks deny-all permissions again. |
| Muse Code | `persistent: true` drops `--no-session-log`, so `muse serve` keeps the session in Muse's own history | `session/resume` by session id with `excludeItems: true`, then `session/setApprovalMode` (and `session/setModel` if needed). An unknown session is `RESUME_UNAVAILABLE`. |
| ACP (native profiles) | The client's own session store, when it advertises `loadSession` | `session/load` (existing behavior; `persistent: true` is accepted only when `loadSession` is advertised). |
| API and local-runtime routes | none | `persistent: true` is `CAPABILITY_UNSUPPORTED`. |

`persistent: true` is refused with `CAPABILITY_UNSUPPORTED` unless the runtime has a state directory (below) and the opened adapter reports native resumption. A persistent session can be suspended only after a turn has completed, because before that no native history exists.

## Durable tokens

When the helper has a state directory, the handshake advertises `session.resume.persistent` and every execution route carries a `session.resume.persistent` capability (`supported`, `conditional` with condition `acp.loadSession`, or `unsupported`). `sessions.suspend` then returns a durable token:

- `lai1_` + base64url(nonce + ChaCha20-Poly1305 ciphertext) under a per-install 32-byte key, `resume.key`, created in the state directory on first use (user-only permissions on Unix; per-user app-data ACL on Windows).
- It carries no credentials. Sealing also keeps the native session id, account id and workspace path out of host storage. A modified token, or one from another installation, is `RESUME_UNAVAILABLE`.
- It is bound to the application identity (see below), connection and account, route, model id, options, workspace id and canonical path, permission mode, review/paid flags and billing classification.
- It survives helper restarts and is reusable: a host can store it with its thread and resume again later. Only one live session may be reattached to a conversation at a time (`CONFLICT`).
- Host MCP bearer tokens are never sealed. Pass `hostMcpServers` again in `sessions.resume`.

`sessions.resume` revalidates everything the token binds against the current catalog and grant before any native client is started. The same model id is required; a changed model revision is tolerated because catalog revisions change when clients update, and the current revision is used. Any mismatch, or native history that no longer exists, is `RESUME_UNAVAILABLE`. It never falls back to a fresh conversation, so the host can decide whether to start over, for example with its own summary. Transient failures (`AUTH_REQUIRED`, `RATE_LIMITED`, `TIMEOUT`, `PROVIDER_UNAVAILABLE`) keep their own codes.

Without a state directory the previous behavior is unchanged: 64-hex tokens live only in the helper's memory, are one-time and owner-scoped, are deleted on revocation, and stop working after a restart.

### State directory and application identity

- The CLI and native host use `LOCAL_AI_KIT_STATE_DIR` when set, otherwise `%LOCALAPPDATA%\originlm` (Windows), `~/Library/Application Support/originlm` (macOS), or `$XDG_STATE_HOME/originlm` (default `~/.local/state/originlm`). With `--home` it is `<home>/.originlm`. Embedded hosts call `KitBuilder::state_dir`, Rust hosts `build_service_with_state`. Nothing is created until a durable token is issued.
- Connection ids change on every pairing or restart, so durable tokens are bound to an application identity instead: `embedded:<label>` for `Kit::connect`, `http-host` for clients paired with an HTTP helper (one helper serves the one host that launched it), `ipc-host` for local IPC, `stdio-owner` for stdio, and the origin-derived id for the browser extension. Hosts using the service directly can call `NativeService::set_app_identity`.

## Public flow

1. Create a session (with `persistent: true` for restart-safe resumption) and use it.
2. When the session is idle or completed and `session.nativeResumption === true`, call `sessions.suspend({ sessionId })`.
3. The service asks the native driver to detach without closing or deleting the provider conversation.
4. The service returns an opaque `resumeToken`. It never returns the provider's native session id.
5. Call `sessions.resume({ resumeToken })` from the same application.
6. The service revalidates the token as described above and only then asks the same native route to reattach the original conversation.

## Safety properties

- Suspending an active turn is rejected.
- Suspending a closed session is rejected.
- Unsupported native drivers never expose a resume token.
- Resuming a changed account, route, billing classification, model, workspace path, or grant fails closed before the provider is reopened.
- A failed or invalid token does not start a new provider conversation as a fallback.
- Raw provider session ids and resume tokens never appear in diagnostics, events, or rendered demo content. The demo keeps the token only in private in-memory controller state until resume/revoke or page teardown.

The demo exposes **Suspend native session** and **Resume native session** only when those methods are negotiated and the current session declares native resumption support.

## Qualification

Fixture-tested: Codex `thread/start` non-ephemeral and `thread/resume` (including missing and different threads), Claude `--session-id` / `--resume` arguments, stored-conversation check and id continuity, OpenCode keep-on-suspend and reattach (including a missing session), ACP `loadSession`, and service-level restart persistence, identity-change refusal, tamper/foreign-key refusal and redaction. Live: `crates/originlm-kit/tests/live_persistent_resume.rs` (`LAI_LIVE_RESUME_CLAUDE=1`) ran one Claude Code Haiku turn, suspended, restarted the helper, resumed, and the second turn recalled a word from the first. Codex and OpenCode resume are fixture-tested only.
