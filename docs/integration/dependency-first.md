# Dependency-first clients

This guide covers the published packages and local packed artifacts/Cargo path dependencies for source development.
See [quickstarts](quickstarts.md) for concrete local installation. An application can use local AI by installing a dependency. It does not need the
standalone app, the broker or the control center, and it never installs them.

Every client resolves in the same order:

1. **Shared broker** the user already granted this application.
2. **Private helper**: a child process owned by this application.
3. **In-process** runtime — Rust only, and only when no helper binary exists.

Automatic clients expose `fallbackReason` (TS), `fallback_reason` (Python), or
`fallback_reason()` (Rust), set to `FALLBACK_PROTOCOL_MISMATCH` when a shared
version mismatch selected a private/embedded runtime. Otherwise it is null/None.
The active mode is always observable, and a missing standalone install is never
an error. A shared broker that is listening but rejects the grant is an error,
not a reason to fall back.

## Rust

```toml
[dependencies]
originlm = { path = "/absolute/path/to/originlm/crates/originlm" }
```

```rust
let mut ai = originlm::LocalAi::start()?;
let snapshot = ai.snapshot()?;
match ai.mode() {
    originlm::Mode::Shared => { /* one scanner shared by several apps */ }
    originlm::Mode::Child => { /* isolated helper owned by this app */ }
    originlm::Mode::InProcess => { /* fallback, no binary packaged */ }
}
println!("{} models", snapshot.models.len());
```

`StartOptions` narrows the policy (passive discovery is the default; probes are
opt-in) and can point at a specific helper, address or state directory. The
private helper is preferred over in-process because it keeps broker work out of
the application process; in-process is the fallback that makes "no separate
install" true, and it is reported as `Mode::InProcess`.

### Strict shared bootstrap

An application that must not silently fall back sets `required_mode` and its
application identity. The descriptor must be an application grant (`scope:
"application"`, `independent: true`, matching `appId`, protocol `1.0`); an owner
or single-use descriptor is rejected before authentication:

```rust
use originlm::{LocalAi, Mode, StartOptions};

let mut ai = LocalAi::start_with(StartOptions {
    required_mode: Some(Mode::Shared),
    address: Some(address),
    token_file: Some(token_file),
    expected_app_id: Some("haiwire".into()),
    ..Default::default()
})?;
```

Missing, malformed, foreign or revoked explicit grants fail closed. Automatic
resolution (`required_mode: None`) may fall back only when no explicit descriptor
or identity was supplied.

### Deterministic, cancellable embedded runtime

`Mode::InProcess` as the required mode ignores shared-token discovery, `PATH`,
adjacent helpers and `LAI_HELPER_PATH`. Host-owned configuration is forwarded
without changing process-global environment:

```rust
use originlm::{CancelToken, LocalAi, Mode, StartOptions};

let mut ai = LocalAi::start_with(StartOptions {
    required_mode: Some(Mode::InProcess),
    home: Some(app_owned_home),
    inventory: Some(synthetic_inventory),
    native_clients: native_registry,
    app_label: Some("haiwire".into()),
    request_timeout: std::time::Duration::from_secs(10),
    close_timeout: std::time::Duration::from_secs(2),
    ..Default::default()
})?;

let cancel = CancelToken::new();
// Host Stop/Disable path, any thread:
let stop = cancel.clone();
std::thread::spawn(move || stop.cancel());
let snapshot = ai.call_cancellable("snapshot.get", serde_json::json!({}), &cancel)?;
```

`call` uses `request_timeout`; `close` uses `close_timeout`. Embedded
cancellation drops the pending future and keeps the client usable; close
cooperatively cancels background lanes and the active inventory refresh, then
waits at most `close_timeout`. A sync inventory callback that ignores its cancel
flag cannot be aborted by Rust and is detached after that bound. Cancelling a
shared or child request returns `Cancelled` and closes that transport. The
private helper's drop reaps its immediate child on a best-effort bound; descendant
cleanup across platforms is not claimed. Helper bytes are not authenticated, so
use required in-process mode when an arbitrary local executable must not be
trusted.

`home`, `inventory`, `native_clients`, `state_dir` and `app_label` are honored
only by in-process mode. Supplying them with automatic, shared or child mode is
rejected before any broker connection or helper spawn. The facade re-exports
`ScanResult`, `LocatedClient`, `ExecutableIdentity`, `NativeProfile`,
`NativeProvider`, `Snapshot` and `DataMode`, so this compiles with `originlm` as
the only OriginLM dependency:

```rust
use originlm::{
    DataMode, InventorySource, NativeClientRegistry, ScanResult, Snapshot,
};
use std::{collections::BTreeMap, sync::Arc};

let inventory: InventorySource = Arc::new(|_cancel| {
    Ok(ScanResult {
        snapshot: Snapshot::empty(DataMode::Synthetic),
        locations: BTreeMap::new(),
        metadata_probes: 0,
    })
});
let native = NativeClientRegistry::default();
```

For exact Codex, Claude Code and OpenCode accounts, use the facade's
scoped-discovery configuration. It names the exact host-owned home and logical
executable for each account; the official scanner derives installation/profile/
connection IDs and Unix symlink identity. Accounts are scanned independently, a
failed account is omitted with a path-free diagnostic, and no global `HOME`,
`PATH` or provider environment is changed:

