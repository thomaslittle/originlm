# OpenCode gateway

The adapter uses the public local OpenCode HTTP API. It never reads OpenCode's credential files. Explicit metadata permission permits `/provider`; the parser includes only connected upstreams and drops keys, headers, raw options, and endpoint URLs. Native model IDs are retained under distinct upstream route IDs. Advertised variants and context bounds become native option descriptors and evidence-backed fields.

The standard gateway response does not verify a subscription plan or publish account allowance. These remain unknown. Optional `providerBilling` declarations are host-supplied and labeled as such; catalog prices are not treated as proof of subscription access.

OpenCode Go is the exception in route classification, not entitlement: official OpenCode docs publish it under the exact provider id `opencode-go`, separate from the Console pay-as-you-go provider `opencode`. The adapter therefore records the `opencode-go` route as a subscription **class** with `derived`/`inferred` evidence before consulting declarations. It does not claim that entitlement, quota, or zero overage was verified. Nonzero token prices in Go's catalog measure allowance consumption and are not evidence of PAYG. Unknown or missing quota remains unknown rather than an access denial. Console `opencode` routes keep their existing billing evidence/declaration requirement, and the provider's Console Extra usage toggle remains an unverified separate state; no fallback to another route or account is automatic.

```json
{"endpoints":[{"id":"gateway","label":"OpenCode","kind":"opencode","baseUrl":"http://127.0.0.1:4096/","providerBilling":{"openai":"subscription"}}]}
```

Use `--endpoints ABSOLUTE_JSON --allow-metadata opencode` to point the kit at a server you run yourself.

## Kit-managed server (discovered installs)

Without an explicit endpoint entry, a discovered OpenCode install is driven the way T3 Code drives it: for each metadata probe and each text session the kit starts its own `opencode serve --pure --hostname=127.0.0.1 --port=<free port>` with a fresh random `OPENCODE_SERVER_PASSWORD`, talks to it over HTTP Basic auth (user `opencode`), and kills the whole process tree when the probe or session ends. The npm `.cmd`/`.ps1` shim is never executed; it is resolved to the packaged `opencode-ai/bin/opencode.exe`. `--pure` disables external plugins. The server inherits only the kit's scrubbed environment, so upstream credentials must live in OpenCode's own `auth.json` (as `opencode auth login` stores them), not in ad-hoc environment variables. No default port is probed any more.

Billing for managed routes is unknown (OpenCode does not report it), so selection refuses them with `BILLING_CONSENT_REQUIRED` until a host declares billing through an explicit endpoint entry's `providerBilling` **or the route is the exact `opencode-go` subscription class**. The Go route class does not require a declaration; its quota and Console Extra usage remain unknown separate states. A separately configured `keyEnv` supplies a password for the gateway's HTTP Basic authentication with username `opencode`. API keys for upstream providers remain owned by OpenCode.

## Text sessions

`--allow-execution opencode --workspace ABSOLUTE_DIRECTORY` enables explicit sessions through the common SDK. The host preserves the selected upstream provider ID independently from the native model ID. Only text-only sessions are exposed. The adapter creates its own native session with a deny-all tool permission ruleset and verifies that the server echoes the restriction **before sending a prompt**. It refuses write/review modes rather than claiming an OS sandbox. It does not change the gateway's global configuration.

The adapter subscribes to the native event stream before posting `prompt_async`, filters events to the owned session and assistant messages, preserves incremental text, and reconciles the final message without duplicating output. Errors and premature stream closure fail explicitly. Cancellation requests abort only that owned native session. Explicit close deletes that session, not the user's existing conversations. No persisted cross-process resumption is claimed. A forcibly killed helper may leave an inert native session record on the gateway.

Gateway authentication is not independently verified identity for every upstream account. Account changes inside the gateway need native metadata/protocol evidence; the toolkit does not infer account IDs from an email or hidden credential. Prefer a separately managed server/profile for strict isolation.

## Sources and evidence

- https://opencode.ai/docs/server/
- https://opencode.ai/docs/permissions/
- `anomalyco/opencode` SDK v2 schema revision `0f549842ee746e400b1f72516b0b2e292e267e2c` (`SessionCreateData`, `SessionPromptData`, `PermissionRuleset`, `Provider`, and streaming event records).

