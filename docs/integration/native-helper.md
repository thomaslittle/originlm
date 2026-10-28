# Native helper and SDK bridge

The standalone helper, embedded Rust facade, shared broker, and live showroom use the same native service. The separate synthetic demo never substitutes for a failed native connection.

## Build and verify

```sh
npm ci --ignore-scripts
npm run build
cargo build --locked -p originlm-cli
python scripts/test_native_bridge.py
```

Bridge tests run Node/Python against the actual executable with isolated temporary homes. They check discovery, concurrent requests, permission denial, broker ownership, and revocation without needing a provider account.

## Direct CLI

```sh
originlm scan --json
originlm serve --stdio --allow-passive-discovery
```

Imports, handshake, and cached snapshot reads do not scan. `--allow-passive-discovery` permits metadata-only requests. `--home ABSOLUTE_DIRECTORY` isolates roots/PATH/home overrides for deterministic tests and never falls back to the real home.

Stdio uses bounded UTF-8 newline JSON-RPC. The host supplies caller identity; request parameters cannot grant permissions or choose an arbitrary executable. Handshake precedes other operations. Subscribe to native events only when advertised. Large model catalogs can be read with [bounded pages](catalog-pages.md).

## TypeScript

```ts
import { createLocalAI } from 'originlm';
import { openStdioTransport } from 'originlm/node';
const transport = await openStdioTransport({
  executable: '/absolute/path/to/originlm',
  args: ['serve', '--stdio', '--allow-passive-discovery'],
});
const ai = await createLocalAI({ transport });
try {
  await ai.discovery.refresh({ level: 'passive' });
  console.log(await ai.connections.list());
} finally { await ai.close(); }
```

Use the full `.exe` path on Windows. The package's optional `openBundledTransport` verifies a local distribution manifest before launching; no shell or implicit download is required.

## Python

```python
from originlm import Client
with Client.from_stdio(
    executable='/absolute/path/to/originlm',
    args=['serve', '--stdio', '--allow-passive-discovery'],
) as ai:
    ai.discovery.refresh({'level': 'passive'})
    print(ai.connections.list())
```

## Metadata, execution, and browser access

Provider metadata requires a separate host flag, for example `--allow-metadata codex`. Actual turns additionally require `--allow-execution codex --workspace ABSOLUTE_WORKSPACE` and explicit session/turn requests. A metadata probe is not an inference request. Inspect the [support matrix](../adapters/SUPPORT-MATRIX.md) for each provider's actual operations.

HTTP/SSE and browser pairing are implemented. Use `serve --http --assets ABSOLUTE_DEMO_DIRECTORY --allow-passive-discovery`; enter the one-use code printed by the host. See the [live demo guide](live-demo.md), [distribution guide](distribution.md), and [artifact interface](artifacts.md). Closing a client releases its own scopes, not another application's broker session. Static assets are limited to `html`, `css`, `js`, `mjs`, `svg`, and `woff2` files under the assets root (2 MiB cap); the exact paths `/overview`, `/connections`, `/models`, `/usage`, `/selector-lab`, `/playground`, and `/diagnostics` serve `index.html` for client-side routing, and every other path without a matching file is a 404.

Consult the [support matrix](../adapters/SUPPORT-MATRIX.md) for current provider limits and qualify the built helper on each target platform. Passive inventory never reports an active subscription merely from a local installation. Native Google/Grok execution, automatic service-level watch refresh, and persistent cross-process resumption remain explicitly unavailable where documented.
