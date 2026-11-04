# Session-history usage totals

`originlm usage --json [--home ABSOLUTE_DIRECTORY]` reports token totals from
the local Claude Code, Codex, Gemini CLI, Grok, and Kiro session stores. The
authenticated broker also serves the same report as `usage.history`, and the
desktop Costs page shows the provider totals from `costs.read.historical`. It is an explicit, bounded,
read-only command. It does not run at import time, does not start a client, and
never reads credentials, prompts, or tool output. The JSONL records are
deserialized into usage-only structs, so `content`, tool results, and
attachments are not materialized.

## Sources

| Provider | Roots | Format |
| --- | --- | --- |
| Claude Code | `~/.claude/projects`, `~/.claude/transcripts` | `*.jsonl` assistant messages with `message.usage` |
| Codex | `~/.codex/sessions`, `~/.codex/archived_sessions` | `*.jsonl` `session_meta`, `turn_context`, `token_count` |
| Gemini CLI | `~/.gemini/tmp` (or `GEMINI_CLI_HOME`) | `session-*.json` chat recordings and headless `*.jsonl` token events |
| Grok | `~/.grok/sessions` (or `GROK_HOME`) | `updates.jsonl` `turn_completed` usage |
| Kiro | `~/.kiro/sessions/cli` | session JSON `user_turn_metadatas` recorded token counts |

Compressed `.jsonl.zst` sessions are skipped and reported as
`COMPRESSED_SESSION_SKIPPED` rather than silently omitted. Files over 64 MiB,
total input over 512 MiB, more than 4096 files, or more than one line over
1 MiB are reported through `filesSkipped`, `truncated`, or `malformedLines`.

## Normalized fields

- `input_tokens` is fresh input. Claude reports it that way. Codex defines
  `inputTokens` as the whole prompt, including cache reads **and** cache writes
  (`codex-acp` `TokenCount`, `openai/codex` `TokenUsage`); both are subtracted
  here and kept separately. Tokscale and Codenotch currently ignore
  `cache_write_input_tokens`, so their reported input is larger when that field
  is present.
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

## Cross-check against Tokscale

A shared synthetic home with two Claude streaming writes and two Codex
`token_count` events (one per model) was read by Tokscale `v4.21.0` and by
`originlm usage`. Per-category results reconcile:

| Category | OriginLM | Tokscale | Explanation |
| --- | --- | --- | --- |
| Claude input / output | 14 / 8 | 14 / 8 | identical |
| Claude cache read | 90 | 90 | identical |
| Codex fresh input | 83 | 90 | Tokscale's session parser ignores `cache_write_input_tokens`; OriginLM removes the 7 written tokens from fresh input and reports them separately |
| Codex cache read | 60 | 60 | identical |
| Codex output | 35 | 29 | Tokscale reports output excluding reasoning; OriginLM keeps output whole and reports the 6 reasoning tokens as a flagged subset |
| Codex cache write | 7 | not reported | OriginLM parses the field; Tokscale's JSONL model does not |
| Codex reasoning | 6 | 6 | identical |

Summed totals agree: OriginLM `fresh input + cache read + cache write` equals
Tokscale `input + cacheRead`, and OriginLM `output` equals Tokscale
`output + reasoning`. No prompt or tool content was read by either run.

A second shared fixture covered Grok and Kiro. Kiro matched Tokscale exactly
(input 10, output 4, cache read 2, cache write 1). Grok matched after the same
two conventions: Tokscale reported input 60 / cache write 0 (it ignores the
field) and output 16 / reasoning 4; OriginLM reported input 55, cache write 5,
output 20, reasoning 4. Tokscale did not discover the Gemini fixture in that
run, so Gemini remains fixture-tested by OriginLM rather than cross-checked.

## Providers without a token-history reader

Cursor's local history is a dashboard/cache surface, not a transcript token
feed, and OriginLM already treats Cursor usage as a separate opt-in dashboard
request. Copilot and Antigravity store conversation data in databases that
OriginLM does not open during history reads. Those are intentional boundaries,
not silent zeroes; `usage.history` simply omits providers with no readable
tokens.

## Service surface

`usage.history` and `costs.read.historical` use a broker-configured home
(`set_usage_home`) and cache the scan for 60 seconds. The CLI prints `roots`
for local debugging; the service clears them, so a broker client gets totals
and diagnostics without host directory paths, matching the public snapshot.

## Limits

This command reports only what the selected roots contain. It does not infer a
provider when no root exists, does not estimate missing tokens, and does not
roll totals up by day. `usage.read` remains the quota-window API; the token
history is `usage.history` and appears on the desktop Costs page.
