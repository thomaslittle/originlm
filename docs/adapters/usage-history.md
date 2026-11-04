# Session-history usage totals

`originlm usage --json [--home ABSOLUTE_DIRECTORY]` reports token totals from
the local Claude Code and Codex session stores. It is an explicit, bounded,
read-only command. It does not run at import time, does not start a client, and
never reads credentials, prompts, or tool output.

## Sources

| Provider | Roots | Format |
| --- | --- | --- |
| Claude Code | `~/.claude/projects`, `~/.claude/transcripts` | `*.jsonl` assistant messages with `message.usage` |
| Codex | `~/.codex/sessions`, `~/.codex/archived_sessions` | `*.jsonl` `session_meta`, `turn_context`, `token_count` |

Compressed `.jsonl.zst` sessions are skipped and reported as
`COMPRESSED_SESSION_SKIPPED` rather than silently omitted. Files over 64 MiB,
total input over 512 MiB, more than 4096 files, or more than one line over
1 MiB are reported through `filesSkipped`, `truncated`, or `malformedLines`.

## Normalized fields

- `input_tokens` is fresh input. Claude reports it that way. Codex includes
  cache reads and cache writes inside `inputTokens`, so both are subtracted
  and kept separately.
- `cached_input_tokens` is cache-read input.
- `cache_creation_tokens` is cache-write input.
- `output_tokens` is provider-reported output.
- `reasoning_tokens` is a subset of output and is never added to it.

## Deduplication

- Claude writes the same `requestId:message.id` several times while streaming.
  Each field keeps its maximum. When one snapshot reports the cache split and
  another reports a whole-prompt input count, the split snapshot supersedes the
  whole-prompt `input` value instead of comparing incompatible units.
- Codex `total_token_usage` is thread-wide. Events are sorted and a turn's
  delta is `total - previous total`, with `last_token_usage` as the reset
  fallback. Identical events from a resumed copy are deduplicated by
  `(session, timestamp, total, last)`. A counter decrease is a reset, not a
  negative total.
- Mixed-model Claude and Codex sessions produce one `models[]` row per native
  model id. No alias or family mapping is applied.

## Limits

This command reports only what the selected roots contain. It does not infer a
provider when no root exists, does not estimate missing tokens, and does not
roll totals up by day. The public service and desktop surface do not consume it
yet; `usage.read` remains the quota-window API.
