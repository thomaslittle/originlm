# OriginLM dependency audit — 2026-10-25

Verified checkout: `SOURCE_CHECKOUT`, branch
`feat/direct-cached-usage-remote`, observed HEAD
`95a3425c2d0e9188d9dedc9006ae87c645843290`. Working tree was clean at inventory.
This is the actual AI Kit repository, not an assumed similarly named project.

Inventory observed 2026-10-25; final documentation preparation 16:11 UTC.

## Current inventory and recommended follow-up

| Stack | Actual source / resolved version | Verified upstream and recommendation |
| --- | --- | --- |
| GPUI/rendering | No GPUI, GPUI Kit, Bezel or gpui-fast package in Cargo manifests/lock. Rust broker/core plus optional React consumer | No direct gpui-fast v0.1.0 migration applies. Crntly/Remotif native UI decisions do not require a GPUI dependency here. |
| Rust | Toolchain exactly 1.97.1, workspace MSRV 1.85, edition 2024/resolver 3 | [Stable 1.99.1, Oct 15](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/) is an evaluated update target. Keep declared MSRV distinct; no compiler update was performed. |
| SQLite | rusqlite 0.32.1, libsqlite3-sys 0.30.1; bundled+limits in store and adapters | [rusqlite 0.41.0 / libsqlite3-sys 0.39.0](https://docs.rs/crate/rusqlite/latest) is newer stable. This is several breaking lines behind and deserves a scoped storage review. Bundled SQLite does not inherit system SQLite security fixes. No confirmed CVE asserted. |
| HTTP/TLS | reqwest 0.12.28, rustls 0.23.45; jsonwebtoken 10.3.0, ring 0.17, Windows aws-lc-rs 1.18.2/prebuilt-nasm | [reqwest 0.13.5](https://docs.rs/crate/reqwest/latest) latest stable; evaluate feature/API migration for adapters and auth together. No latest claim for unqueried crypto crates. Separate safe patch/advisory review from a breaking HTTP migration. |
| Discovery/watch | notify exactly 8.2.0, file-id exactly 0.2.3 | [notify 8.2.1](https://docs.rs/crate/notify/latest) is a small candidate refresh. Preserve exact watcher/platform feature rationale, staged discovery and import side-effect rules. |
| Web demo | React/DOM 19.3.0, Vite exactly 8.3.3, plugin-react 6.1.1, TS 7.0.2, Tailwind 4.3.3 | [React 19.3.1](https://registry.npmjs.org/react/latest) paired patch and [Vite 8.3.5, Oct 21](https://github.com/vitejs/vite/releases/tag/v8.3.5) are bounded upgrades. [TS 7.0.2](https://registry.npmjs.org/typescript/latest), [Tailwind 4.3.3](https://registry.npmjs.org/tailwindcss/latest) match current stable registry tags. Vite 8.3.3 already meets October 6 advisory fixes. |
| SDK/runtime boundary | Internal Rust/TS SDK packages 0.1.0-alpha.18; Python preview 0.1.0a18, Python >=3.11, no declared third-party runtime dependencies | Deliberate project prerelease versions, not outdated third-party stable SDKs. No direct OpenAI, Anthropic or Google SDK dependency found in these manifests; adapters implement HTTP/CLI/ACP contracts. Do not add SDKs just to mirror provider package releases. |
| Build-only Python | setuptools 84.0.0, wheel 0.48.0, psutil 7.2.2, playwright 1.63.0 in scripts/requirements-build.txt | Explicit build/harness pins, distinct from thin Python SDK runtime. Latest versions not independently checked; update only with relevant build/browser compatibility evidence. |
| Terminal UI | ratatui 0.29.0, crossterm 0.28.1 | Older selected lines; lower priority than broker/storage/discovery. Current latest not independently checked; no unsupported migration target inferred. |
| Discovery knowledge: Tokscale | upstream/tokscale/lock.json: release baseline 4.17.0; extraction commit `1d9a9395418efc6952944b794097935d7d6fa1e8`, reviewed Sep 29; release commit `dcf8d3656bbcecf05d112c6f35f9be328e4f66b7` | [4.20.1, Oct 23](https://github.com/junhoyeo/tokscale/releases/tag/v4.20.1) is latest stable. Diff watched client/config/session paths and adopt relevant discovery corrections, not the dashboard/pricing/submission product. |
| Provider knowledge: T3 Code | UPSTREAM_T3CODE.md / snapshot: `cfafb4bb54d244e4612f5681aad3832ea64cb0fb`, Oct 16; recorded stable 0.1.0, nightly 0.1.1-nightly.20261016.3203 | [Stable 0.1.2, Oct 22](https://github.com/pingdotgg/t3code/releases/tag/v0.1.2); [Oct 25 nightly 0.1.3-nightly.20261025.3669](https://github.com/pingdotgg/t3code/releases) is prerelease. Diff consumed provider/auth/discovery files against stable first, then relevant nightly fixes. Never move the extraction pin merely because main/nightly moved. |

## Actual provider and integration exposure

`upstream/sources.json` says Tokscale/T3 are discovery/provider knowledge inputs;
released builds do not clone or execute them. Openusage/agentmanager are planned,
not installed dependencies. Existing sync scripts/workflow and attribution should
be reused; guidance here creates no new scheduler or wake-up.

The adapters and support matrix use native Codex app-server/CLI, Claude CLI,
ACP peers and explicit local/API HTTP paths. External client versions and
capability negotiation are runtime compatibility inputs, not entries in Cargo.lock.
Passive discovery is not execution approval. Do not run installed providers or
read auth caches merely to get versions during this audit. Capture returned
installation/version/protocol evidence in a later authorized consuming-flow smoke.
Remotif/Crntly consumers may run an older helper binary despite this source HEAD;
record binary generation and handshake rather than assuming the source version is
what is running. No provider account or inference was consumed here.

## Priority and acceptance

1. **P1:** refresh the overdue discovery/provider source review (Sep 29 / Oct 16)
   against the releases above using existing watched-path tooling and attribution.
   Preserve native provider IDs, scopes and unknown states; do not import upstream
   credential/account workflows wholesale.
2. **P1:** review bundled SQLite upgrade to 0.41.0 plus relevant upstream release
   notes; a wrapper jump requires API/storage/limits and cache-format review.
3. **P2:** qualify Rust 1.99.1, notify 8.2.1, paired React 19.3.1 and Vite 8.3.5.
   Evaluate reqwest 0.13.5 separately from auth/crypto patch maintenance.

Use focused locked builds and existing synthetic consuming-flow/visual smoke for
broker discovery, model selection, cached metadata, cancellation, storage reopen,
and the demo. No new unit tests or repeated unchanged audit campaigns. This audit
is implemented documentation only; no upgrade is implemented, fixture-tested,
platform-executed or live-qualified by this report.

## Scope and evidence limits

Reviewed 2026-10-25 UTC, from the actual Fedora 44 checkout. This is a bounded
manifest/lock/source audit, not runtime qualification or an exhaustive advisory
scan. Recommendations below are unimplemented. No dependency install/update,
build, app launch, branch/worktree change, staging, commit, push, account probe or
credential read was performed. Official public release/registry reads only;
private source stayed local. Shell DNS is unavailable; public research used the
web reader. Some API endpoints failed, so release pages and published crate docs
were used; unverified latest values are explicitly left open.

Host inventory: Node 24.22.1, npm 11.19.1; installed Rust toolchains 1.97.1,
1.98.1 (default) and a separately installed stable channel whose version was not
resolved. Java reports OpenJDK 25.0.2. RPMs: Vulkan loader 1.4.341.0-1.fc44,
Mesa Vulkan 26.0.3-4.fc44, PipeWire development 1.6.2-1.fc44,
libva development 2.23.0-3.fc44, ffmpeg-free 8.0.1-6.fc44. These are installed
versions, not assertions that Fedora updates are current. Distro backports need
Fedora package/advisory checks; upstream version comparison alone is insufficient.
Node 24.22.1 matches the [2026-10-21 security release](https://nodejs.org/en/blog/vulnerability/october-2026-security-releases),
and 24 is the [recommended LTS line](https://nodejs.org/en/about/previous-releases).
Do not substitute Current/nightly merely for a larger number.
