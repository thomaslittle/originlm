# Bind Kit cached usage to the existing T3 MCP host

This source module composes with T3's existing authenticated MCP host. It starts
no transport and creates no credentials, account feed or access grant. It is not
enabled by importing it. Cloud dot needs that existing reachable authenticated
host; the standalone Kit stdio demo cannot supply cloud connectivity.

Reconciled against T3 `5bb54a0f77c6655fdb2ab551612ca1ca1fcd3667`:
[`McpHttpServer`](https://github.com/pingdotgg/t3code/blob/5bb54a0f77c6655fdb2ab551612ca1ca1fcd3667/apps/server/src/mcp/McpHttpServer.ts),
[`McpToolAccess`](https://github.com/pingdotgg/t3code/blob/5bb54a0f77c6655fdb2ab551612ca1ca1fcd3667/apps/server/src/mcp/McpToolAccess.ts),
and [`McpInvocationContext`](https://github.com/pingdotgg/t3code/blob/5bb54a0f77c6655fdb2ab551612ca1ca1fcd3667/apps/server/src/mcp/McpInvocationContext.ts).
T3 and its installed Effect 4.0.5 are MIT-licensed. This module uses their public
APIs/patterns; it copies no implementation. Build the local OriginLM SDK first; registry packages are not yet published.

## Explicit composition, after host/scope approval

Place [kit-usage-toolkit.ts](kit-usage-toolkit.ts) in the selected host's source
and reference it from that host's normal layer composition. The following names
are operator-supplied inputs, not auto-discovery or existing configuration fields:

```ts
import * as Layer from 'effect/Layer';
import {createCachedUsageProducer, createT3CachedUsageTool} from 'originlm/mcp/host';
import * as McpHttpServer from './mcp/McpHttpServer.ts';
import * as McpToolAccess from './mcp/McpToolAccess.ts';
import * as McpInvocationContext from './mcp/McpInvocationContext.ts';
import {createKitUsageToolkit} from './mcp/kit-usage-toolkit.ts';

// The selected host already owns/maintains this approved SnapshotStore.
const producer = createCachedUsageProducer({
  store: alreadyMaintainedStore,
  scope: {connectionIds: approvedConnectionIds, accountIds: approvedAccountIds},
  mappings: operatorVerifiedAccountMappings,
});
const binding = createT3CachedUsageTool({
  source: producer,
  environmentId: approvedEnvironmentId,
  clientSessionIds: approvedExternalMcpClientSessionIds,
});
const {toolkit, handler} = createKitUsageToolkit(McpInvocationContext.McpInvocationContext);
const handlers = McpToolAccess.toLayer(toolkit, {
  kit_usage_cached: McpToolAccess.reads(handler(binding)),
});
const usageLayer = McpHttpServer.toolkitRegistration(toolkit, handlers);
const combinedMcpLayer = Layer.merge(McpHttpServer.layer, usageLayer);
// Provide combinedMcpLayer through the host's existing service composition.
// Attach once in the selected cache owner's lifecycle; dispose on its finalizer:
producer.attach();
// Retirement: binding.revoke(); producer.dispose();
```

Do not provide a captured caller at layer construction. Pass T3's real
`McpInvocationContext` tag; its authentication middleware supplies the request's
environment and client session. Keep `toolkitRegistration`: it handles that
request-scoped context without registering a raw alternative transport. The
tool accepts `{}` only; its `Schema.Never` record rejects extra keys instead of
silently stripping a requested refresh or another account scope.

The Kit binding is a second, narrower authorization boundary. T3's existing
read-only MCP access is necessary transport permission, not an implicit grant to
Kit accounts. Only the configured external client session can read. Agent/thread
credentials remain denied. OAuth renewal/replacement may change a session ID;
never follow it by account/name inference. Deliberately recreate the binding
after the operator approves that session and account scope.

No host was modified, built, restarted or deployed by this example. This composition
must be reconciled again if T3 changes its toolkit/auth APIs. The SDK producer and
binding are fixture-tested on Windows. The module was type-checked and executed
against the installed Effect toolkit in an isolated in-memory five-call journey
(allowed, different caller, excess arguments, retired source, revoked binding),
with zero broker/provider requests and zero network listeners. T3's complete
HTTP/OAuth/layer composition and an actual dot call remain unexecuted.

See [scope, lifecycle, interest leases and exact live approvals](../../../docs/integration/cached-usage-mcp.md).
