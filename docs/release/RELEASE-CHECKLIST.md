# Release checklist

Every release reaches five places: npm (`alpha` and `latest`), crates.io, PyPI,
the GitHub prerelease and the site. The registry publishers are manual
`workflow_dispatch` workflows, so a forgotten dispatch silently skips a channel
(alpha.29 first missed npm `latest` and crates.io). Work through this list in
order and tick every line. Background is in [PUBLISHING.md](PUBLISHING.md) and
[REGISTRY-PUBLISHING.md](REGISTRY-PUBLISHING.md).

## 1. Prepare

- [ ] Merge the `chore(release): prepare <version>` PR into `main`. It bumps the
      workspace version in `Cargo.toml`, every crate's internal pins and
      `Cargo.lock`, `package.json`, `packages/sdk-python/pyproject.toml`, the
      `CHANGELOG.md` entry and the install docs.
- [ ] The version-file push starts three preparation workflows on that commit.
      Wait until all three succeed:
      `Prepare release artifacts` (`release.yml`),
      `Qualify Python wheels` (`python-wheels.yml`) and
      `Prepare crate release set` (`prepare-crates.yml`).

## 2. Publish the registries

- [ ] `python scripts/release_dispatch.py plan` and read it. It resolves the
      release commit (the newest `main` commit whose `release.yml` run produced
      artifacts), the three preparation run IDs and the publisher inputs, and
      shows which channels are already live. It dispatches nothing.
- [ ] `python scripts/release_dispatch.py dispatch` fires every missing
      publisher as a dry run: `publish-npm.yml` (`alpha`), `publish-npm.yml`
      promotion to `latest`, `publish-pypi.yml` and `publish-crates.yml`.
      Approve each run at its `release-npm`, `release-pypi` or `release-crates`
      environment and confirm every run is green.
- [ ] `python scripts/release_dispatch.py dispatch --publish` fires the same
      publishers with `dry_run=false` and `confirm=PUBLISH`. Approve each
      environment again. The npm promotion to `latest` is queued behind the
      `alpha` publish (same concurrency group) and needs its own approval.

## 3. GitHub prerelease

- [ ] Open the Release Drafter draft, set the tag to `v<version>` on the
      release commit, mark it as a prerelease and describe it as unsigned.
- [ ] Attach the assets from the `release.yml` run, all from that one commit:
      every `originlm-<target>` archive, its `manifest.json` and the
      concatenated `SHA256SUMS` (see "Assembly and publication boundaries" in
      PUBLISHING.md). Publish the release; that creates the tag.
- [ ] Delete stale drafts for earlier versions.

## 4. Site

- [ ] Rebuild and deploy https://originlm.pages.dev/. The build reads the live
      npm, PyPI and crates.io versions (`apps/site/scripts/fetch_versions.mjs`
      on `feat/switchboard-v2`, the branch the site deploys from), so it must
      run after step 2.

## 5. Verify and record

- [ ] `python scripts/release_dispatch.py verify` exits 0: npm `alpha`,
      `latest` and helpers, crates.io, PyPI, the GitHub release and the site all
      show the version.
- [ ] Clean consumer checks: `npm install originlm@<version>`,
      `pip install originlm==<pep440 version>` and `cargo add originlm@=<version>`.
- [ ] Update the registry table in
      [PACKAGE-RELEASE-LEDGER.md](PACKAGE-RELEASE-LEDGER.md) with the observed
      state and the run IDs.
