# Host a cache-only usage MCP route

This example mounts `createCachedUsageRemoteMcp` on an existing authenticated HTTPS host. It starts no listener and contains no live endpoint, credentials or deployment configuration.

```ts
import {createCachedUsageRemoteMcp} from 'originlm/mcp/remote';

const usageRoute = createCachedUsageRemoteMcp({
  source: existingApprovedUsageProducer,
  publicOrigin: approvedHttpsOrigin,
  path: '/mcp/kit-usage',
  principalId: approvedUsageOnlyPrincipal,
  authenticate: existingHostAuthenticate,
});
// Your host router passes a verified Request to usageRoute.handle(request).
// authenticate returns verified {subject, scopes}; do not return raw credentials.
// Retire this route explicitly with usageRoute.revoke().
```

The principal needs `kit.usage.cached.read`. The route accepts stateless MCP POST requests and advertises only `kit_usage_cached`. Browser origins are denied unless explicitly allowed; GET and DELETE return 405. The host owns TLS, authority validation, authentication, timeouts and concurrency limits.

Prefer an already maintained, approved producer. `owner-cache.mjs` is an optional read-only SQLite example for an owner-designated cache; it refuses access unless `ownerApprovedCacheRead` is explicitly true. It does not grant permission to read another application's database.

Run the synthetic lifecycle check from the repository root:

```sh
node --experimental-strip-types examples/mcp/remote/inert-check.mjs
```

This creates only a temporary synthetic database and in-memory requests. The disabled `host-config.json.template` and `plugin-template/` show the required explicit deployment inputs; replace placeholders only after choosing and authorizing a host and data scope.
