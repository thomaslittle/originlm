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
Claude Code · Default profile  ·  Claude Max  ·  you@example.com  ·  live, read 19s ago
  5-hour session                44% used    56% left  resets Wed Nov 11 15:10 (in 2h 49m)
  Weekly (all models)           88% used    12% left  resets Sat Nov 14 16:00 (in 3d 3h)
  Weekly (Fable)                58% used    42% left  resets Sat Nov 14 16:00 (in 3d 3h)

Codex · Default profile  ·  ChatGPT Pro 100 Subscription  ·  you@example.com  ·  live, read 21s ago
  Weekly                        61% used    39% left  resets Sun Nov 15 15:54 (in 4d 3h)

Antigravity · Additional profile 4  ·  Google AI Pro  ·  live, read 17s ago
  Claude models                  0% used   100% left  resets Wed Nov 11 17:20 (in 4h 59m)
  GPT-OSS models                 0% used   100% left  resets Wed Nov 11 17:20 (in 4h 59m)
  Gemini models                  0% used   100% left  resets Wed Nov 11 17:20 (in 4h 59m)

Cursor · Default profile  ·  Premium  ·  you@example.com  ·  live, read 12s ago
  Included usage               3.7% used  96.3% left  resets Sun Nov 15 13:53 (in 4d 1h)

GitHub Copilot · Default profile  ·  Copilot Free  ·  your-github-login  ·  live, read 8s ago
  Chat messages                  0% used   100% left  resets Mon Nov 30 19:00 (in 19d 6h)
  Code completions             0.2% used  99.8% left  resets Mon Nov 30 19:00 (in 19d 6h)

Grok · Default profile  ·  SuperGrok  ·  live, read 9s ago
  Weekly credits                 3% used    97% left  resets Sun Nov 15 13:53 (in 4d 1h)

OpenCode · Default profile · Installation 2  ·  OpenCode Go Subscription  ·  live, read 1s ago
  Rolling                        1% used    99% left  resets Wed Nov 11 12:42 (in 21m)
  Weekly                         1% used    99% left  resets Sun Nov 15 19:00 (in 4d 6h)
  Monthly                       29% used    71% left  resets Fri Dec 4 00:39 (in 22d 12h)

Pi · Default profile  ·  live, read 11s ago
  OpenRouter key (free tier)  $0.17 used, no limit

