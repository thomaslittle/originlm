# Host-provided MCP servers

A host application can run a loopback MCP server exposing its own tools (for example design-editor actions) and attach it to a native-client session. The default remains a read-only, host-tools-only session. A separately granted `native_policy` session can also use the native CLI's shell, file, web, and build tools under that CLI's own permission policy. This delegates those limits to the installed client and is not a OriginLM sandbox.

## Contract

`sessions.create` accepts an optional, additive `hostMcpServers` list. Omitting it (or sending `[]`) is unchanged behavior.

```json
{
  "selection": { "routeId": "...", "modelId": "...", "revision": "...", "options": {} },
  "workspaceId": "scratch",
  "hostMcpServers": [
    {
      "name": "design",
      "url": "http://127.0.0.1:48123/mcp",
      "bearerToken": "<32-512 printable ASCII characters>",
      "allowedTools": ["echo_color", "set_fill"]
    }
  ]
}
```

Validation is strict; any violation is `INVALID_REQUEST` before a client is launched:

| Field | Rule |
| --- | --- |
| list | at most 2 servers, at most 256 allowed tools in total, unique names |
| `name` | `^[a-z][a-z0-9_]{0,31}$` |
| `url` | `http://127.0.0.1:<port>/...` or `http://[::1]:<port>/...`; port 1-65535; a path is required; no other host, no `https`, no userinfo, whitespace, `\` or fragment |
| `bearerToken` | 32-512 characters, printable ASCII without spaces |
| `allowedTools` | non-empty, unique, each `^[a-z][a-z0-9_]{0,63}$` |

Further rules:

- The existing execution grant is required. Omitting `permissionMode` or using `read_only` keeps the current host-tools-only behavior. `workspace_write` with host servers is `INTEGRATION_RESTRICTED`; `native_policy` can be combined with host servers only when the native-tools capability is advertised and the operator granted it.
- `permissionMode: "native_policy"` preserves native shell, file, web, and build tools and native permission behavior. It does not impose a helper sandbox. Native tools are reported with neutral `tool` events containing only name and status. Arguments and results are not copied into events.
- `reviewRequests` remains a separate opt-in. The operator must use `--review-native-requests`; requests become the existing `approval_request` events and must receive a once-only `approvals.respond` decision. Without host review, native approval requests are declined. Paid-route consent remains independent.
- The helper operator must use `--allow-native-policy` in addition to the normal `--allow-execution PROVIDER --workspace ABS` grant. `--allow-writes` grants the separate helper-controlled `workspace_write` mode and does not grant native policy.
- Billing rules are unchanged: metered, prepaid or mixed routes still need `allowPaid` plus the grant's paid consent.
- The token is never logged or echoed. `HostMcpServer`'s `Debug` output redacts it, sessions/events/errors never contain it, and no adapter places it on a command line.
- The handshake advertises `session.host_mcp` and `session.host_mcp.native_tools` when an execution driver is registered. Support is still per route: integrations that cannot attach host servers or native tools return `CAPABILITY_UNSUPPORTED`.
- Host tool calls surface as the neutral `tool` event, `name` = `mcp__<server>__<tool>`, `status` only (no arguments or results).
- Session resumption carries the same host servers to the resumed native session.

## Per-provider support

| Integration | Wiring | Status |
| --- | --- | --- |
| Claude Code | Per-session `--mcp-config <file>` in a private temp directory (`0600` on Unix; per-user temp ACL on Windows) and `--strict-mcp-config`. Read-only mode keeps `--tools ""`; native-policy mode leaves built-in tools and native permission settings enabled. `--allowedTools=mcp__<server>__<tool>,...` scopes and pre-approves only host tools. Native-policy mode adds `--permission-prompt-tool stdio` so the CLI sends permission requests through its stream-json control protocol; the file is deleted on session close. | Native Write, Read, WebSearch and a host tool call completed in one Claude Code Haiku session against a fresh scratch workspace and local test MCP server; three native approval requests were approved once each by the test host (October 20, 2026). |
| Codex app-server | Per-thread `thread/start` config `mcp_servers.<name>` = `{url, http_headers.Authorization, enabled_tools, default_tools_approval_mode: "approve", required: true}`. The token travels only over the app-server stdio pipe. Every other MCP server configured in the Codex home is disabled for the thread (`mcp_servers.<other>.enabled = false`); a configured server with the host server's name is refused (`INTEGRATION_RESTRICTED`). Read-only mode retains the existing read-only sandbox/tool restrictions. Native-policy mode uses `danger-full-access`, enables shell and web tools, and sets native operation requests to `on-request`; the host mediates those requests only when `reviewRequests` is enabled. | Integrated live Remotif Control TV session with Codex Luna completed native shell HTTPS fetch, workspace JSON write, and host MCP action (October 20, 2026). |
| ACP (native profiles, e.g. Grok) | `session/new` / `session/load` `mcpServers: [{type:"http", name, url, headers:[{name:"Authorization", value:"Bearer …"}]}]`, only when the peer advertises `agentCapabilities.mcpCapabilities.http`; otherwise `CAPABILITY_UNSUPPORTED`. A permission request for a host tool is answered `allow_once`. | Fixture-tested only. |
| Muse Code | `session/start` / `session/resume` `config.mcpServers.<name> = {transport: "streamableHttp", url, headers: {Authorization: "Bearer …"}, mode: "optional"}`, after `initialize` grants `sessionMcp` (else `CAPABILITY_UNSUPPORTED`). The token travels only over the stdio pipe. `optional` keeps the turn running without the tools when Muse cannot reach the server, as T3 Code does. | Fixture-tested; live session start with an unreachable server verified on Muse 1.4.7 (echo provider, no inference). |
| OpenCode, HTTP/API endpoints | — | `CAPABILITY_UNSUPPORTED` |

## Enforcement

- **Claude Code:** read-only `system/init` may advertise only allowed `mcp__<server>__<tool>` names; other tools fail with `INTEGRATION_RESTRICTED`. Native-policy mode admits the CLI's built-in tools and reports only name/status. An MCP tool is reported as a host tool only if it matches an allowed pair. Startup waits for every host server to report `connected` through `mcp_status`; a failed or never-connecting server is `PROVIDER_UNAVAILABLE`.
- **Codex:** read-only, unreviewed sessions report allowed `mcpToolCall` items and reject other tools. Native-policy sessions report native `commandExecution`, `fileChange`, MCP, web, and other tool items as neutral events; only configured host MCP pairs receive a host-tool name. Approval requests follow the independent native review grant.
- **Muse Code:** a `toolCall` named `mcp__<server>__<tool>` for an allowed pair is a host tool; any other MCP tool fails the turn. Muse cannot remove its own read-only tools (`read_file`, `search`, `tool_search`, `read_skill`, `write_todos`, `request_user_input`, `subagent_*`), so read-only sessions allow those; shell, writes, web tools and workflow launches are disabled at launch, and any other native tool fails the turn with `INTEGRATION_RESTRICTED`. See [Muse Code](../adapters/muse.md).
- **ACP:** ACP does not say which MCP server a tool call came from. A `tool_call` counts as a host tool only when it has no native `kind` (absent or `other`) and its `title` resolves to exactly one allowed tool (`mcp__s__t`, `s__t`, `s/t`, `s.t`, `s:t`, or an unambiguous bare tool name). Everything else keeps the text-only rule from `docs/adapters/acp.md`.

## Claude Code and safe mode

Claude Code 2.1 ignores `--mcp-config` servers in `--safe-mode` (verified: `mcp_status` returned no servers). Host-MCP sessions therefore omit `--safe-mode`. `--restricted` still ignores user/project/local settings files (hooks, plugins, permission rules), `--disable-slash-commands` keeps skills off, `--tools ""` removes built-in tools, and `--strict-mcp-config` excludes every other MCP server. What safe mode additionally suppressed and host-MCP sessions may load: CLAUDE.md instructions, custom agents and output styles. They add instructions, not tools, and the tool checks above still apply. `--bare` is not an alternative because it disables OAuth sign-in.

## Live checks

Codex models that defer tools to code mode need the host MCP namespaces exposed as direct tools. The adapter sets `features.code_mode.direct_only_tool_namespaces` for only those namespaces and disables `features.non_prefixed_mcp_tool_names` so they cannot overlap built-ins such as `functions`. An already-prefixed server name retains its prefix; server names that normalize to the same namespace fail with `INTEGRATION_RESTRICTED`. The code-mode host, shell/exec, unrelated integrations and existing tool-call guards stay unchanged. See the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

On Windows with Codex 0.159.2, a separately owned managed ChatGPT session and a Claude Code session each called one fixed, read-only host tool and completed with the exact expected reply. The earlier managed configuration completed text turns but made no tool call and reported that the tool host was disabled. A later Claude Code Haiku test used a fresh scratch workspace to trigger host-mediated approvals for native Write, Read and WebSearch, then completed those tools plus the authenticated local host MCP tool in the same session. These qualify the bounded tool round trips, not every host application's editor actions.

`crates/originlm-adapters/tests/host_mcp_live.rs` is opt-in: set `LAI_LIVE_CLAUDE`, `LAI_LIVE_CLAUDE_HOME`, `LAI_LIVE_MCP_URL`, `LAI_LIVE_MCP_TOKEN` (and optionally `LAI_LIVE_CLAUDE_MODEL`, default `haiku`). `LAI_LIVE_MCP_STATUS_ONLY=1` stops after the no-inference connection check; otherwise the read-only path calls only `echo_color`. Setting `LAI_LIVE_MCP_NATIVE_TOOLS=1` runs the native-policy path in a fresh scratch workspace with no native tool pre-allow rules; the test host approves each native request once and verifies file write/read, web search, and the host `echo_color` call in one session. The Codex test `live_codex_native_tools_and_host_mcp_share_a_session` is also opt-in via `LAI_LIVE_CODEX` and `LAI_LIVE_CODEX_HOME`.
