# Incremental metadata, usage, and backoff

OriginLM separates the mechanics for reading changing local evidence from the provider-specific interpretation of that evidence.

## Bounded log/activity tailing

`originlm_discovery::log_tail::read_lines` is a provider-agnostic incremental newline-record reader. No current provider adapter uses it: Codex and Claude usage data come from their structured native interfaces, and no undocumented activity/log source is inferred. A future adapter may use the primitive only after identifying a specific nonsecret file and understanding its schema.

The caller owns the state; it stores an offset, a platform file identity and birth time when available, at most 1 MiB of buffered records, a leading fingerprint of up to 64 consumed bytes, and a 64-byte anchor immediately before the current offset. It:

- reads a data window of at most 1 MiB plus at most 194 bytes of bounded fingerprint checks per call;
- preserves partial records until a newline arrives;
- limits returned records to 4,096 per call;
- detects truncation when the current file is shorter than the saved offset;
- detects replacement/rotation by comparing file identity/birth time and both byte fingerprints, including inode reuse when the leading bytes change but the previous offset anchor is identical;
- restarts from byte zero after truncation/rotation without replaying records from the previous file;
- serves already-buffered complete records before reading more bytes;
- checks caller cancellation before each bounded filesystem operation and leaves saved state unchanged when cancelled;
- fails closed when one record exceeds the bounded buffer.
- reads only a single explicitly supplied path; it does not traverse a home directory, and the native filesystem rejects dot segments and symlink/reparse-point components anywhere in the file path.

Tests include append/partial records, truncation, replacement rotation with matching anchor bytes, cancellation, symlink rejection, line-window paging, an intentionally sparse 1 GiB file, and oversized records. The sparse-file test verifies that only a few kilobytes are actually read.

The returned `rotated`/`truncated` flags let an adapter reset source-specific accounting. The reader does not count usage or deduplicate logical records copied between files. A filesystem that reuses identity without distinct birth time, combined with identical bounded fingerprints, cannot be distinguished from an append without a stronger source-specific generation marker; no full-file scan is performed to hide that limitation.

The toolkit does **not** infer quota, token cost, or plan access merely because a log exists. Codex/Claude quota data currently comes from structured native interfaces. A future adapter can connect a verified log schema to normalized usage records without changing the tailer.

## Metadata refresh and failure backoff

Connection probes remain single-flight per connection. Successful metadata is cached and field evidence becomes stale rather than silently refreshed in the background.

Usage and reset observations are polled on a cadence shorter than their 60-second freshness window, including while no client interest is active. Interest changes metadata/inventory cadence only; it cannot let a monitor wake to an expired usage window while waiting for a slower idle poll.

Retryable failures use bounded exponential backoff starting near 250 ms, capped at 30 seconds, with deterministic per-connection jitter. An upstream `Retry-After` value takes precedence and is bounded to 24 hours. Nonretryable failures retain a short negative cache to prevent tight loops.

Only safe retry information crosses the public API:

- `retryable: true|false`
- optional `retryAfterMs`

Provider response bodies, URLs, headers, tokens, local paths, and raw error messages are discarded before caching/serialization. TypeScript and Python SDK errors expose the same safe fields and never retry a request automatically. In particular, a generation/turn request is never replayed merely because an error is retryable.

OS metadata notifications invalidate cached evidence only. They do not launch a client, probe quota, or run inference; the host still explicitly chooses when to refresh.
