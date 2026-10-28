# Status badge

The same facts power the dashboard, `originlm status` and a small badge on a
website. The browser badge reads no credential; the CLI reads its configured grant descriptor. It shows what a granted client
is allowed to see.

## The data

`originlm status --json` prints one object and exits:

| exit | meaning |
| --- | --- |
| 0 | running and this credential is accepted |
| 3 | no helper is listening (or no descriptor is configured) |
| 4 | the helper explicitly rejected this credential |
| 1 | connection, descriptor, protocol or other runtime failure; authorization is unknown |

Illustrative shape (counts are example values, not a product claim):

```json
{
  "running": true,
  "authorized": true,
  "mode": "shared",
  "protocolVersion": "1.0",
  "revision": "projection_…",
  "providers": ["Codex", "Claude Code", "Ollama"],
  "models": 371,
  "connectedApps": 2
}
```

## The badge

```ts
// Uses the web bridge (extension first, pairing second).
const status = await detect(); // 'extension' | 'pairing' | 'none'

const badge =
  {label: badgeLabel(status, [], false), action: status === 'none' ? 'Install OriginLM' : 'Connect'};
// Pass authenticated=true only after a successful SDK handshake, with provider
// names from that granted client's snapshot.
```

Rules:

- Render at most three provider names, then "+N".
- "Not detected" always offers the install page; it never says "no providers
  found", because that would hide a sandbox or a missing helper.
- Detection alone cannot identify a sandbox. Show a sandbox explanation only
  when the host supplies that evidence.
- The badge is read-only: it cannot start a scan, grant access or run a turn.
  Those still require an explicit call and the normal grant.

## Terminal

```sh
originlm status
originlm status --json
```

`status` reads an existing broker only. It never starts, scans or probes one, so
it is safe in a shell prompt or a health check.
