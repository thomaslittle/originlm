# Changelog

## Unreleased

- fix(refresh): keep usage polling inside the 60s freshness window even when no client interest is active, preventing monitors from oscillating between stale idle cache and fresh post-read data.
- fix(opencode): recover OpenCode 2 usage refreshes from a stale service descriptor by starting the owner's own background service, and attribute Go quota windows to the exact `opencode-go` route when it is published.
Initial OriginLM source distribution: native Rust library and CLI, GPUI desktop manager, shared runtime, TypeScript and Python SDKs, browser bridge, React hooks and selectors, cache-only MCP/host/remote integrations, and release preparation tooling. Registry publication remains disabled.
