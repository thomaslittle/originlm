# Codex metadata adapter

## Implemented surface

The explicit `connections.probe` operation launches the installed native Codex app-server in a neutral temporary workspace. It negotiates initialization, reads account metadata without requesting a token refresh, paginates `model/list`, and reads rate-limit information when available. It does not start a thread, turn, tool, or generation request.

The native client continues to own authentication. This toolkit does not read `auth.json` to extract tokens, export credentials, impersonate another client, or call an image/text API with a subscription token.

### Observed metadata

- Native authentication mode and plan identifier, with field-level evidence.
- Connection-scoped account identity, masked unless the host grants identity access. A shared email does not merge two accounts or quota pools.
- Native model identifiers and supported effort/service-tier choices. Missing controls remain missing. No universal effort table or default-model override.
- Explicit input modalities; image input is not labeled image generation.
- Percent-based rate windows, durations, reset times, and `ordinaryUsageAllowed` when reported. Historical and multi-bucket rate-limit views are not double-counted. No conversion to fictitious token counts.
- Unknown usage remains unknown. Credit balances, reset-credit actions, and spend-control details are not fully normalized in this increment.

The native session engine and Codex text/approval/cancellation driver are implemented. Metadata-only hosts still advertise execution unavailable; an explicit execution grant and workspace are required. Billing-sensitive options require separate consent, and native permissions remain host-controlled. Execution protocol fixtures are not a live inference qualification.

## Explicit authorization

```sh
originlm serve --stdio --allow-passive-discovery --allow-codex-metadata
```

The first flag permits metadata-only filesystem inventory. The second separately permits the owning SDK to request Codex metadata. Merely launching the helper, importing an SDK, or reading a snapshot does neither. The RPC cannot grant itself this permission.

```python
ai.discovery.refresh({'level': 'passive'})
ai.connections.probe({'connectionId': selected_codex_connection_id})
snapshot = ai.snapshot.get()
```

An SDK application may inspect `runtime.handshake` and `grants.list`; the host must decide its authorization policy. Embedded Rust hosts can authorize specific connection IDs instead of an entire provider. The stdio CLI uses an owner context; the separate implemented IPC broker supports explicitly authorized applications without sharing their permissions.

Successful probes are cached for 60 seconds; repeated concurrent requests share work. Failed probes have a five-second cooldown. Cached snapshots do not create new observation timestamps on each read. Account changes reported during a probe invalidate the result. A startup account announcement before the authoritative account read is not treated as an account switch.

## Native launching and isolation

Windows native executables and known npm Codex shims resolve to the installed package's actual `codex.exe`; the shim is not run through a shell. macOS/Linux direct native paths are supported in code but have not been executed on those operating systems in this checkpoint. Shell/script packaging formats outside the qualified resolver remain unsupported.

Invocation-only flags disable hooks, plugins, remote plugins, apps, code-mode host, and shell snapshots. Saved client settings are not rewritten. There are no project sessions or tools. Child stdout is framed and bounded; stderr is drained without exposing raw logs; unexpected server execution/approval requests are rejected; timeout and cancellation clean up the owned client. This is a metadata integration, not a general-purpose security sandbox.

## Sources and qualification

Official interface: https://developers.openai.com/codex/app-server/

Installed version tested: **Codex CLI 0.156.1**, Windows x64. Exact local protocol schemas were generated with `codex app-server generate-json-schema` and inspected for account/model/rate-limit shapes. Executable resolution was checked against the installed package's `bin/codex.js` launcher. This implementation uses independently written parsers and protocol tests; it does not copy the provider applications.

The live end-to-end check used the Python SDK -> native OriginLM helper -> installed Codex client. It returned authenticated account/plan evidence, seven models with different native effort choices, and a rate-limit pool. No prompt was sent. Personal identity, tokens, raw native responses, and exact user allowance readings are not stored in this repository.

## Remaining limits

Native execution, approvals, paired local HTTP, shared IPC, and release bundles now have separate qualification records. Persistent cross-process resumption, automatic service-level file watching, cross-client authoritative quota linking, full balance normalization, and cross-platform live-account qualification remain unfinished. A metadata probe alone certifies none of those features. See ../qualification/RELEASE-CANDIDATE.md for the exact verified code and artifact scope.

## Host MCP servers

Sessions may attach host-provided loopback MCP servers; see [host MCP servers](../integration/host-mcp.md) for the wiring, enforcement and qualification status of this adapter.

## Persistent resumption

Persistent sessions and durable resume tokens are described in [native session resumption](../integration/native-session-resumption.md).
