# Official registry publishers

These workflows are manual and non-publishing by default. They publish only
immutable artifacts built by a successful integrated-`main` preparation run at
the exact commit supplied as `source_commit`. A GitHub Environment with a
required reviewer is the second gate; `confirm=PUBLISH` is the third.

## Workflow and environment names

| Registry | Publisher workflow | Artifact source | Environment |
| --- | --- | --- | --- |
| npm | `.github/workflows/publish-npm.yml` | `release.yml` → `npm-release` | `release-npm` |
| crates.io | `.github/workflows/publish-crates.yml` | `.github/workflows/prepare-crates.yml` → `crates-release` | `release-crates` |
| PyPI | `.github/workflows/publish-pypi.yml` | `.github/workflows/python-wheels.yml` → `python-wheel-*` | `release-pypi` |

Create each environment in GitHub repository settings with **required
reviewers** (and, preferably, a deployment branch policy limited to `main`).
The preparation workflows stay read-only and secret-free.

## npm trusted publishing

Configure a trusted publisher for **each** of the six packages:

- `originlm`
- `@originlm/cli-linux-x64`
- `@originlm/cli-linux-arm64`
- `@originlm/cli-darwin-arm64`
- `@originlm/cli-darwin-x64`
- `@originlm/cli-win32-x64`

Fields on npmjs.com:

- Organization or user: `thomaslittle`
- Repository: `originlm`
- Workflow filename: `publish-npm.yml`
- Environment name: `release-npm`
- Allowed action: `npm publish`

The publish job requests `id-token: write` and uses npm CLI `11.5.1` or newer.
npm automatically generates provenance for public packages. The `repository`
field in each package manifest must resolve to the GitHub repository
`https://github.com/thomaslittle/originlm`.

The workflow publishes platform packages first, then the one universal SDK. It
uses the workflow input `dist_tag` (default `alpha`) because this is a
prerelease; set `latest` only after the stable release is approved.

Official reference: <https://docs.npmjs.com/trusted-publishers/>

## crates.io

Crates.io trusted publishing **cannot bootstrap a crate that does not yet
exist**. The first publication of each of the 13 crates therefore uses a
scoped crates.io API token stored as the `release-crates` environment secret
`CARGO_REGISTRY_TOKEN`. After each crate exists and is owned by the intended
account, configure its trusted publisher:

- Repository: `thomaslittle/originlm`
- Workflow filename: `publish-crates.yml`
- Environment name: `release-crates`

Then rerun the publisher with `auth_mode=trusted`; the workflow exchanges a
GitHub OIDC token through `rust-lang/crates-io-auth-action@v1` and never needs a
long-lived token. The closure order is fixed and derived from Cargo metadata:

```text
originlm-contracts
originlm-platform
originlm-core
originlm-chatgpt-auth
originlm-discovery
originlm-runtime
originlm-store
originlm-service
originlm-transports
originlm-control
originlm-adapters
originlm-kit
originlm
```

The publisher verifies `crates-release/SHA256SUMS`, the source commit, the exact
13-crate order, and that a fresh `cargo package` reproduces every prepared
`.crate` hash before publishing. Unrelated private crates (`originlm-cli`,
`originlm-desktop`, `originlm-update`) are not in the artifact.

Official references: <https://crates.io/docs/trusted-publishing> and the
[crates.io trusted-publishing RFC](https://github.com/rust-lang/rfcs/blob/master/text/3691-trusted-publishing-cratesio.md).

## PyPI trusted publishing

Create a pending trusted publisher for the `originlm` project:

- Owner: `thomaslittle`
- Repository: `originlm`
- Workflow filename: `publish-pypi.yml`
- Environment name: `release-pypi`

No API token is needed. The workflow requests `id-token: write` and uses
`pypa/gh-action-pypi-publish@release/v1`; it refuses to upload unless both
manylinux_2_28 wheels and their `python-wheel`/`python-consumer` evidence are
present and pass `auditwheel show` and `twine check`.

Official reference: <https://docs.pypi.org/trusted-publishers/using-a-publisher/>.

## Operation

1. Merge the prepared package changes and run the preparation workflows on the
   final integrated `main` commit.
2. Complete the package/project trusted-publisher setup above. For crates.io's
   first publication, add `CARGO_REGISTRY_TOKEN` to `release-crates`.
3. Run each publisher with `dry_run=true` and the preparation run ID plus full
   `source_commit`; inspect the summary.
4. Re-run with `dry_run=false`, `confirm=PUBLISH`, approve the environment, and
   publish. For crates.io use `auth_mode=token` for the bootstrap release, then
   configure trusted publishing and use `auth_mode=trusted` for later versions.

No workflow triggers on tags or pushes. No publisher runs without a human
dispatch, environment approval and explicit confirmation. Do not put credentials
in issues, pull requests or chat; use the GitHub environment secret UI only.
