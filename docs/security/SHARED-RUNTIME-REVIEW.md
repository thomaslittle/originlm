# Shared runtime security review

- **Scope:** broker lifetime, SQLite state, snapshot/event permissions, issue
  and notification data, control center, update verifier, consumer bridges.
- **Date:** 2026-10-22
- **Status:** implementation review with fixture and Windows platform evidence;
  not an external audit.

## Assets and boundaries

| Asset | Storage/owner | Boundary |
|---|---|---|
| Provider credentials | Native client / supported keyring | Never read into snapshots, SQLite, logs, exports, or the control center |
| Normalized snapshot | Broker memory + broker-owned SQLite | Per-client `ClientPolicy` projection at the service boundary |
| Inventory journal | Broker-owned SQLite | Same projection; cursor is opaque and epoch-bound |
| Issue evidence | Broker-owned SQLite + projected snapshot | Sanitized; no raw provider bodies or credential material |
| Notification delivery state | Broker-owned SQLite | Deduplicated; private-content mode omits issue text |
| Broker token | Parent descriptor, legacy owner token, or per-app grant manifest | Not argv, not logs, not snapshots; independent app tokens are app-scoped and broker-revocable |
| Native endpoint | Windows pipe owner-only DACL / Unix socket 0600 | One-use grant in parent mode; per-app reusable grant in independent mode when a manifest is supplied |
| Route billing | Adapter evidence + host declarations | OpenCode Go route class is derived/inferred, not user-supplied; entitlement, quota, and Console Extra usage remain separate unknowns |
| Update manifest/archive | Release pipeline + host-pinned key | Ed25519 publisher signature plus checksum; checksum alone is insufficient |

## Findings and controls

1. **Independent mode supports two credential shapes.** The legacy owner token
   is reusable and single-application; any process that can read the file can
   connect. A `--grants-file` manifest instead mints a distinct reusable token
   and stable identity per app, with per-app policy and execution scopes. The
   broker records the app identity, projects accounts/routes for that grant, and
   `apps.revoke` removes the app's grant and sweeps every live connection with
   that identity; the old token cannot reconnect. This is app isolation, not a
   defense against another process running as the same OS user that can read the
   token file. The admin credential is separate and should be control-center
   only.
2. **Windows token-file ACLs.** The descriptor is created and then restricted
   with `icacls /inheritance:r /grant:r <current-user>:F`; if that fails, the
   file is removed and the broker fails closed. The native endpoint remains
   owner-only. This is platform-executed by the independent-broker test on
   Windows. Unix files are written 0600 and the SDK rejects group/other access.
3. **Out-of-order publication.** Metadata base revisions are checked before
   commit. A stale probe cannot overwrite a newer inventory/identity state.
4. **Persistence sanitization.** Account labels and identity sources are
   replaced with `NOT_PERSISTED` before SQLite writes. Metadata reports are
   sanitized separately. Snapshot serialization is sanitized in the store, not
   only in the in-memory public projection.
5. **Corruption.** A failed integrity check preserves the database and WAL
   beside it, creates a new store, and records a visible storage issue. It does
   not silently delete account or issue state.
6. **Permission filtering on errors.** Access checks precede path lookup; issue
   listing and mutation use the projected snapshot; error messages are stable
   and do not echo adapter/OS paths or raw provider strings.
7. **Revocation.** `apps.revoke` checks the admin policy, refuses self-revoke,
   removes the broker grant first, sweeps every live app connection and its
   sessions/inputs/resume records, and invalidates subsequent calls and token
   reuse. Existing transports are not trusted to re-authorize themselves.
   Remotif's bridge tracks whether its broker worker is still alive before it
   serves a cached catalog; a revoked or disconnected app gets an explicit
   failure, not its pre-revocation view.
8. **Billing classification is not entitlement.** The exact OpenCode `opencode-go`
   route is classified as subscription with derived/inferred evidence because
   the official provider id denotes Go; Console `opencode` and lookalikes remain
   unknown. Quota, auth, and the Console Extra usage toggle stay separate. A Go
   route does not need a redundant billing declaration or generic paid-API
   opt-in, and unknown quota is not an access denial. Metered routes still
   require explicit paid consent; unknown billing is never unlocked by paid
   consent.
9. **Notification delivery.** Deduplicated decisions are computed without any
   desktop contact. A host must install a sink explicitly; the optional
   `os-notifications` build plus `--os-notifications` is the only real delivery
   path, and it was not enabled in qualification. Private-content preferences
   are applied before a decision reaches a sink.
10. **Notification privacy.** Private-content mode replaces title/body; quiet
    hours suppress delivery; snooze does not resolve; inline health remains.
11. **Control center.** It uses only the authenticated JSON-RPC API and never
    opens SQLite. Export redacts secret-shaped keys recursively, though the
    protocol already never returns provider tokens. Keyboard confirmation keeps
    `Esc` canceling a destructive prompt instead of quitting, digit tab changes
    reset stale selections, and non-selectable reports can scroll.
12. **Update trust.** Signature verification is mandatory before staging.
    Activation refuses while a session marker/lease is present, uses exclusive
    locks, backs up the current binary, and preserves the failed version.
    Signing/publication/installation remain approval gates.
13. **Threat-model limits.** The runtime is not a defense against malware that
    already controls the user's OS account. CORS is not authentication.
    Loopback HTTP still requires pairing and per-client grants.

## Required follow-ups before a production claim

- Re-verify the owner-only Windows DACL on a second OS account and on a clean
  machine; the current independent-broker test runs as one local user.
- Cross-platform qualification of app-scoped grant revocation, stale-cache
  refusal, and route-billing tests on Linux and macOS.
- External security review of the update signer/publisher process and the
  independent token distribution.
- Live-account qualification for each provider remains separate from this
  review. No Go subscription entitlement, quota, or Extra usage state was
  verified with a live account.
