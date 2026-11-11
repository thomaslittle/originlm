# Built-in MCP server (`originlm mcp`)

`originlm mcp` runs a Model Context Protocol server over stdio from the shipped
`originlm` binary. Any MCP client (Claude Code, Codex, T3 Code, or another
client) can start it and ask how much Claude and Codex plan usage is left.

Here OriginLM is the MCP *server*. In [host-provided MCP servers](host-mcp.md)
the direction is reversed: OriginLM attaches a host's server to a native-client
session. The SDK's [cached-usage MCP](cached-usage-mcp.md) is a library for
apps that already own an OriginLM snapshot cache. `originlm mcp` needs no app
code.

## Tools

| Tool | What it returns | Backing data |
| --- | --- | --- |
| `usage` | For each connected provider: percent used and remaining per window (Claude: 5-hour session, weekly all-models, weekly per-model such as Fable, monthly spend or extra usage when enabled; Codex: each window it reports, named by its duration, e.g. `5 hr`, `Weekly`, plus enterprise monthly credits), reset time and time until reset, account (with `--allow-identity`) and plan, and whether the numbers are `live` or `cached` and how old they are. | The adapters' own quota probes: Claude Code's control-protocol `get_usage` and Codex app-server rate limits. No model turn is started. |
| `providers` | Clients that passive discovery found, with sign-in state, account, plan, whether usage is granted, and how many usage windows have been read. | Passive discovery plus earlier probes. It does not contact providers itself. |
| `usage_history` | Token totals per model from local session-history files: input, cached input, cache writes, output and reasoning. | The same reader as `originlm usage --json`. Prompts and outputs are never read. |

`usage` accepts `{"providers": ["claude_code"], "refresh": true}`. Both fields
are optional. `refresh` skips OriginLM's 60-second probe cache. A provider that
has not been granted is a tool error, not an empty result.

Every result has readable `content` text. It also has `structuredContent`, which
carries the same summary in a `summary` field because some clients show only
structured output. A window with no reading has `usedPercent: null` and means
unknown, never zero. When a refresh fails and an earlier reading exists, the
provider is `cached` with `ageSeconds` and `refreshError`.

Example `usage` text (from this repository's proof run):

```text
Claude Code · Default profile (you@example.com, plan Claude Max) — live, read 2s ago
  5-hour session: 18% used, 82% left, resets 2026-11-11T20:09:59Z (in 4h 3m)
  Weekly (all models): 82% used, 18% left, resets 2026-11-14T20:59:59Z (in 3d 4h)
  Weekly (Fable): 54% used, 46% left, resets 2026-11-14T20:59:59Z (in 3d 4h)
Codex · Default profile (you@example.com, plan ChatGPT Pro 100 Subscription) — live, read 3s ago
  Weekly: 61% used, 39% left, resets 2026-11-15T20:54:50Z (in 4d 4h)
```

## Flags

```text
originlm mcp [--allow-metadata PROVIDER]... [--allow-identity] [--allow-usage cursor]
             [--home ABSOLUTE_DIRECTORY] [--no-metadata-cache]
```

- Without `--allow-metadata`, the server may probe `claude_code` and `codex`.
  Any `--allow-metadata` list replaces that default. The values are the same
  ones `serve` accepts, such as `cursor`, `kiro` and `muse`.
- `--allow-identity` includes the signed-in account label (an email). Without
  it, `account` is `null` and `identityHidden` is `true`.
- Readings persist in `mcp-state.db` in OriginLM's state directory. This is a
  separate file from the broker's `state.db`, so the two never contend. After
  a restart, a provider that cannot be reached shows its last reading as
  `cached` with that reading's age. `--no-metadata-cache` turns persistence off.
- Execution, writes, HTTP and IPC flags are refused. This mode is read-only.

## Client configuration

Use an absolute path to the binary, for example
`C:\\Users\\you\\.cargo\\bin\\originlm.exe` after `cargo install originlm-cli`,
or `target/debug/originlm` from a source build.

**Claude Code**

```sh
claude mcp add originlm -- /absolute/path/to/originlm mcp --allow-identity
```

Or in `.mcp.json` / `--mcp-config`:

```json
{
  "mcpServers": {
    "originlm": {
      "type": "stdio",
      "command": "/absolute/path/to/originlm",
      "args": ["mcp", "--allow-identity"]
    }
  }
}
```

**Codex** (`~/.codex/config.toml`):

```toml
[mcp_servers.originlm]
command = "/absolute/path/to/originlm"
args = ["mcp", "--allow-identity"]
# The first call starts the Claude and Codex clients to read usage.
tool_timeout_sec = 120
```

**T3 Code and other clients**: add a stdio server with the same command and
args. Most clients accept the `mcpServers` JSON shape above.

## Latency

The first `usage` call runs a passive scan and then reads each provider through
its installed client. On the proof machine this took a few seconds. Different
providers are probed concurrently. Profiles of the same provider are probed one
at a time, because a probe can refresh that provider's credentials. Repeat
calls within 60 seconds use the probe cache unless `refresh` is set.
