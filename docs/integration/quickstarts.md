# SDK and framework quickstarts

The Rust crates are published at `0.1.0-alpha.22`, including the `originlm-cli` helper; npm and PyPI remain at `0.1.0-alpha.20` until their publishers run:

```sh
npm install originlm
pip install originlm==0.1.0a21
cargo add originlm@=0.1.0-alpha.22
```

The local-artifact flow below remains for source development and offline
verification. Use a synthetic home for verification; these examples never request
inference.

## Prepare local Node and Python artifacts

From the checkout on macOS/Linux, after installing CONTRIBUTING's prerequisites:

```sh
cargo build --locked --release -p originlm-cli --bin originlm
npm ci --ignore-scripts
npm run build --workspace originlm
mkdir -p .verification/quickstart-artifacts
TARGET="$(rustc -vV | sed -n 's/^host: //p')"
node scripts/npm-platform-packages.mjs --binary "$PWD/target/release/originlm" \
  --target "$TARGET" --out .verification/quickstart-platform
node scripts/npm-add-platform-deps.mjs --packages .verification/quickstart-platform \
  --sdk packages/sdk-typescript --out .verification/quickstart-sdk --preview
for package in .verification/quickstart-platform/* .verification/quickstart-sdk; do
  npm pack "$PWD/$package" --ignore-scripts --pack-destination "$PWD/.verification/quickstart-artifacts"
done
python scripts/stage_python_package.py --binary "$PWD/target/release/originlm" \
  --target "$TARGET" --out .verification/quickstart-wheels
(cd .verification/quickstart-wheels/originlm && python setup.py bdist_wheel --dist-dir ../../quickstart-artifacts)
```

The wheel defaults to the host's platform when built, not a claimed manylinux baseline.
Use the manylinux 2.28 qualification workflow for a publishable Linux wheel;
`--release` is required to remove the private publication guard. Windows can use the
preparation workflow's x64 artifacts; the helper filename ends in `.exe`.
[Release preparation](../release/PUBLISHING.md) lists the exact target set.

## Plain Node

Install from the registry:

```sh
npm install originlm
```

For offline or source development, set `ORIGINLM_ARTIFACTS` to the absolute artifact directory:

```sh
npm init -y
npm install --offline --ignore-scripts "$ORIGINLM_ARTIFACTS"/originlm-*.tgz
node hello.mjs
```

`hello.mjs`:

```js
import { createLocalAI } from 'originlm';
const ai = await createLocalAI();
try {
  const snapshot = await ai.snapshot.get();
  console.log(ai.mode, snapshot.revision);
} finally {
  await ai.close();
}
```

Install both SDK and matching platform tarballs in that command. No helper-path
override is needed when the optional platform package is present. For source-only
iteration, `LAI_HELPER_PATH` can point at an absolute executable instead.

## Python

Install from the registry:

```sh
python -m pip install originlm==0.1.0a21
```

For offline or source development, install the matching wheel by its actual path:

```sh
python -m pip install --no-index /absolute/path/to/originlm-0.1.0a21-py3-none-PLATFORM.whl
python hello.py
```

`PLATFORM` stands for the filename printed by the build; it is not a literal tag.
`hello.py`:

```python
from originlm import Client
with Client.auto() as ai:
    print(ai.mode, ai.snapshot.get()['revision'])
```

For an async application, use `async with await AsyncClient.auto() as ai` and await
RPC methods. Both forms own and close their private helper.

## Rust

```sh
cargo new originlm-hello
cd originlm-hello
cargo add originlm@=0.1.0-alpha.22
cargo run
```

For source development, use a path dependency instead:
`cargo add originlm --path /absolute/path/to/originlm/crates/originlm`.

`src/main.rs`:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ai = originlm::LocalAi::start()?;
    let snapshot = ai.snapshot()?;
    println!("{:?}: {}", ai.mode(), snapshot.revision);
    Ok(())
}
```

Rust embeds the runtime if no helper exists. Use `ai.scan()?` for an explicit passive
refresh. Metadata probes and execution need additional policy; a discovery signal is
not permission to run a provider.

## Tauri

Add the same path dependency to the existing `src-tauri/Cargo.toml`. Register this
command with your builder's `invoke_handler` and invoke it from a user Connect action:

```rust
#[tauri::command]
async fn ai_snapshot() -> Result<originlm::Snapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut ai = originlm::LocalAi::start().map_err(|e| e.to_string())?;
        ai.scan().map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}
```

This is an integration into an existing Tauri app, not a complete generated Tauri
scaffold. Install [Tauri’s OS prerequisites](https://v2.tauri.app/start/prerequisites/)
first. The shared Rust call pattern is executed here; a separate Tauri command-build
attempt on this Linux host is blocked by missing GTK/WebKit development packages. Keep executable paths, arbitrary RPC and grant construction out of renderer
input. For repeated use, retain the owned client in host state and close it on exit.

## Electron

Install the same local tarballs in your existing Electron app. Run the Node example
from a trusted main-process Connect handler after `app.whenReady()`. Expose only a
narrow snapshot command through preload; keep context isolation enabled. Include
`node_modules/originlm-cli-*/bin/**` in the packager's `asarUnpack`. The helper must
be a real executable outside the asar archive. The SDK resolves Electron’s
`.asar.unpacked` path; `HELPER_NOT_UNPACKED` means the binary was not unpacked.
Close the client when its owner exits.
Do not run a Node helper transport in the renderer.

## GPUI

Add the Rust dependency to your existing app. Run the Rust start/scan sequence inside
`cx.background_executor().spawn(async { ... })`; return `Result<Snapshot, String>`.
Await that task in `cx.spawn`, then update your view through its context. Do not block
the render thread with broker startup or provider I/O. This repository's
[`originlm-desktop`](../../crates/originlm-desktop/src/broker.rs) is the complete worker/channel
consumer and also demonstrates cancellation and shutdown ownership.

## Browser: extension or same-origin pairing

Build the extension with `node scripts/build_extension.mjs`. Register the actual
native host for its exact extension ID as documented in [web bridge](web-bridge.md).
Inside the registered extension page, after a Connect action:

```js
import { createLocalAI, createNativeMessagingTransport } from 'originlm';
const transport = createNativeMessagingTransport({
  connect: () => chrome.runtime.connectNative('com.originlm.host'),
});
const ai = await createLocalAI({ transport });
try { console.log((await ai.snapshot.get()).revision); }
finally { await ai.close(); }
```

A plain hosted website cannot use that API. No page-facing extension content bridge
ships yet. The implemented alternative is the dashboard served by the helper at its
own loopback origin: start `originlm serve --http --port 47821 --allow-passive-discovery`,
open the served dashboard, and exchange its one-time code. Cross-origin hosted-site
pairing/PNA is not enabled. See [pairing](pairing.md) for the actual transport API.

## Reference and troubleshooting

The generated API is [protocol/methods.json](../../protocol/methods.json),
[OpenAPI](../../protocol/openapi.json), TypeScript declarations and Python stubs.
Negotiate methods through `runtime.handshake`; generated shapes do not grant access.
See [troubleshooting and FAQ](troubleshooting.md) for missing helpers, grants and costs.
