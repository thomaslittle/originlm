# ADR-001: One durable, permissioned local AI runtime

- **Status:** accepted and implemented; see the [shared broker guide](../integration/shared-broker.md) for current integration behavior.
- **Date:** 2026-10-22
- **Supersedes:** the implicit "one broker, metadata JSON cache" behavior
  described in earlier handoffs. It does not replace the public contracts.

## Context

OriginLM already had a Rust discovery engine, a shared broker, scoped
client policies, single-flight discovery, per-connection metadata caching,
backoff, and file-watch invalidation. Inspection found five integration gaps:

1. A broker started for one parent stopped when that parent closed its control
   input, so independently launched applications could not reliably converge.
2. Filesystem invalidation removed metadata but did not schedule a re-read, so
   the cache could remain empty until a UI request.
3. Metadata publication did not advance the inventory revision or emit a
   shared change record; each consumer had to re-read the entire snapshot.
4. Warm startup restored per-connection metadata but not the ready-to-serve
   snapshot, health, issues, or notification delivery state.
5. There was no persistent, actionable health model, no single notification
   coordinator, and no broker-owned upgrade/rollback path.

The provider remains authoritative for authentication, entitlement, quota, and
billing. This runtime is authoritative only for its own observations.

## Decisions

### D1. Keep the Rust core and add broker-owned SQLite

`originlm-store` is a new crate used only by `originlm-service`. It commits normalized,
sanitized observations, the last ready snapshot, monotonic revisions, a bounded
journal, issue state, and delivery deduplication state in one SQLite file with
WAL. Clients never open SQL. The database is local disk state, not a network
share. A damaged database is renamed to `state.corrupt-<timestamp>` and a
visible recovery issue is raised; it is never silently erased. A future schema
fails closed.

Metadata is sanitized before persistence: account labels and identity sources
become `NOT_PERSISTED`. Raw provider tokens are never stored, logged, or
exported; provider credentials stay with native clients or supported secure
storage.

### D2. Explicit broker lifetime, never an accidental second scanner

`serve --ipc` remains parent-owned and one-use by default. A second explicit
mode, `serve --ipc ADDRESS --independent --token-file PATH`, owns its lifetime,
publishes a reusable owner-only token descriptor, and supports an optional idle
timeout. Endpoint binding is the single-instance race boundary: a competing
launch fails with `CONFLICT` and must connect to the existing broker. Closing a
consumer closes only its bridge.

The SDK helper `openSharedBrokerTransport` reads the descriptor, tries the live
broker first, and starts it only when absent. If a shared broker is configured
but unreachable, consumers must report that failure; they must not silently
start a managed full scanner. Embedded and managed-helper modes remain for
compatibility.

Independent mode has two credential shapes. A legacy owner-scoped reusable
`--token-file` is the single-application compatibility boundary. For multiple
applications, `--grants-file` names each app id, its scoped policy/execution
grant, and its own owner-only token file; the broker mints a distinct reusable
credential per app and records the app identity on the connection. The optional
admin credential's `apps.revoke` removes that app's grant and every live
connection with that identity, so a token cannot reconnect. Token files remain
the OS-account boundary: any process that can read an app's file can use that
app's grant. Cross-app identity, account projection, and revocation are enforced
by the broker, not by a shared owner token. See
[shared-broker.md](../integration/shared-broker.md) for the manifest shape.

### D3. Revisions, epochs, and a bounded journal

Every committed inventory change gets a monotonic revision and a
`<epoch>:<sequence>` cursor. The epoch changes on a fresh database process
lifetime. `snapshot.get` returns the committed cursor. Clients subscribe with
`events.read({"scope":"inventory","after":cursor})`; the transport keeps the
same subscription framing used by session events. A cursor from a previous
epoch, ahead of the tail, or before the bounded journal produces a typed
`EVENT_GAP`; the only recovery is a full snapshot resynchronization. Events are
published after commit, never before.

### D4. Three refresh lanes and stale-while-revalidate

- Host-opted-in passive inventory reconciliation (`discovery.refresh` shares its
  single flight), every 5 minutes with active read/activity interest and every
  30 minutes idle. CLI authorization is `--allow-passive-discovery`, independent
  of provider metadata grants.
- Targeted metadata/auth refresh after a debounced local change, an explicit
  probe, or expiry. The autonomous lane probes only host-authorized providers,
  coalesces dirty connections, and uses bounded concurrency and backoff.
- Slow metadata expires for scheduling at 15 minutes active / 60 minutes idle.
  Provider-supported usage has its own 60-second active / 5-minute idle deadline.
  A 120-second authenticated interest lease selects cadence; open idle sockets
  do not count as active. Invalidation remains eligible at each 5-second tick.
- Usage may use the same full native report where no usage-only operation
  exists; one successful report satisfies both logical lanes. Unknown usage
  does not create fast polling. Provider reset timestamps become monotonic
  recheck deadlines. A timer never declares a reset or clears an exhausted
  allowance; stale warnings require provider recovery evidence.
