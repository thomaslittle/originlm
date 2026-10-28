# ChatGPT managed authentication

The opt-in `codex_managed` connection uses OpenAI's public Sign in with ChatGPT
flow and a dedicated Codex app-server profile. It is separate from an installed
Codex client's existing login. A native Codex executable (0.157 or newer) is
required; no API key or developer client secret is required.

```sh
originlm serve --stdio --allow-passive-discovery \
  --executable-only-provider codex \
  --chatgpt-app-name my_app --allow-metadata codex_managed
```

Use your application's stable name. The same name is sent in
the first OAuth registration and Codex `initialize.clientInfo.name`. The helper
creates one managed connection from a detected Codex executable. With
`--executable-only-provider codex`, the executable stays a private candidate
for that managed connection: passive discovery does not inspect Codex profile
roots or expose a separate legacy Codex provider, profile, connection, or route.
The option accepts any known discovery provider ID, may be repeated, and cannot
be combined with a metadata or execution grant or explicit registration for
the same provider. Omit it to retain the default passive discovery behavior.
Startup and
passive discovery do not read credentials or contact the OAuth service.

Every `auth.chatgpt.*` RPC requires the host's `codex_managed` metadata grant.
An opted-in helper advertises the `auth.chatgpt` capability:

| Method | Parameters | Result |
| --- | --- | --- |
| `auth.chatgpt.status` | `{}` | `state`, `loginId`, `email`, `errorCode` |
| `auth.chatgpt.start` | `{}` | `loginId`, `authUrl` |
| `auth.chatgpt.cancel` | `{ "loginId": "…" }` | `{}` |
| `auth.chatgpt.disconnect` | `{}` | `{}` |

Open `authUrl` in the system browser and poll status while `state` is `pending`.
The helper binds the IPv4 loopback listener before returning the URL. It uses
PKCE S256, a fresh state and nonce, validates the ID token against OpenAI's JWKS,
and requires the exact `chatgpt.tokens.use.direct` scope. Issued registration,
verified account identity and host ID persist across reconnects. The host must
keep the helper alive while sign-in is pending; killing it closes the callback
listener. Callback URLs, authorization codes and tokens belong in the native
host and should never be logged or passed into application UI state.

Credentials live under `LOCAL_AI_KIT_STATE_DIR/chatgpt-auth`. Windows stores a
random encryption key in Windows Credential Manager and atomically replaces an
authenticated encrypted credential file. Unix uses owner-only state files and
directories. Refresh uses a per-store OS file lock, rereads the latest tokens,
and rotates the complete token record atomically. Disconnect first persists an
inactive credential marker, then clears local credentials and closes managed
sessions before attempting refresh-token revocation. Remote revocation has a
ten-second budget; network failure does not restore the local session. The
registration remains for returning sign-in. A persisted epoch prevents an older
callback from undoing a disconnect in another helper process. Status rereads the
atomic records to reflect another process's rotation or disconnect. Preserve this
state directory when upgrading the helper.

An explicit `connections.probe` obtains the managed model catalog. Plan and
quota fields remain unknown; a catalog is not an entitlement guarantee. Actual
turns additionally require `--allow-execution codex_managed --workspace` with an
absolute workspace. The adapter keeps the existing tool, host MCP, image,
cancellation and resume policies. Only the child process receives `ACCESS_TOKEN`;
Codex uses an isolated `chatgpt-codex-home` and the documented Responses provider
at `https://api.openai.com/v1`. Before each turn, the adapter renews credentials
when the access token is inside 60 seconds of expiry. A token response field
`earliest_refresh_at` (unix seconds, or an RFC3339 timestamp) holds that
renewal back: a still-valid access token is reused, and an already-expired
token fails with `REFRESH_DEFERRED` instead of racing the provider. ID tokens
may use RS256 or P-256 ES256, with five seconds of clock skew. If sign-in
returns tokens without `chatgpt.tokens.use.direct`, the registration is kept
and the next attempt sends `prompt=consent`. A different account on the same
registration is still rejected. A replacement access token starts a new app-server and resumes the
same native thread. An uncertain turn is never automatically replayed.

OAuth and execution tests use synthetic signed-token and native-peer fixtures.
They do not authorize a real account or qualify live plan inference. See
[OpenAI's sign-in specification](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
and [Codex app-server integration](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server).
