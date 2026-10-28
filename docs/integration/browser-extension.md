# Browser extension and Native Messaging host

The optional example is generic: it inspects connections/models and can run an explicitly permitted text session. It contains no page content scripts, cookie access, or provider credentials. It uses the public TypeScript client and `createNativeMessagingTransport`.

Build the binaries and extension:

```sh
cargo build --release -p originlm-cli
npm ci --ignore-scripts
npm run build
node scripts/build_extension.mjs
```

Load `dist/browser-extension` as an unpacked Chrome/Chromium/Edge extension and copy its extension ID. Prepare its host in a NEW private directory:

```sh
python scripts/native_host.py --binary /absolute/path/originlm-native-host \
  --destination /absolute/private/application/host --extension-id YOUR_EXTENSION_ID
```

On Windows, use the `.exe` binary. Preparation copies the binary, writes the exact origin allowlist, and produces a checksummed receipt; it does **not** change the browser or registry. Add `--install --browser chrome` to explicitly register for the current user. Existing registrations and nonempty destination directories are never overwritten. `--unregister /absolute/path/com.originlm.host.json` removes only the matching unmodified registration; installed payload files are retained.

By default only explicit passive scanning is permitted. Add repeatable `--allow-metadata PROVIDER` to let the extension request account/catalog checks. Text execution additionally requires matching `--allow-execution PROVIDER --workspace /absolute/workspace`; the example always requests read-only, non-metered execution and requires an unchecked-by-default consent control. Approvals for tools are denied by this simple example. No generation happens on connection, scan, or model selection.

The native host independently verifies Chrome's calling origin against its installation configuration. Native messages use a native-endian 32-bit length prefix and UTF-8 JSON. Both directions are deliberately capped at 1 MiB. Malformed/truncated frames fail closed. The same Rust service performs authorization, selection, execution, and cancellation; there is no separate privileged browser API.

References: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging

Tests exercise actual native-binary framing and origin checks, JavaScript Port transport requests/events, and generated registration files. Browser-store publication and unattended host installation are not performed.
