# @originlm/web

Browser bridge detection for OriginLM. Importing performs no request.

```ts
import {badgeLabel, detect} from '@originlm/web';

const bridge = await detect(); // 'extension' | 'pairing' | 'none'
document.body.dataset.originlm = badgeLabel(bridge);
// Only pass authorized=true and snapshot provider names after SDK authentication.
```

The extension path needs its page content bridge (see
[docs/integration/web-bridge.md](../../docs/integration/web-bridge.md)); the
pairing fallback probes a loopback origin and leaves the code exchange to the
explicit connect flow.

This package is experimental. The current host permits same-origin pairing only;
cross-origin website pairing and a page-facing extension bridge are not implemented.
An unavailable probe does not establish that the helper is absent.
