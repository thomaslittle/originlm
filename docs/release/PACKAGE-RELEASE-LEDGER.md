# Official package release ledger

Updated 2026-11-22. This ledger records what is actually built, verified and
published; it does not claim consumer migration before that happens.

## Registry status

Re-checked 2026-11-22 against the public npm, crates.io and PyPI APIs, the GitHub
releases API and https://originlm.pages.dev/. `python scripts/release_dispatch.py
verify` repeats this check.

| Channel | Published state | Notes |
| --- | --- | --- |
| npm | `originlm@0.1.0-alpha.29` on both `alpha` and `latest` | Its optional dependencies are the `originlm-cli-linux-x64`, `-linux-arm64` and `-windows-x64` helpers at `0.1.0-alpha.29` (`alpha` tag). The helpers' own `latest` tag still reads `0.1.0-alpha.20`, which no install resolves because the SDK pins exact versions. `originlm-cli-darwin-x64`/`-arm64` stopped at `0.1.0-alpha.27`; macOS builds from source |
| crates.io | 14-crate closure at `0.1.0-alpha.29` | Published by `publish-crates.yml` run `45001371796` from `bce5fc1`; `originlm-update` is not published |
| PyPI | `originlm==0.1.0a29` | Three wheels: manylinux 2.28 x86_64/aarch64 and Windows x64. No macOS wheel |
| GitHub release | **No `v0.1.0-alpha.29` release** | Newest published prerelease is `v0.1.0-alpha.28` (no assets). Two stale drafts exist (`v0.1.0-alpha.24` and an untitled one) |
| Site | https://originlm.pages.dev/ shows `0.1.0-alpha.29` / `0.1.0a29` | Deployed from `feat/switchboard-v2`, which is not merged into `main` |

alpha.29 was prepared from `bce5fc1` (`chore(release): prepare 0.1.0-alpha.29`,
preparation runs `44995197266` release, `44995197185` wheels, `44995197115`
crates). Two channels were missed at first and dispatched by hand: npm `latest`
and crates.io. [RELEASE-CHECKLIST.md](RELEASE-CHECKLIST.md) and
`scripts/release_dispatch.py` exist so that cannot recur.

Publication uses the protected `release-npm`, `release-crates` and `release-pypi`
environments. The preparation workflows remain read-only and secret-free.

## Publication setup

The first publication was completed on 2026-10-30. Subsequent versions use the
gated publisher workflows. No secret belongs in the repository.

### npm

1. Confirm the intended npm account owns the unscoped `originlm` name and the
   `@originlm` scope, or is authorized to claim them.
2. Authenticate with `npm login` (interactive, 2FA) or create a granular
   automation token with publish permission for `originlm` and `@originlm/*`.
   Store it only as `NPM_TOKEN` in the protected release environment.
3. Publish platform packages first, then the universal SDK:
   `npm publish <platform.tgz> --access public` for each
   `originlm-cli-*`, then `npm publish originlm-<version>.tgz --access public`.
4. Verify with `npm view originlm@<version> optionalDependencies` and a clean
   `npm install originlm@<version>` consumer on each platform.

### crates.io

1. Confirm the intended crates.io account owns `originlm` and the 14-crate
   publishable closure, or is authorized to claim the names.
2. Authenticate interactively with `cargo login`, or use a scoped API token in
   `CARGO_REGISTRY_TOKEN` in the protected release environment. Do not commit
   `~/.cargo/credentials.toml`.
3. Run `python3 scripts/publish_order.py --check` and a dry run, then publish in
   the emitted order. Registry ownership must be checked before the first upload.
4. Verify each version is visible and that `cargo add originlm@<version>` resolves
   without path or git patches.

### PyPI

1. Confirm the intended PyPI account/project ownership for `originlm`.
2. Authenticate with an API token as `TWINE_USERNAME=__token__` and
   `TWINE_PASSWORD=<token>` in the protected release environment. Do not commit
   `.pypirc`.
3. Upload only the manylinux-qualified wheel after `auditwheel show` and
   `twine check`; verify with a clean `pip install originlm==<version>` and
   `Client.auto()` proof.

## npm universal SDK

- Branch: `release/full-official-packages`
- PR: [#14](https://github.com/thomaslittle/originlm/pull/14)
- Head: `64942f0d385912aaef6e24427e3de7d63d31673c`
- Shape: one universal `originlm` tarball with exactly five exact-version
  `originlm-cli-*` optional dependencies.
- Platform tarballs carry native `manifest.json`, `sbom.cdx.json`, notices,
  repository `LICENSE` and full dependency license texts.
- Local proof on Linux x64: 34 packaging tests pass (1 Windows-only skip);
  `verify_npm_release.py` pass; fresh `createLocalAI()` consumer pass with
  `mode=child`, protocol `1.0`, zero connections/models/accounts, zero inference,
  empty home, empty PATH, no checkout and no `LAI_*` overrides.
- CI: rerun pending on `a0c18e5`.

## Python wheel qualification

- Branch: `t3/python-wheel-qualification`
- PR: [#15](https://github.com/thomaslittle/originlm/pull/15)
- Head: `a4a5ccb02ac33e83f8c833d61f110d9bfed536d5` (stacked on PR14); manylinux run `40912541952` passed both targets, with `manylinux_2_28` wheels and passing consumer evidence.
- Adds a read-only manylinux_2_28 x86_64/aarch64 workflow using official
  `quay.io/pypa/manylinux_2_28_*` images, explicit `--release` staging that
  removes the private upload classifier only for publishable wheels, embedded
  native helper plus LICENSE/notices/SBOM/native manifest/license texts, wheel
  verification with `auditwheel show` and `twine check`, and a fresh
  `Client.auto()` venv proof with empty HOME/PATH, no `LAI_*`, no shared grant and
  no persistent service.
- Local evidence: 57 packaging tests, 40 SDK Python tests, 11 generation tests,
  `check_project.py` pass; host-tagged wheel built and smoke-tested. The real
  manylinux_2_28 build is being run in CI; it is not claimed until that job passes.
- The first manylinux aarch64 run failed on a Rust shim conflict before building.
  Isolated `RUSTUP_HOME`/`CARGO_HOME` and per-step exports now avoid the image
  shims; the follow-up workflow run is the authoritative manylinux proof.

## Cargo closure and Rust facade

- Branch: `t3/publish-facade-execution-api`
- PR: [#16](https://github.com/thomaslittle/originlm/pull/16)
- Head: `71c03abf525eac7aaa82520eac7538f8f183c407` (stacked on PR14; includes the npm evidence target fix, MSRV evidence gate, and evidence snapshot)
- 13 publishable crates; `originlm-cli`, `originlm-desktop` and `originlm-update`
  remain private.
- All intra-workspace dependencies in publishable crates use workspace version +
  path. `scripts/publish_order.py --check` emits a dependency-first order and
  `cargo package --no-verify --offline` succeeds for all 13.
- Public `originlm` facade now exposes session create/get/suspend/resume/update/
  close, turn start/cancel, cursor event reads/subscriptions, approval response,
  explicit `authorize_execution`, and idempotent `close`. Registration is not a
  grant; shared/child modes cannot be granted through the facade.
- Actual declared MSRV evidence is durable in workflow `Verify declared MSRV`
  (run `40913002077`, artifact `msrv-1.88-evidence`) on current head `71c03ab`.
  The artifact tested GitHub PR merge SHA `95aa965d` with rustc/cargo 1.88.0,
  exit 0 and the exact command `cargo +1.88.0 check --workspace --exclude
  originlm-desktop --all-targets --locked`, with the full compiler log.
- An earlier snapshot for head `2bed31f` (tested merge `61dcb65d`) is committed at
  `docs/release/evidence/msrv-1.88.json`; the current run artifact is the
  authoritative evidence for `71c03ab`.

## Consumer targets

Worktrees and committed manifests will only be prepared after the SDK interfaces
are covered. Final consumer manifests must use registry packages, never git/path
overrides.

| App | Repo | Branch | Status |
| --- | --- | --- | --- |
| Remotif | `thomaslittle/remotif` | `main` | Not modified |
| Haiwire | `zomlit/xhw_haiwire` | `continuation/fedora-dev-runtime` | PR #13; builds against the published discovery crates; Linux E2E passed |
| Crntly | `thomaslittle/crntly-desktop` | `feature/gpui-native-rust-studio` | Not modified; general parity paused |

## Publisher workflows (PR17)

- Branch `release/publishers`, draft PR
  [#17](https://github.com/thomaslittle/originlm/pull/17), head
  `4e8e77a4c1f4879db9dd404149943999e778b19d`.
- `.github/workflows/publish-npm.yml` — environment `release-npm`, npm trusted
  publishing, verifies the assembled `npm-release` set and publishes platform
  packages before the universal SDK.
- `.github/workflows/prepare-crates.yml` + `publish-crates.yml` — environment
  `release-crates`; 14-crate order, prepared `.crate` hashes, bootstrap
  `CARGO_REGISTRY_TOKEN` or post-bootstrap OIDC trusted publishing.
- `.github/workflows/publish-pypi.yml` — environment `release-pypi`, PyPI
  trusted publishing for the two qualified manylinux wheels only.
- All publishers are manual, default to `dry_run=true`, require
  `confirm=PUBLISH` and environment approval, and reject artifacts not built by
  a successful integrated-`main` run at the exact `source_commit`.

## Non-goals in this ledger

- No merge, tag, registry publish or consumer pin is claimed complete.
- No import-time download, silent persistent service, credential read, or
  automatic execution grant is acceptable.
