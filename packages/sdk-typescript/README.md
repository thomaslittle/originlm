# OriginLM TypeScript SDK (`originlm`)

Published alpha on npm (`npm install originlm@alpha`) with prebuilt native helpers for **Linux (x64, arm64)** and **Windows (x64)**. On macOS or unsupported architectures, build the helper from source (`cargo install originlm-cli`) and set `LAI_HELPER_PATH`. See the [install guide](https://github.com/thomaslittle/originlm/blob/main/docs/integration/install.md) and [SDK quickstarts](https://github.com/thomaslittle/originlm/blob/main/docs/integration/quickstarts.md).

## CLI and built-in MCP server

Install globally to put `olm` and `originlm` on your `PATH`:

```sh
npm i -g originlm@alpha
olm usage
olm mcp
```

See the [CLI usage & MCP guide](https://github.com/thomaslittle/originlm/blob/main/docs/integration/mcp-server.md).

## Quick start (Node.js)

```ts
import { createLocalAI } from 'originlm';

const ai = await createLocalAI();
try {
  await ai.discovery.refresh({ level: 'passive' });
  const snapshot = await ai.snapshot.get();
  console.log(ai.mode, snapshot.revision, snapshot.connections.length);
} finally {
  await ai.close();
}
```

Automatic startup uses a granted shared broker or the packaged private helper. An incompatible broker protocol may select the private helper; denied authorization never triggers fallback. Requests themselves are not automatically retried.

### Explicit execution grants

Discovery never grants permission to execute. To run a turn through the private helper, pass explicit `probeProviders`, `executionProviders`, and a scoped `workspace`:

```ts
import { createLocalAI } from 'originlm';

const ai = await createLocalAI({
  auto: {
    probeProviders: ['codex'],
    executionProviders: ['codex'],
    workspace: { notes: process.cwd() },
  },
});
try {
  await ai.discovery.refresh({ level: 'passive' });
  const conns = (await ai.connections.list()).filter((c) => c.providerId === 'codex');
  for (const c of conns) await ai.connections.probe({ connectionId: c.id }).catch(() => {});
  const [pick] = await ai.selection.resolve({
    query: 'codex',
    requiredCapabilities: ['generation.text'],
    eligibleOnly: true,
  });
  if (pick) {
    const session = await ai.sessions.create({
      selection: pick.selection,
      workspaceId: 'notes',
      permissionMode: 'read_only',
    });
    const events = ai.events.subscribe({ sessionId: session.id });
    await ai.turns.start({
      sessionId: session.id,
      requestId: crypto.randomUUID(),
      prompt: 'Summarize the open questions in these notes.',
    });
    for await (const { payload } of events) {
      if (payload.type === 'text_delta') process.stdout.write(payload.text);
      if (payload.type === 'final') break;
    }
  }
} finally {
  await ai.close();
}
```

## Browser / loopback HTTP transport

Browser-safe HTTP/JSON-RPC and SSE client, plus a separate Node stdio entry point (`originlm/node`). No package import scans, reads credentials, starts processes, or makes requests. No provider-specific discovery logic lives here.

```ts
import { createHttpTransport, createLocalAI } from 'originlm';

const ai = await createLocalAI({
  transport: createHttpTransport({ baseUrl: 'http://127.0.0.1:4499', token: pairingToken }),
});
try {
  const snapshot = await ai.snapshot.get();
  // Host-approved policy is required for enrichment or execution.
  // grants.request reports requires_host_approval; it does not grant access.
} finally {
  await ai.close();
}
```

For a custom native helper invocation, import `openStdioTransport` from `originlm/node`, pass its absolute executable path and explicit argument array, then pass that transport to `createLocalAI`.

Domain and RPC types are generated from the Rust contracts (`src/generated.ts`); regenerate with `python scripts/generate_contracts.py` and verify with `npm run check:generated`.

## Implemented behavior

- Protocol-major handshake and advertised-method checks.
- HTTP requests use bearer headers, omit cookies, and reject redirects; literal loopback hosts only.
- Request deadlines, AbortSignal propagation, explicit close, no automatic RPC retries.
- Frame/body bounds, response ID validation, redacted error messages.
- SSE preserves UTF-8/chunk/line boundaries and reports explicit event gaps.
- Stdio launches only the explicitly supplied absolute binary with `shell:false`; approved environment names only, no inherited loader injection.
- Concurrent requests are correlated. Events arriving in the subscribe-response chunk are retained. Bounded event queues fail with `EVENT_GAP` rather than silently losing output.

The native service remains responsible for grants, workspace enforcement, secret redaction in successful payloads, native approvals, quota/billing authorization, and cleaning up its own agent descendants. Closing the SDK terminates its owned helper, not arbitrary user processes. A timed-out/cancelled RPC is not proof an already accepted native turn stopped; call `turns.cancel` for a known turn ID.

## Documentation & source

- [Repository](https://github.com/thomaslittle/originlm)
- [Install guide](https://github.com/thomaslittle/originlm/blob/main/docs/integration/install.md)
- [SDK quickstarts](https://github.com/thomaslittle/originlm/blob/main/docs/integration/quickstarts.md)
- [Provider support matrix](https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md)
- [Threat model](https://github.com/thomaslittle/originlm/blob/main/docs/security/threat-model.md)
