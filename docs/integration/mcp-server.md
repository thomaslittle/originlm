# `olm usage` and the built-in MCP server

## Install

```sh
npm i -g originlm
```

This puts two names for the same command on your PATH, `olm` and `originlm`.
npm installs the native helper for your platform (`originlm-cli-<platform>`) as
an optional dependency. The `olm` command is a small dependency-free launcher in
the `originlm` package that runs that helper with your arguments, stdio and exit
status unchanged, the same pattern esbuild and Biome use. Do not install with
`--omit=optional`. If the platform package is missing, `olm` says which one to
install. `LAI_HELPER_PATH` can point the launcher at a helper you built yourself
(`cargo install originlm-cli`).

Helpers are published for Windows x64, Linux x64 and Linux arm64.

## `olm usage`

```text
$ olm usage --allow-identity
Claude Code · Default profile  ·  Claude Max  ·  you@example.com  ·  live, read 2s ago
  5-hour session         23% used    77% left  resets Wed Nov 11 15:09 (in 3h 25m)
  Weekly (all models)    83% used    17% left  resets Sat Nov 14 15:59 (in 3d 4h)
  Weekly (Fable)         54% used    46% left  resets Sat Nov 14 15:59 (in 3d 4h)

Codex · Default profile  ·  ChatGPT Pro 100 Subscription  ·  you@example.com  ·  live, read 3s ago
  Weekly                 61% used    39% left  resets Sun Nov 15 15:54 (in 4d 4h)

Detected without a usage reading:
  Claude Code · Additional profile 2  detected; no usage reported
  Codex · Additional profile 2        not signed in; no usage
  Cursor · Default profile            detected; usage not read (enable with --allow-metadata cursor --allow-usage cursor)
  Gemini CLI · Default profile        detected; usage unavailable (OriginLM has no usage source for Gemini CLI)
  Ollama · Default profile            detected; usage unavailable (OriginLM has no usage source for Ollama)

Percentages are the providers' own plan-quota readings; unknown is not zero.
Local token totals: olm usage --history
```

(Shortened. The real output lists every provider and profile detected.)

Every provider and profile that OriginLM discovery finds is listed:

- **Reading.** Claude Code and Codex are read by default through their installed
  clients. This uses Claude Code's `get_usage` and Codex app-server rate limits,
  and never starts a model turn. Each window shows percent used and left, the
  reset time in your local zone and the time until reset. Each account shows its
  plan and whether the reading is `live` or `CACHED` and how old it is. A window
  without a reading prints `unknown`, never `0%`.
- **Usage not read.** OriginLM has a usage source for this provider (Cursor,
  Muse, OpenCode) but you have not granted it. The row shows the flags that
  turn it on.
- **Usage unavailable.** OriginLM has no usage source for this provider today
  (for example Gemini CLI, Grok, Ollama, LM Studio). It is still listed, never
  with a number.

| Flag | Effect |
| --- | --- |
| `--json` | The same report as JSON, the `structuredContent` of the MCP `usage` tool. |
| `--refresh` | Skip the 60-second probe cache. |
| `--allow-identity` | Show the signed-in account label (an email). Hidden by default. |
| `--allow-metadata PROVIDER` | Which providers to read (repeatable). Replaces the Claude Code + Codex default. |
| `--history [--json]` | Local session-history token totals instead of plan quota. This was the output of `usage` before plan quota. |
| `--no-metadata-cache` | Don't keep readings in `usage-state.db` between runs. |

## Built-in MCP server (`olm mcp`)

`olm mcp` serves the same data over MCP stdio to any client (Claude Code, Codex,
T3 Code, or another client).

**Claude Code**

```sh
claude mcp add originlm -- olm mcp --allow-identity
```

Or in `.mcp.json`. On Windows, npm's shim is `olm.cmd`, so run it through
`cmd /c`:

```json
{
  "mcpServers": {
    "originlm": {
      "type": "stdio",
      "command": "cmd",
      "args": ["/c", "olm", "mcp", "--allow-identity"]
    }
  }
}
```

On macOS and Linux, use `"command": "olm", "args": ["mcp", "--allow-identity"]`.

**Codex** (`~/.codex/config.toml`):

```toml
[mcp_servers.originlm]
command = "olm"            # Windows: command = "cmd", args = ["/c", "olm", "mcp", "--allow-identity"]
args = ["mcp", "--allow-identity"]
tool_timeout_sec = 120     # the first call starts the Claude and Codex clients to read usage
```

**T3 Code and other clients**: add a stdio server with the same command and
args.

### Tools

| Tool | What it returns |
| --- | --- |
| `usage` `{providers?, refresh?}` | The `olm usage` report: every detected connection with `usageSource` (`granted` / `not_granted` / `none`), `data.state` (`live` / `cached` / `unavailable` / `not_read` / `no_source`), `ageSeconds`, `refreshError`, windows with `usedPercent` / `remainingPercent` / `resetsAt` / `resetsInSeconds`, plan, account (with `--allow-identity`), and a readable `summary`. `providers` narrows the listing to known provider ids. |
| `providers` | Discovered connections with sign-in state, account, plan, usage grant and window count. Does not contact providers. |
| `usage_history` | Local token totals (same as `olm usage --history`). |

`olm mcp` takes the same `--allow-metadata`, `--allow-identity`,
`--allow-usage cursor`, `--home` and `--no-metadata-cache` flags as
`olm usage`. Execution, write, HTTP and IPC flags are refused, so this mode is
read-only. Readings persist in `mcp-state.db`, separately from the broker's
`state.db` and from `usage-state.db`, so the three never contend.

Here OriginLM is the MCP *server*. In [host-provided MCP servers](host-mcp.md)
the direction is reversed. The SDK's [cached-usage MCP](cached-usage-mcp.md) is
a library for apps that already own an OriginLM snapshot cache.

## Latency

The first read runs a passive scan and then reads each granted provider through
its installed client, which takes about 5 seconds on the proof machine.
Different providers are read concurrently. Profiles of the same provider are
read one at a time, because a read can refresh that provider's credentials.