September 28 (OpenCode 1.18.32, Windows): the managed server probe returned 111 connected-provider models live. One live text turn on a free Zen model (`longcat-2.5-preview-free`) ended in a `session.error` after ~75 s; the adapter then dropped OpenCode's reason, and now surfaces it (redacted). OpenCode is therefore not live-proven for text turns. `crates/originlm-adapters/tests/opencode_live.rs` repeats the check when `LAI_LIVE_OPENCODE` and `LAI_LIVE_OPENCODE_MODEL` are set.

Four gateway catalog tests and two actual mock-server/runtime session tests passed after observed red tests. These verify parsing, routing, permission rejection, streaming, and owned cleanup; they do not certify a live upstream subscription. No live inference was submitted.

## Persistent resumption

Persistent sessions and durable resume tokens are described in [native session resumption](../integration/native-session-resumption.md).

## OpenCode 2 adapter staging (October 2026)

The independent `opencode::client_v2` and `opencode::catalog_v2` modules model
OpenCode 2.0.23's public protocol, compared with T3 Code source
`ca952677321abf9f3762ae4bb0129454e5a63955` and the official
`@opencode/protocol`/`@opencode/schema` 2.0.23 packages (MIT). No upstream
implementation was copied wholesale. They are **not wired into automatic
managed-client discovery or execution**. `server::start` still fails closed
on v2 rather than running it with v1 flags.

The v2 catalog preserves gateway owner/upstream/native-model identity and
reported capability/variant metadata. It deliberately reports unknown auth:
`/api/integration` projects configured credentials and process-local errors;
absence of `needs_auth` is not positive authentication evidence. Explicit
billing declarations do not change that state or bypass selection guards.

The v2 transport accepts an explicitly supplied endpoint, verifies returned
model/workspace/deny-all permissions, subscribes before prompting, reconciles
text deltas with final blocks, rejects native tool requests, and cleans up
only the owned session. Images and native resume are not advertised. Its
loopback fixture enforces synthetic Basic Auth, selected model and no-tool
permissions and covers streaming/final-only/duplicate replies, missing or
mismatched text, errors, disconnected streams, cancellation, foreign-session
events, and rejected model/workspace/permission echoes.

Automatic startup remains blocked: the v2 CLI lacks the v1 `--pure` contract.
Project-config disabling and a temporary config directory still leave global
compatibility and well-known configuration sources. A later `plugins: ["-*"]`
directive is insufficient: the upstream plugin supervisor loads add
operations before processing the removal. Deny-all session rules do not
prevent startup module side effects. Do not enable v2 startup until a
supported isolation path is independently verified; do not change user
configuration, copy credentials, downgrade clients, or infer authentication
to bypass this gate.

Qualification: 12 synthetic v2 transport/catalog tests and 12 runtime
cleanup/lifecycle tests pass on Linux, Rust 1.97.1. No live account, installed OpenCode binary, provider inference,
Windows/macOS execution, release artifact, or downstream helper pin is qualified
by these tests.

### Review hardening

Text-block start markers are tracked even when no text arrives. New text
ordinals must increase within one assistant message (gaps may represent
reasoning blocks), and an earlier assistant message cannot re-enter later
output. Assistant IDs observed on prior turns cannot be reused as fresh
text. Known durable execution/text boundaries validate the session aggregate
and monotonic durable sequence; an old terminal cannot complete a new turn. UTF-8 event chunks stay at or below 8 KiB without splitting a character.

A failed or cancelled v2 DELETE retains the owned session ID and prevents
further turns. The runtime keeps that adapter for explicit close retries,
returns cleanup errors/timeouts, and does not prune closed records while
cleanup is pending. Failed-turn cleanup follows the same retention rule.
These are retry guarantees while the runtime and owner remain present;
revocation, runtime destruction, process failure, or an abandoned caller
can still leave an external server record. No reliable remote deletion is
claimed outside the owned-server lifecycle. Unsupported v2 managed launch
and unknown authentication remain unchanged.

For a lost successful DELETE response, a retry may complete only on an
authenticated `application/json` HTTP 404 whose bounded body carries the
official `SessionNotFoundError` tag and this exact owned `sessionID`.
Wrong IDs/tags, unauthenticated responses, HTML, 403 and oversized bodies
remain uncertain failures. Legacy deletion behavior is unchanged. Cleanup
of an open that fails validation remains best-effort because no usable
adapter is returned; this staging does not promise external-record cleanup.