Detected without a usage reading:
  Codex · Additional profile 2                 no usage: not signed in
  Gemini CLI · Default profile                 no usage: Gemini CLI signs in with a Gemini API key here; that is metered and Google publishes no usage endpoint for a key
  Muse Code · Default profile                  no usage: Muse reports usage only after this login has run a turn
  OpenCode · Default profile · Installation 1  no usage: this installation's runtime is missing
  LM Studio · Default profile                  detected; usage unavailable (local runtime; LM Studio has no plan quota)
  Ollama · Default profile                     detected; usage unavailable (Ollama's local API reports sign-in and plan, not cloud usage limits)

Percentages are the providers' own plan-quota readings; unknown is not zero.
Local token totals: olm usage --history
```

(From the proof machine, shortened: duplicate profiles are left out and the
account labels are replaced. The real output lists every detected profile.)

Every provider and profile that OriginLM discovery finds is listed. Usage is
read by default from every provider that has a usage source, through the
source that provider's own client uses. No model turn is started.

| Provider | Usage source | What is shown |
| --- | --- | --- |
| Claude Code | Claude Code control protocol `get_usage` | 5-hour session, weekly (all models), weekly per model, monthly spend when enabled |
| Codex | Codex app-server rate limits | Each window Codex reports, named by its length (`5 hr`, `Weekly`), monthly credits |
| Cursor | Cursor dashboard `GetCurrentPeriodUsage`, with the session token Cursor stores | Included usage (and API usage when non-zero) |
| OpenCode | The local OpenCode service's workspace limits | Rolling, weekly and monthly OpenCode Go limits |
| Muse Code | Muse `usage/read` | Plan windows once Muse has observed usage for the login; until then the row says so |
| Grok | Grok's ACP `_x.ai/billing` extension | Credit usage for the current period (weekly for SuperGrok) and the tier |
| Antigravity | Google Code Assist `fetchAvailableModels`, with the managed profile's own `acp_token.json` | Remaining quota per model family (Claude, Gemini, GPT-OSS) and the Google tier |
| Gemini CLI | Google Code Assist `retrieveUserQuota` with the CLI's saved Google login | Per-model quota for a Google-login profile. A Gemini API key or Vertex AI profile has no plan quota, and the row says which |
| GitHub Copilot | `api.github.com/copilot_internal/user` for the GitHub account signed in to `gh` | Premium requests, chat and completions allowances, monthly reset |
| Pi | OpenRouter `/api/v1/key` for an OpenRouter login saved in Pi | Dollars used on the key, and the limit when one is set. Other Pi logins have no usage call |
| Ollama | None: the local API reports sign-in and plan (`/api/me`), not cloud usage limits | Listed as usage unavailable |
| LM Studio | None: local runtime with no plan quota | Listed as usage unavailable |

How credentials are handled:

- Tokens are read into memory for one request. They are never logged, never
  serialized, and never written back.
- The Antigravity credential is refreshed in memory with that file's own
  client, and only `oauth2.googleapis.com` may receive it.
- The Gemini CLI token is used only while it is unexpired.
- Code Assist answers only requests that name the client the login belongs to.
  Those requests send `antigravity originlm/<version>` or
  `GeminiCLI originlm/<version>`, so they stay identifiable as OriginLM's.

Each row shows one of these states:

- **Reading.** Percent used and left per window, the reset time in your local
  zone and the time until reset, the plan, and whether the reading is `live` or
  `CACHED` and how old it is. A non-percent allowance such as OpenRouter
  dollars is shown as an amount. A window without a reading prints `unknown`,
  never `0%`.
- **No usage: reason.** The source was read but gave nothing. The reason comes
  from the provider, for example "not signed in", "this installation's runtime
  is missing", "Muse reports usage only after this login has run a turn", or
  "the client did not answer in time".
- **Usage not read.** The provider has a usage source, but this run excluded
  it with `--no-usage` or an explicit `--allow-metadata` list.
- **Usage unavailable.** The provider has no usage source. It is listed with
  the reason, never with a number.

Claude Code and Codex default profiles are read first. Other providers'
profiles follow, three at a time, and the whole read is bounded at 60 seconds.
A client that does not answer by then gets its own "did not answer in time"
row.

| Flag | Effect |
| --- | --- |
| `--json` | The same report as JSON, the `structuredContent` of the MCP `usage` tool. |
| `--refresh` | Skip the 60-second probe cache. |
| `--allow-identity` | Show the signed-in account label (an email). Hidden by default. |
| `--no-usage PROVIDER` | Leave one provider's usage unread (repeatable), e.g. `--no-usage cursor --no-usage pi`. |
| `--allow-metadata PROVIDER` | Read only these providers (repeatable). Replaces the default; it cannot be combined with `--no-usage`. |
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
tool_timeout_sec = 120     # a full read starts each provider's client; it is bounded at 60 s
```

**T3 Code and other clients**: add a stdio server with the same command and
args.

### Tools

| Tool | What it returns |
| --- | --- |
| `usage` `{providers?, refresh?}` | The `olm usage` report: every detected connection with `usageSource` (`granted` / `not_granted` / `none`), `data.state` (`live` / `cached` / `unavailable` / `not_read` / `no_source`), `ageSeconds`, `refreshError`, windows with `usedPercent` / `remainingPercent` / `resetsAt` / `resetsInSeconds`, plan, account (with `--allow-identity`), and a readable `summary`. `providers` narrows the listing to known provider ids. |
| `providers` | Discovered connections with sign-in state, account, plan, usage grant and window count. Does not contact providers. |
| `usage_history` | Local token totals (same as `olm usage --history`). |

`olm mcp` takes the same `--no-usage`, `--allow-metadata`, `--allow-identity`,
`--home` and `--no-metadata-cache` flags as `olm usage`. Execution, write, HTTP and IPC flags are refused, so this mode is
read-only. Readings persist in `mcp-state.db`, separately from the broker's
`state.db` and from `usage-state.db`, so the three never contend.

Here OriginLM is the MCP *server*. In [host-provided MCP servers](host-mcp.md)
the direction is reversed. The SDK's [cached-usage MCP](cached-usage-mcp.md) is
a library for apps that already own an OriginLM snapshot cache.

## Latency

The first read runs a passive scan and then reads each provider through its
installed client. On the proof machine, reading all twelve providers took
about 50 seconds; with `--allow-metadata claude_code --allow-metadata codex`
it takes about 5.
Different providers are read concurrently. Profiles of the same provider are
read one at a time, because a read can refresh that provider's credentials.
