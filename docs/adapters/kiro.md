# Kiro CLI

Kiro is detected from its official CLI, `kiro-cli`. The passive profile is
`~/.kiro`; session files live under `sessions/cli` and are not opened during a
scan. The binary is looked up on `PATH` and in the platform fallback
directories, so a desktop app launched with a minimal `PATH` still finds it.

## Metadata

A metadata probe runs three client commands:

- `kiro-cli --version` — the client version.
- `kiro-cli whoami --format json` — sign-in state. A non-zero exit with
  `{"account":null}` is an explicit signed-out observation; any other failure
  stays unknown. `KIRO_API_KEY` is passed to the child when set and is never
  serialized or logged.
- `kiro-cli chat --list-models --format json` — the account's model catalog.
  This runs only after `whoami` confirms a session, because a signed-out
  request starts Kiro's browser login.

The catalog keeps Kiro's `model_id` as the native id and `model_name` as the
display name. `default_model` marks the default; no model is invented when the
list is unavailable. Reasoning-effort controls are not exposed because the
model list does not report them; T3 Code's per-model effort table is not
adopted as provider evidence.

Plan, billing, quota, and token usage are not reported by these commands and
stay unknown. Kiro execution is not implemented. OriginLM never reads Kiro
credential files.

## Sources

- T3 Code `packages/provider-kiro/src/server/status.ts` at
  `8b44f42b14c8e273156474a924ccf29680dd7857` (MIT), reimplemented in Rust.
- Kiro CLI documentation: <https://kiro.dev/docs/cli/>.