- Admission rotates across connections and alternates metadata/usage priority,
  with 64-item pages, two shared probe slots, one inventory flight, shared
  jitter/backoff/Retry-After, and cancellation scoped to owned work. A canceled
  worker retains its flight until it unwinds; restart retries promptly and
  foreground joiners retain ownership.

Current Codex, Claude and Muse adapters report connection-scoped quota pools
with unknown cross-profile sharing. Auth/catalog/usage operations are also
profile-bound. Coalescing therefore covers all verified routes/pools and apps
within that exact connection; it never merges different profiles from an email
or matching display label. Verified cross-connection quota operations are not
provided by these adapters. The behavioral duplicate-route test and adapter
source paths are recorded in the refresh-lane qualification evidence.

Inventory and per-connection generations guard both successful and failed
metadata publication atomically. Unrelated account reports can commit in
parallel; a newer scan or invalidation rejects older results. Public journal
pages retain currently scoped IDs, permitted connection removals, or anonymous
invalidation hints for historical resources whose scope cannot be reverified.
Clients resnapshot on hints; no historical private resource ID is exposed.

A failed provider does not block another provider or erase last-known data.
File invalidation marks data stale immediately and schedules a re-read; it does
not claim a logout. A failed probe keeps the last observation visible as stale
with a typed refresh error. Persisted startup data is always labeled stale and
reverified, never treated as a current session or credential.

### D5. Typed health, one notification coordinator

`Issue` is persistent and stable across restarts: kind, severity, state,
affected provider/connection/account/route/pool/window, first/last observation,
evidence, remediation, snooze, and resolution. Dismissal and snooze do not mark
an issue resolved; resolution requires a later verified observation. Transient
failures are debounced and failures remain inline even when OS notifications are
disabled.

`notifications.poll` is the only delivery decision authority. It applies
severity, enablement, quiet hours, privacy, snooze, reminders, and durable
deduplication. Crash recovery cannot guarantee exactly-once OS delivery; the
contract says best-effort with a stable dedupe key.

### D6. Control center is a native Rust TUI over the public API

The repository has no existing desktop UI shell; adding a GPUI/egui stack just
for a reference consumer would couple the core to a new UI dependency. A
lightweight Rust terminal UI (`originlm-control`) consumes the authenticated broker
protocol only: snapshot, health, apps, adapters, refresh, probe, issue
acknowledge/snooze, revoke, and redacted export. It never opens SQLite and never
calls a provider. The UI is optional; the broker runs headless. Tray/OS
notification integration belongs to a host application using the coordinator.

### D7. Upstream knowledge is still normalized, reviewed evidence

Sources have separate responsibilities: official provider protocols for
authoritative semantics; T3 for executable resolution/platform environment;
Tokscale for storage paths/migrations; OpenUsage for additional path knowledge
after review; OpenCode for local model-server discovery; Quotio only for
verified, appropriately licensed complementary adapters. Released builds never
clone or execute upstream code. Upstream reports are artifacts/branches, not
automatic adapter updates. Provider-relevant changes are classified separately
from release/relay/docs noise, and lost watched coverage is an explicit failure.

### D8. Updates are signed, staged, and coordinated

Checksums are integrity, not publisher authentication. `originlm-update` verifies an
Ed25519 signature over canonical manifest JSON, platform, protocol family,
version monotonicity, archive size/checksum, and staged bytes. Activation takes
an exclusive session lease and an activation lock, backs up the previous
binary, replaces atomically, and supports explicit rollback. Signing
credentials, publication, and installation are separate approval gates.

## Consequences

- Consumers get a fast cached snapshot immediately, then a small change signal,
  then a full snapshot only when they choose or when a cursor gap requires it.
- A database migration is now a release concern; schema versions fail closed.
- Independent mode's legacy owner token is weaker than per-app grants; hosts
  that need distinct identities should use `--grants-file` in independent mode
  or the parent/embedded broker. This is documented, not hidden.
- The control center is intentionally small. A richer native UI can be added
  later without changing the broker contract.
- Live provider qualification is still separate from fixture tests and platform
  execution. No claim in this ADR turns a fixture into a live account.

## Upgrade strategy

1. Protocol 1.x is additive for clients. New methods/fields are advertised by
   `runtime.handshake`; unknown methods return `CAPABILITY_UNSUPPORTED`.
2. SQLite schema versions are integer-gated. A newer store is not opened by an
   older runtime. Migrations run in one transaction and preserve a backup.
3. A replacement executable is installed only after signature/checksum/platform
   checks and while no session lease is held. The previous binary remains in
   `backups/` for explicit rollback.
4. Consumers may pin a known-good helper or connect to a shared broker; a
   provider-adapter fix does not require every consumer to ship at once.
