# V1 dependency security review — 2026-10-28

Scope: integration branch `feat/v1-principal-review`. Reused the October 25
review for unchanged dependencies. GitHub's 15 open Dependabot alerts triggered
this focused follow-up; no account credentials were inspected.

| Dependency | Decision and official source |
|---|---|
| undici | 7.30.0 → 7.30.1, compatible security patch. [October 15 release](https://github.com/nodejs/undici/releases/tag/v7.30.1). Dev dependency through the JS tooling. |
| fast-uri | 3.1.8 → 3.1.9, compatible URI validation/security patch. [October 19 release](https://github.com/fastify/fast-uri/releases/tag/v3.1.9). |
| rustls | 0.23.45 → 0.23.46, compatible TLS security patch. [October 14 release](https://github.com/rustls/rustls/releases/tag/v/0.23.46). |
| setuptools | 84.0.0 → 84.1.0 for the build environment. [October 19 release](https://github.com/pypa/setuptools/releases/tag/v84.1.0), [Windows legacy-wheel advisory](https://github.com/pypa/setuptools/security/advisories/GHSA-xmj3-9gf7-h52w). |
| ratatui / lru | ratatui 0.29 pins vulnerable lru 0.12; directly selecting lru 0.16.3 was rejected by Cargo. Adopt stable ratatui 0.30.2 and its compatible lru 0.18.5, with crossterm 0.29. Enable only the used crossterm backend. [June 19 release](https://github.com/ratatui/ratatui/releases/tag/ratatui-v0.30.2), [upstream lru fix](https://github.com/jeromefroe/lru-rs/pull/224). Workspace MSRV becomes 1.88 to reflect this dependency; the pinned compiler is newer. |
| GPUI Kit | Keep 0.8.1, latest stable in the official registry and [October 26 release](https://github.com/longbridge/gpui-kit/releases/tag/v0.8.1). The October 25 audit predates its introduction; this records the missing review. |

No fork patches or Git-only snapshots were adopted. These were implemented before final verification. The complete local workspace
tests, all-feature clippy, native transport/TLS fixtures, JS tooling and clean
Python packaging passed for the reviewed source revision. No live provider operations
are needed. Windows and macOS results require their actual CI executions.

## Updater SemVer reuse

2026-10-28: adopted the existing lockfile's semver 1.0.28 as an exact direct
dependency of originlm-update; no new version or fork patch. Official
[crate release](https://docs.rs/crate/semver/1.0.28) and
[Version precedence API](https://docs.rs/semver/1.0.28/semver/struct.Version.html#method.cmp_precedence)
support numeric prerelease ordering and build-metadata-independent precedence.
This removes the updater's incorrect string comparison and rejects invalid staging
version syntax. Focused builds and signed updater fixture tests pass in the final local workspace matrix.


## CI runner baseline, 2026-10-28

The current `macos-latest` full-verification job was cancelled before any step
executed: GitHub's check annotations reported that no hosted runner acquired the
job and noted macOS ARM capacity constraints. Pin the main matrix to Ubuntu 24.04,
Windows Server 2025 and macOS 15 Intel. These are supported image labels in the
[official runner-image catalog](https://github.com/actions/runner-images#available-images).
This stabilizes the OS/architecture baseline rather than depending on a moving
`latest` label. The preparation workflow still builds macOS ARM and Intel artifacts.
This pin does not qualify unsupported OS versions or all architectures.

Evidence: [cancelled unacquired job](https://github.com/thomaslittle/originlm/actions/runs/40684248696/job/122802732310).

## Electron packaging qualification tools

Electron 45.1.0 and `@electron/asar` 4.3.1 were resolved from their official npm
packages for the Linux headless ASAR consumer smoke. They live only in ignored
verification tooling, not the SDK's dependency graph. The adopted path behavior
follows [Electron's ASAR documentation](https://www.electronjs.org/docs/latest/tutorial/asar-archives):
`spawn` needs the physical unpacked executable. This qualifies Linux packaging;
macOS/Windows Electron applications and signing remain unexecuted here.
