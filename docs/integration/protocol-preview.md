# Implemented SDK wire subset (protocol 1.0)

The canonical domain records are in Rust. This document describes the currently tested transport envelope and is not a complete generated OpenAPI contract.

Every request: `{ "jsonrpc": "2.0", "id": 1, "method": "snapshot.get", "params": {} }`.
Every response uses the same ID and exactly one of `result` or `error`.
Error: `{ "code": -32000, "message": "safe description", "data": { "code": "AUTH_REQUIRED", "retryable": false } }`.

Handshake: `runtime.handshake` with `protocolVersion: "1.0"` and `clientName`. Result has `protocolVersion` and `methods: string[]`. A different major fails closed. Handshake never grants access or triggers discovery.

HTTP facade: POST `/v1/rpc` for the same JSON-RPC envelope. All requests use header-based authentication and per-client grants. This facade complements the planned generated REST routes; it does not replace them. GET `/v1/events?sessionId=...&after=...` is a bounded authenticated SSE stream. No token in query strings. `event: gap` produces `EVENT_GAP`; clients never silently replay a turn.

Stdio: newline-delimited UTF-8 frames; stdout is protocol-only. `events.subscribe` returns `{subscriptionId}`. Notifications are `{jsonrpc:"2.0",method:"events.event",params:{subscriptionId,event}}`. The subscribe response precedes its events, but both may share one stdout chunk. `events.unsubscribe` takes the same subscriptionId. Unknown notifications fail closed in this preview until negotiated.

Transport cancellation does not guarantee reversal or cancellation of a submitted side effect. Explicitly cancel native turns with `turns.cancel`. No client automatically retries an ambiguous turn or swaps account/billing routes.
