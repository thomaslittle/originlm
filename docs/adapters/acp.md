# Shared ACP v1 adapter

The toolkit implements ACP initialization, session setup, route-scoped model catalogs, native configuration, text streaming, one-shot approval decisions, cancellation, and explicit close. It uses the existing owned-process boundary and the public session engine.

## Boundaries

No prompt is sent during inspection. Inspection may create a native session, so it requires explicit metadata/probe permission; it is not passive filesystem discovery. No filesystem or terminal callback is advertised. Unadvertised callbacks are rejected. Native built-in tools remain the native client's responsibility: ACP is not an OS sandbox. A provider integration must select an appropriate native permission policy before registering execution.

Omitted optional protocol capabilities are unsupported. Models retain native IDs. Controls apply to the currently inspected model; another model must be selected and its configuration re-read before applying dependent controls. Unknown control types are inspectable but not editable. Permission-mode controls cannot silently weaken grants. Model changes during a turn fail rather than silently billing a different route.

A standard ACP session does not publish universal plan, identity, billing, or account-quota fields. Provider adapters must supplement these using verified native metadata; absent information stays unknown. Context usage events are labelled `context_tokens`, not account balances. Native load-session support is recorded but persisted cross-process resumption is not exposed by this driver yet.

## Sources and verification

Protocol sources reviewed 2026-09-24:
- https://agentclientprotocol.com/protocol/v1/initialization
- https://agentclientprotocol.com/protocol/v1/session-setup
- https://agentclientprotocol.com/protocol/v1/session-config-options
- https://agentclientprotocol.com/protocol/v1/prompt-turn
- https://agentclientprotocol.com/protocol/v1/tool-calls

`cargo test -p originlm-adapters --test acp_catalog --test acp_process` exercises nine catalog and eight process/runtime cases with actual synthetic child processes. Both suites were observed failing before implementation. The full Windows workspace passed after implementation. A separate installed Grok initialization/session metadata probe confirmed protocol version 1 and native model/effort descriptors without generation; it is not an execution qualification.

## Text-only sessions

A session opened without `reviewRequests` is text-only: the host executes any tools itself from the model's text. The kit only opens such a session for clients whose native tools can be removed at launch, and fails the turn (`INTEGRATION_RESTRICTED`, with `session/cancel`) if the client emits any `tool_call`/`tool_call_update` anyway.

| Client | Text-only | Mechanism |
|---|---|---|
| Grok | Yes | `--tools= --disable-web-search --permission-mode=dontAsk --no-subagents` on every launch (probe and session) |
| Gemini CLI | Refused | No launch flag removes built-in tools; read tools (file read, glob, grep, search) run inside the client without an ACP permission request |
| Antigravity | Refused | Same as Gemini CLI |

Refusal is a typed `INTEGRATION_RESTRICTED` error from session open. Reviewed sessions (`reviewRequests: true`) keep the existing per-request mediation.

## Host MCP servers

Sessions may attach host-provided loopback MCP servers; see [host MCP servers](../integration/host-mcp.md) for the wiring, enforcement and qualification status of this adapter.
