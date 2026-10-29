# Official package release ledger

Updated 2026-10-29. This ledger records what is actually built and verified; it
does not claim registry publication or consumer migration before those happen.

## Registry status

| Registry | Package/name state | Credential state |
| --- | --- | --- |
| npm | `originlm` and `@originlm/cli-*` unclaimed (`404` at check time) | No local npm login (`ENEEDAUTH`); no GitHub repository secret/environment |
| crates.io | Not published | No local cargo credentials file; no GitHub repository secret/environment |
| PyPI | `originlm` unclaimed (`404` at check time) | No local `.pypirc` or twine configuration; no GitHub repository secret/environment |

Actual publication must use maintainer/root-worker credentials in a protected
environment. No credential was created, read or exposed.

## npm universal SDK

- Branch: `release/full-official-packages`
- PR: [#14](https://github.com/thomaslittle/originlm/pull/14)
- Head: `a0c18e5d82c6b87ce7a7e0624790b11cdffcca04`
- Shape: one universal `originlm` tarball with exactly five exact-version
  `@originlm/cli-*` optional dependencies.
- Platform tarballs carry native `manifest.json`, `sbom.cdx.json`, notices,
  repository `LICENSE` and full dependency license texts.
- Local proof on Linux x64: 34 packaging tests pass (1 Windows-only skip);
  `verify_npm_release.py` pass; fresh `createLocalAI()` consumer pass with
  `mode=child`, protocol `1.0`, zero connections/models/accounts, zero inference,
  empty home, empty PATH, no checkout and no `LAI_*` overrides.
- CI: rerun pending on `a0c18e5`.

## Python wheel qualification

Being implemented by an independent worker. Required evidence: real manylinux
baseline (not Ubuntu retagging), embedded qualified native helper, full
license/provenance metadata, wheel inspection, and fresh `Client.auto()` proof
with empty home, empty PATH, no checkout, no path override and no persistent
service.

## Cargo closure and Rust facade

Being implemented by an independent worker. Required evidence: every
intra-workspace dependency uses version + path in the publishable closure,
topological package order, actual MSRV verification, and public
session/turn/events/cancel/approval/close APIs through explicit existing grants
with no auto-authorization.

## Consumer targets

Worktrees and committed manifests will only be prepared after the SDK interfaces
are covered. Final consumer manifests must use registry packages, never git/path
overrides.

| App | Repo | Branch | Status |
| --- | --- | --- | --- |
| Remotif | `thomaslittle/remotif` | `main` | Not modified |
| Haiwire | `zomlit/xhw_haiwire` | `continuation/fedora-dev-runtime` | Not modified |
| Crntly | `thomaslittle/crntly-desktop` | `feature/gpui-native-rust-studio` | Not modified; general parity paused |

## Non-goals in this ledger

- No merge, tag, registry publish or consumer pin is claimed complete.
- No import-time download, silent persistent service, credential read, or
  automatic execution grant is acceptable.
