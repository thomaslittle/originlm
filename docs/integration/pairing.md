# Local runtime pairing

The HTTP runtime binds to loopback and requires explicit pairing. Its active code is random, single-use, and expires after five minutes. Reading the code does not consume it; successful redemption does. The code endpoint stops returning a code after redemption, expiry, or ten failed redemption attempts.

## Browser demo

On the pairing screen, choose **Show current pairing code**. The page reads the current code, fills the field, and leaves the final **Pair with this runtime** action to the user. If that code has already been redeemed or expired (for example, after a browser refresh lost its in-memory pairing token), the same button issues a fresh code and fills it in.

## HTTP API

Call `GET /runtime/pairing-code` on the exact loopback origin printed by the host. The response uses `Cache-Control: no-store` and contains the code and remaining lifetime:

```json
{"code":"012345abcdef","expiresInSeconds":287}
```

If the active code was redeemed or expired, explicitly create a replacement with `POST /runtime/pairing-code`. This replaces the pending code but leaves existing paired clients connected. Both methods are same-origin guarded and return `no-store` responses. The TypeScript SDK exports both `getPairingCode({ baseUrl })` and `issuePairingCode({ baseUrl })`.

The server validates the Host header and denies foreign Origin and cross-site browser requests. These endpoints have no bearer-token requirement because they are used before pairing. Keep the returned code in process memory, do not place it in a URL or log, and redeem it once with `POST /v1/pair`.

## TypeScript SDK

```ts
import { getPairingCode, issuePairingCode } from 'originlm';

const pairing = await getPairingCode({ baseUrl: 'http://127.0.0.1:4499' });
// pairing.code is the active one-use code; pairing.expiresInSeconds is its remaining lifetime.

// After a refresh or expiry, explicitly rotate the code:
const replacement = await issuePairingCode({ baseUrl: 'http://127.0.0.1:4499' });
```

The helper only accepts a loopback origin, sends no credentials, disables caching and redirects, and bounds the response before parsing it. `baseUrl` should be the origin returned by the local host. This API does not start a model turn or change any host grants.
