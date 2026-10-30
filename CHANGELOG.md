# Changelog

## Unreleased

- release(alpha.22): add the host-owned `--allow-identity` opt-in so an app-managed helper can reveal the provider-reported signed-in account label (email); without it the projection still withholds identity. The 14-crate closure is published at `0.1.0-alpha.22`; npm and PyPI remain at `0.1.0-alpha.20`.
- release(alpha.21): wire the OpenCode 2 text execution client into the service builder, dispatch 2.x sessions to the owner's local service API, accept the service's default model variant, and surface provider failure text. The 13-crate closure, npm platform packages and PyPI wheels are published at `0.1.0-alpha.21` / `0.1.0a21`.
- release(alpha.20): publish `originlm@0.1.0-alpha.20` to npm with five platform packages, `originlm==0.1.0a20` to PyPI with five wheels, and the 13-crate Rust closure to crates.io. The Windows npm package is `originlm-cli-windows-x64`. Runtime archives remain unsigned.
- feat(rust): mark the `originlm` facade dependency closure publishable with exact `version`/`path` pairs, add deterministic Cargo-metadata publication ordering and packaging checks, and expose session, turn, event, approval, execution-grant, and client-close APIs without weakening consent boundaries.
- release(npm): stage one universal `originlm` SDK tarball with all five exact-version platform optional dependencies, propagate native manifest/SBOM/notices/license texts into each platform package, and prove a fresh `createLocalAI()` consumer with empty home, no PATH helper and no overrides.
- release(alpha.20): prepare unsigned runtime archives with native per-target smoke gates, exact source/toolchain/target provenance, installer-compatible `helper-<target>/` layout, archive member manifests, and a complete per-target `originlm-cli` license closure. Registry publication is handled by the gated workflows; signing remains pending.
- fix(refresh): keep usage polling inside the 60s freshness window even when no client interest is active, preventing monitors from oscillating between stale idle cache and fresh post-read data.
- fix(opencode): recover OpenCode 2 usage refreshes from a stale service descriptor by starting the owner's own background service, and attribute Go quota windows to the exact `opencode-go` route when it is published.
Initial OriginLM source distribution: native Rust library and CLI, GPUI desktop manager, shared runtime, TypeScript and Python SDKs, browser bridge, React hooks and selectors, cache-only MCP/host/remote integrations, and release preparation tooling. Registry publication is handled by the gated workflows.
