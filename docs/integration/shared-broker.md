# Shared local broker

`originlm serve --ipc ADDRESS --allow-passive-discovery` runs one local service. Use a simple local pipe name on Windows and an absolute socket path inside your private application directory on Unix. Windows uses an owner-only pipe security descriptor and rejects remote pipe clients. Unix sockets use mode 0600. An existing listener is not unlinked or replaced.

## Lifetime modes

**Parent-owned (default).** The broker's stdin/stdout are a **trusted parent application control channel**, not a log: stdout returns an initial JSON descriptor with `address`, an expiring one-use `token`, and `protocolVersion`. The host may send `{"action":"authorize"}` on stdin to issue another independent client credential. Do not publish or log this control output. Closing the parent control input stops the broker; closing an application connection does not.

**Independent.** `serve --ipc ADDRESS --independent (--token-file ABSOLUTE_PATH | --grants-file ABSOLUTE_PATH) [--admin-token-file ABSOLUTE_PATH] [--idle-timeout SECONDS]` owns its own lifetime. It stays alive when every consumer disconnects and stops after the idle timeout when configured. Endpoint binding is the single-instance boundary: a second launch fails with `CONFLICT`; it never replaces or unlinks a live endpoint. Token files are removed on graceful shutdown.

`--token-file` is the legacy owner-scoped reusable credential for a single application. `--grants-file` is the multi-app boundary: an owner-only JSON manifest with `schemaVersion: 1` and up to 32 apps. Each app has a unique `id`, its own absolute `tokenFile`, optional policy fields (`allowPassiveDiscovery`, `connectionIds`, `revealIdentity`, `probeConnectionIds`, `probeProviderIds`), and an optional execution grant (`providerIds`, `connectionIds`, absolute `workspaces`, `allowPaid`, `allowWrites`, `reviewNative`, `allowNativePolicy`). The broker writes a distinct reusable credential per app; each connection carries that app identity. The optional admin descriptor is a separate owner-only credential with `apps.list` / `apps.revoke`; give it only to the control center, not to ordinary applications. Revoking an app deletes its broker grant and sweeps every live connection with its identity, so the old token cannot reconnect. An app token file is still readable by the same OS user; file/ACL isolation is not a defense against that user's other processes.

```json
{"schemaVersion":1,"apps":[
  {"id":"remotif","tokenFile":"C:/Users/me/AppData/Local/originlm/remotif-token.json",
   "allowPassiveDiscovery":true,"probeProviderIds":["opencode"],
   "execution":{"providerIds":["opencode"],"workspaces":{"default":"C:/Users/me/remotif"}}}
]}
```

Use `originlm serve --ipc ADDRESS --independent --grants-file ABSOLUTE_PATH --admin-token-file ABSOLUTE_PATH`. The legacy owner token and app grants may be combined; each app should be configured with its own token file, not the owner token.

Applications connect using the native `connect --ipc ADDRESS` bridge. The token travels as the first JSON-RPC frame over stdin, never a command-line argument. The bridge does not start a broker automatically.

```ts
import {createLocalAI} from 'originlm';
import {openSharedBrokerTransport} from 'originlm/node';
// Starts one independent broker only if no live instance is reachable.
const ai = await createLocalAI({transport: await openSharedBrokerTransport({
  executable: approvedBinaryPath,
  address: 'originlm-user',
  tokenFile: 'C:/Users/me/AppData/Local/originlm/broker-token.json',
  startArgs: ['--allow-passive-discovery', '--allow-metadata', 'codex'],
})});
await ai.discovery.refresh({level:'passive'});
const env = await ai.snapshot.get();
console.log(env.revision, env.cursor, env.health, env.issues);
const stop = ai.inventory.subscribe({after: env.cursor});
await ai.close(); // Other applications and the independent broker remain.
```

Python: `AsyncStdioTransport.open_broker(executable=..., address=..., token=...)` followed by `AsyncClient.connect(transport)`, or `client.subscribe_inventory(after=snapshot['cursor'])`. Embedded Rust hosts can call `Broker::authorize(policy, execution_grant)` separately for each application, or `Broker::authorize_shared_for(app_id, policy, execution_grant)` for an independent app-scoped broker. Each application has independently revocable scopes; no protocol request can grant itself broader access. Disconnect unregisters that client. Parent tokens expire after five minutes and are consumed once; app-scoped independent credentials are valid until the broker stops or the administrator revokes the app.

Verified on Windows with two independent SDK helper processes, one shared scan, denied third-client access, one-use credentials, duplicate broker rejection, client-specific revocation, independent lifetime across consumer disconnects, duplicate-launch convergence, idle shutdown, and a control-center process using only the public API. App-scoped grant tests additionally prove two distinct app identities and account projections, cross-account denial, and live revocation of one app's reusable grant without affecting the other app. The same workspace and its broker/transport tests pass in CI on Ubuntu and macOS; CI does not exercise a real desktop GUI. No implicit installation, telemetry, remote relay, or cross-user brokerage.


## Autonomous refresh and subscriber hints

`--allow-passive-discovery` separately authorizes background inventory. Existing
`--allow-metadata PROVIDER` grants authorize that provider's background metadata
and supported usage observations; client interest cannot grant either. Cached
reads do not rescan. The service uses slower inventory/metadata and faster usage
cadences, reduces idle work, coalesces all routes/apps in one verified connection,
and shares bounded concurrency and Retry-After/backoff with explicit probes.

Inventory event pages are scoped invalidation hints. Currently visible resource
IDs are retained; historical removals whose scope cannot be reverified use the
anonymous `inventory/*` hint. Read a fresh scoped snapshot after a hint, preserve
its cursor, and use the existing gap/full-resync path on `EVENT_GAP`. A timer may
mark usage stale and request authoritative data; it never resets an allowance.
