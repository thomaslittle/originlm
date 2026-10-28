# Cursor metadata and opt-in allowances

Cursor is read through its own installed **`cursor-agent`** CLI, plus the
editor's on-disk `User/globalStorage/state.vscdb` cache. No browser cookie, no
API-key conversion and no execution are involved.

## What the CLI reports

Discovery provides the absolute `cursor-agent` path; the adapter never searches
`PATH` itself and never attributes a generic `agent` binary to Cursor. With
`--allow-metadata cursor` an explicit probe runs:

| Command | Used for |
| --- | --- |
| `cursor-agent status` | Signed-in state and account label (`✓ Logged in as …`) |
| `cursor-agent about` | Subscription tier (plan) |
| `cursor-agent models` | Provider-native model ids and display names |
| `cursor-agent --version` | Client version evidence |

A missing, failing or signed-out CLI never becomes a sign-out claim: the probe
falls back to the cache reader and marks the result `CURSOR_CLI_NOT_SIGNED_IN`.
Models carry native ids only — the CLI reports no capabilities, context window
or option descriptors, so those stay explicitly unknown instead of being guessed.

## Allowances (`--allow-usage cursor`)

The CLI does not expose allowance percentages. Cursor's dashboard does, through
`DashboardService/GetCurrentPeriodUsage` and `GetPlanInfo` on
`api2.cursor.sh`. Those calls need the session token Cursor stores as
`cursorAuth/accessToken` in the same database.

That read is a separate, explicit grant: the broker must be started with both
`--allow-metadata cursor` and `--allow-usage cursor`. The adapter then:

- reads the token directly into a zeroizing buffer, bounded to 8 KiB, and only
  accepts a three-segment JWT;
- uses it for those two requests and drops it;
- never logs, serializes, exports or writes the token, and never returns it in
  metadata, diagnostics, snapshots or exports.

The returned percentage is stored as a single `percent` window labelled
"Included usage", with the reported reset time. If Cursor reports a non-zero
separate API percentage, that becomes an "API usage" window in the same pool
instead of being summed into the total.

Evidence kind is `community_endpoint` for the allowance request because the
dashboard service is undocumented; the response is still Cursor-reported, never
inferred. Without the grant the connection keeps its cached identity/plan and
the allowance stays `CURSOR_QUOTA_NOT_READ`.

## Cache fallback

`read_cached` reads only `cursorAuth/cachedEmail`, `cursorAuth/stripeMembershipType`
and the account id from the immutable, read-only SQLite URI; it does not write
sidecars. Keys follow CodeNotch's implementation at `vinzdg/codenotch`, commit
`642d329c52ba127adec6eb04e6e2614495b47f19`, `windows/codenotch/src/cursor.rs`
and `Sources/Providers/CursorCredentials.swift`.

The cache may omit newer WAL data, so its records are always marked stale.
Cached plan presence never becomes authentication or billing proof. Identity is
masked by the service unless the embedding host separately grants identity
visibility.

File size, SQLite value size, query text and busy timeout are bounded; the query
plan must use an index on the exact key. Native database corruption or absence
returns a sanitized error.

## Limits

Text execution, image input, approvals and session resumption are not
implemented for Cursor. Applications can use separately configured API routes,
but that is not a conversion of a Cursor subscription. Tests cover the cache
reader, CLI parsers (status, models, tier) and the percent mapping; the
dashboard request is fixture-shaped, not live-qualified on every plan.
