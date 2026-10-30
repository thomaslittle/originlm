# Installing OriginLM

## From a registry

```sh
npm install originlm
pip install originlm==0.1.0a20
cargo add originlm@=0.1.0-alpha.20
```

The npm package is the universal SDK; its five `originlm-cli-*` optional
dependencies carry the platform helper. PyPI publishes one project with five
platform wheels. The Rust crate is the single facade for the 13-crate closure;
the whole closure is published at `0.1.0-alpha.20`.

## Local bundle (offline/development)

Build dependencies live in an isolated environment:

```sh
python -m venv .venv-build
# Activate .venv-build using your platform's shell.
python -m pip install -r scripts/requirements-build.txt
npm ci --ignore-scripts
python scripts/build_release.py --profile release
python scripts/package_smoke.py
```

The bundle contains the native helper and Native Messaging host, the universal SDK and five platform npm tarballs, a Python wheel, static demo, browser extension, generated API documents, examples, checksums, license notices, and a dependency inventory covering the shipped Rust and JavaScript packages. It does not install itself, register an extension, read accounts, or run inference. The local bundle is for offline/development use; registry publication is handled by the gated release workflows.

Release staging produces one universal `originlm` npm tarball with all five
platform helper packages as optional dependencies, so a normal consumer installs
`originlm` and lets its platform resolve `originlm-cli-*` without naming a local
path. The four framework tarballs are staged alongside it for local preview;
install React separately only for the optional React packages. The Python wheel
requires Python 3.11+. The standalone Rust executables do not require Node or
Python.

```ts
import { createLocalAI } from 'originlm';
import { openBundledTransport } from 'originlm/node';
const transport = await openBundledTransport({
  bundleDirectory: '/absolute/path/to/extracted/bundle',
  args: ['serve', '--stdio', '--allow-passive-discovery'],
});
const ai = await createLocalAI({ transport });
try {
  await ai.discovery.refresh({ level: 'passive' });
  console.log(await ai.connections.list());
} finally { await ai.close(); }
```

```python
from originlm import Client, resolve_bundle_binary
binary = resolve_bundle_binary('/absolute/path/to/extracted/bundle')
with Client.from_stdio(executable=binary,
    args=['serve', '--stdio', '--allow-passive-discovery']) as ai:
    ai.discovery.refresh({'level': 'passive'})
    print(ai.connections.list())
```

Bundle resolution checks the manifest, host architecture, fixed binary location, size, and SHA-256 before launch. It never downloads a replacement, bypasses an OS permission, or substitutes a different architecture. Checksums detect corruption; they are not publisher signatures. Start the demo with `bin/originlm serve --http --assets ABSOLUTE_BUNDLE/demo --allow-passive-discovery` and enter the one-use pairing code.

`manifest.json` distinguishes a committed source build from a dirty development bundle. Package smoke tests run from a separate clean temporary consumer, not workspace aliases. Performance reports identify the exact tested binary, source, platform, and fixture; do not treat older measurements as final release results.