```rust
use originlm::{
    AccountScope, Mode, ScopedInventory, ScopedProvider, StartOptions, scoped_inventory_source,
};

let scoped = ScopedInventory::new()
    .with_account(AccountScope::new(ScopedProvider::Codex, codex_home, codex_exe))
    .with_account(AccountScope::new(ScopedProvider::ClaudeCode, claude_home, claude_exe))
    .with_account(AccountScope::open_code(
        opencode_config_home,
        opencode_data_home,
        opencode_state_home,
        opencode_exe,
    ));

let mut ai = LocalAi::start_with(StartOptions {
    required_mode: Some(Mode::InProcess),
    inventory: Some(scoped_inventory_source(scoped)),
    ..Default::default()
})?;
```

`NativeClientRegistry` remains limited to its native agent providers; it is not
used to impersonate Codex/Claude/OpenCode discovery or metadata adapters. An
OpenCode scope requires all three roots: `config_home` maps to
`OPENCODE_CONFIG_DIR`, `data_home` to `XDG_DATA_HOME`, and `state_home` to
`XDG_STATE_HOME`. Its `connections.probe` reads or starts only that account's
service descriptor; it never falls back to the process-global OpenCode service.

Tauri and GPUI use this crate. `LocalAi` also exposes `call`, `scan` and
`status`.

### Rust execution and consent

Starting a runtime is never execution consent. In-process execution drivers are
registered only when `StartOptions::execution` is `true`, and registration still
denies every session until the host records the user's exact grant:

```rust
use originlm::{ExecutionGrant, LocalAi, StartOptions};

let mut ai = LocalAi::start_with(StartOptions {
    required_mode: Some(Mode::InProcess),
    execution: true,
    ..Default::default()
})?;
ai.authorize_execution(ExecutionGrant {
    connection_ids: ["personal".into()].into_iter().collect(),
    workspaces: [("scratch".into(), workspace.canonicalize()?)].into_iter().collect(),
    // allow_paid, allow_writes, review_native, and allow_native_policy stay
    // false unless the user explicitly approved each one.
    ..Default::default()
})?;
```

`authorize_execution` is a host API, not an RPC and not a permission mutation.
`Mode::Shared` and `Mode::Child` return `LocalAccessDenied` because the user
approves applications through the broker; the facade can use that grant but
cannot widen or replace it. A client without a grant receives
`LocalAccessDenied` before an agent starts.

Sessions and turns use the same public contracts as the JSON-RPC surface:
`create_session`, `get_session`, `suspend_session`, `resume_session`,
`update_session_options`, `close_session`, `start_turn`, `cancel_turn`,
`respond_to_approval`, `read_events`, and `subscribe_events`. Subscriptions are
cursor-based polling (`EventSubscription::poll`); they do not spawn a task,
open a socket, or start a service. `LocalAi::close` closes only this client and
reaps an owned private helper.

## npm

```ts
import {createLocalAI} from 'originlm';

const ai = await createLocalAI(); // no transport argument
try {
  const snapshot = await ai.snapshot.get();
  console.log(ai.mode, snapshot.revision);
} finally { await ai.close(); }
```

The helper ships as per-platform optional dependencies, the way esbuild and Biome
do:

```
originlm-cli-linux-x64  originlm-cli-linux-arm64
originlm-cli-darwin-arm64  originlm-cli-darwin-x64
originlm-cli-windows-x64
```

The SDK resolves `originlm-cli-<platform>/package.json` and the binary it
declares, so installation is a normal dependency update: no postinstall script,
no download at import time. The Node transport is intended for Node and Electron's main process; Bun execution must be qualified separately.
Renderer processes use the web bridge instead.

`node scripts/npm-platform-packages.mjs --binary <path> --target <triple> --out <dir>`
builds those packages for a release. The workspace `package.json` stays free of
platform dependencies so `npm ci` is identical on every machine; the preparation
workflow stages private local packs with matching optional dependencies.

An explicit transport still works exactly as before:
`createLocalAI({transport: await openSharedBrokerTransport({...})})`.

## Python

```python
from originlm import Client

with Client.auto() as ai:
    print(ai.mode, ai.snapshot.get({"fields": ["revision"]}))
```

(`AsyncClient.auto()` is the async form.) Wheels bundle the helper under
`originlm/_bin/`, and resolution follows the same order.

## Overrides

| Variable | Purpose |
| --- | --- |
| `LAI_HELPER_PATH` | Use a local source build instead of the packaged binary. Must be an absolute path to a real executable; a `.cmd`/`.bat`/`.ps1` wrapper is rejected. |
| `LAI_BROKER_ADDRESS` | Shared broker address (`$XDG_RUNTIME_DIR/originlm/user.sock` on Linux, `originlm-user` on Windows by default) |
| `LAI_BROKER_TOKEN_FILE` | This application's grant descriptor (`~/.originlm/broker-token.json` by default) |

A shared descriptor must be owner-only (`0600`), may not be a symlink, and its
`address` must match the configured one. Nothing is scanned or read at import
time; all work starts on the first call.

## Private helper lifetime

The helper is started as `serve --ipc <unique socket>` with the parent-owned
lifetime: it prints one single-use descriptor and stops when the application
closes its control channel or exits. It never publishes a reusable token file,
so the broker can detect parent-channel closure; OS crash/shutdown behavior still needs platform qualification. The socket lives in the OS
temporary directory with an owner-restricted, per-process name.

## Safety rules that did not change

- Imports perform no scan, probe, credential read, spawn or network call.
- Each application still has its own grant; a shared broker is only used when a
  valid descriptor exists, and revocation is per app.
- Provider credentials and tokens are never exported, logged or written.
- Transports are owner-only sockets/pipes, never an open port.

## Publishing

Registry publication is handled by the gated release workflows. The packages are
published at `0.1.0-alpha.21`; the preparation workflow itself still produces
unsigned local artifacts only and has no publication authority. See
[release gates](../release/PUBLISHING.md).
