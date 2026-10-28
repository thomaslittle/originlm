# Claude Code native adapter

The adapter uses the unmodified installed native Claude Code client and its own sign-in. Metadata uses its structured control initialization and optional `get_usage` control request; no user prompt is sent. Account/plan fields and native model/effort values are normalized with source evidence. Structured normalized usage windows take precedence over legacy aliases to avoid duplicated capacity. Missing information remains unknown.

CLI grants: `--allow-metadata claude_code` and, separately, `--allow-execution claude_code --workspace ABSOLUTE_DIRECTORY`. Multiple `--allow-metadata` and `--allow-execution` flags can be supplied. The older Codex-specific flags remain supported.

This adapter intentionally exposes **text-only execution**: safe/restricted native startup, no tools, strict empty MCP configuration, no Chrome integration, no slash-command expansion, and no persisted native conversation. Unexpected native tool requests are denied and terminate the turn. It does not claim to provide the full Claude coding toolset. Effort is a session-start option; changing it requires a new native session rather than silently mutating unrelated global preferences.

Native startup flags were verified against the installed client on September 24, 2026. Unsupported versions fail rather than silently fall back to an unrestricted launch. Windows native execution paths are qualified; Unix path handling and protocol tests still need native-platform execution evidence. Account metadata was checked end-to-end through the public Python SDK; generation tests use synthetic peers and consume no account allowance.

Sources: https://github.com/anthropics/claude-agent-sdk-python (native control protocol), https://code.claude.com/docs/en/cli-reference, and T3's audited ClaudeProvider.ts as a read-only integration reference. No T3 application runtime or credentials were copied. Experimental usage fields may change and are handled independently from authentication/model metadata.

## Host MCP servers

Sessions may attach host-provided loopback MCP servers; see [host MCP servers](../integration/host-mcp.md) for the wiring, enforcement and qualification status of this adapter.

## Persistent resumption

Persistent sessions and durable resume tokens are described in [native session resumption](../integration/native-session-resumption.md).
