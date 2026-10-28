# Web bridge

Plain websites need a deliberate way to reach the local runtime. The extension
plus Native Messaging host is the primary path and opens no port. Loopback HTTP
with pairing is the fallback.

## Extension first (no open port)

The host is the existing native messaging binary. Registration is per browser
and per extension identity:

```sh
node scripts/build_extension.mjs
python scripts/native_host.py --binary target/release/originlm-native-host \
  --destination /absolute/private/host --browser chrome \
  --extension-id YOUR_CHROME_EXTENSION_ID --install
```

Firefox identifies extensions by id, not by a `chrome-extension://` origin:

```sh
python scripts/native_host.py --binary target/release/originlm-native-host \
  --destination /absolute/private/host --browser firefox \
  --extension-id originlm@your-domain.example --install
```

- Chrome/Chromium/Edge write `allowed_origins`; Firefox writes
  `allowed_extensions`. The Rust host accepts either shape and checks the caller
  against the exact entry (`valid_caller`), never a prefix.
- Website-origin grants are required for the planned content bridge. The current
  extension does not expose the host to arbitrary website pages.
- Pages never receive credentials; they receive results.
- The host registration is per user and reversible with `--unregister`.

**Gap:** the example extension currently has no page-facing content script. Until
that content bridge ships, `detectExtension()` answers `false` and sites use the
pairing fallback. The host and manifests are ready for it.

## Pairing fallback (loopback)

The helper can serve loopback HTTP with a one-time pairing code:

```sh
originlm serve --http --port 47821 --allow-passive-discovery
```

- Same-origin host boundary; the pairing code is single-use and short lived.
- The page exchanges the code for a short-lived, scoped token.
- Cross-origin website requests and Private Network Access preflights are not
  enabled. Open the dashboard served by the helper to pair today.
- `DEFAULT_PAIRING_ORIGIN` in `@originlm/web` is `http://127.0.0.1:47821`;
  the service installer enables IPC only; start the explicit HTTP command above.

## Detection

```ts
import {detect, badgeLabel} from '@originlm/web';
const bridge = await detect(); // 'extension' | 'pairing' | 'none'
```

- `extension`: the content bridge answered a postMessage ping.
- `pairing`: a readable, validated loopback `/runtime/info` response was received; the page still needs the
  explicit code exchange.
- `none`: show "Connect your local AI" with the install page.

Detection never starts a scan, never grants access and never reads a credential.

## Badge

| State | Copy |
| --- | --- |
| authenticated client with providers | `Local AI connected · Claude, Ollama` (max three, then `+N`) |
| reachable, not authenticated | `Runtime available; pairing required` |
| nothing available | `Not detected` + install link |
| unavailable or browser-blocked | `Not detected`; detection cannot establish a sandbox cause |
