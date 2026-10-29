# Official package release ledger

Updated 2026-10-29. This ledger records what is actually built and verified; it
does not claim registry publication or consumer migration before those happen.

## Registry status

| Registry | Package/name state | Credential state |
| --- | --- | --- |
| npm | `originlm` and `originlm-cli-*` unclaimed (`404` at check time) | No local npm login (`ENEEDAUTH`); no GitHub repository secret/environment |
| crates.io | Not published | No local cargo credentials file; no GitHub repository secret/environment |
| PyPI | `originlm` unclaimed (`404` at check time) | No local `.pypirc` or twine configuration; no GitHub repository secret/environment |

Actual publication must use maintainer/root-worker credentials in a protected
environment. No credential was created, read or exposed.

## Exact safe publication setup still required

Do not paste secrets into chat or commit them. Use the registry's own login flow
on a maintainer-controlled machine, or add repository/environment secrets for a
protected release environment. The preparation workflow itself must remain
read-only.

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

1. Confirm the intended crates.io account owns `originlm` and the 13-crate
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
| Haiwire | `zomlit/xhw_haiwire` | `continuation/fedora-dev-runtime` | Not modified |
| Crntly | `thomaslittle/crntly-desktop` | `feature/gpui-native-rust-studio` | Not modified; general parity paused |

## Publisher workflows (PR17)

- Branch `release/publishers`, draft PR
  [#17](https://github.com/thomaslittle/originlm/pull/17), head
  `4e8e77a4c1f4879db9dd404149943999e778b19d`.
- `.github/workflows/publish-npm.yml` — environment `release-npm`, npm trusted
  publishing, verifies the assembled `npm-release` set and publishes platform
  packages before the universal SDK.
- `.github/workflows/prepare-crates.yml` + `publish-crates.yml` — environment
  `release-crates`; 13-crate order, prepared `.crate` hashes, bootstrap
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
