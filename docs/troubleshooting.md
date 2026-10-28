# Troubleshooting

**A client is detected but no models appear.** Passive detection deliberately does not read login state or contact a provider. Enable a supported metadata grant and use an explicit probe. A detected IDE is not necessarily the provider's native agent executable.

**A plan or quota is unknown.** The provider did not report it through the implemented interface. A cached plan remains stale; the toolkit will not manufacture a token balance or combine unrelated quota windows.

**A route is unavailable or billing is unknown.** Inspect the route's evidence and adapter descriptor. API billing and subscriptions are separate. Missing native integration is not automatically replaced by paid API access.

**The browser needs a pairing code.** On the local pairing screen, choose **Show current pairing code**. The demo retrieves it from the loopback helper; if it expired or was already used, the button issues a replacement. Pairing remains a separate explicit action. For host integrations, call `GET /runtime/pairing-code` / `getPairingCode({ baseUrl })`; rotate after expiry or redemption with `POST /runtime/pairing-code` / `issuePairingCode({ baseUrl })`. Codes expire after five minutes. Do not place them in URLs or logs. Closing/revoking a connection removes that app's access.

**A bundle will not launch.** Verify `manifest.json`, target architecture, and checksums. SDK bundle helpers refuse modified binaries, foreign platforms, relative paths, and missing manifests. These checks detect corruption, not publisher identity; unsigned alpha builds may still produce an OS warning.

**A development run fills the disk.** Rust incremental/test artifacts can be large. Use `CARGO_INCREMENTAL=0` for qualification; remove only this project's generated target cache after stopping its build processes. Never delete unrelated user files, credential folders, or global application caches.

**Windows and WSL show different clients.** They are separate execution environments. Run a WSL-native helper to inspect WSL. Passive Windows inventory does not start a stopped distribution or merge its account data with Windows accounts.
