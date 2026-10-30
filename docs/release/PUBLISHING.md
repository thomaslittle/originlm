# Release preparation and publication

**This source preview is not published.** The `Prepare release artifacts` workflow
builds local-installable **unsigned** artifacts. It has read-only repository
permissions, uses no signing or registry secrets, and cannot publish. A tag does
not release anything.

## Prepare

```sh
gh workflow run release.yml --ref release/v0.1.0-alpha.20
```

The workflow also runs when a PR changes packaging or release scripts. Its five
native runners build the runtime helper for:

| Archive target | Build runner | Supported architecture | Recorded compatibility baseline |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | Linux x86_64, glibc | glibc version of the Ubuntu 24.04 build host; lower versions not qualified |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | Linux ARM64, glibc | glibc version of the Ubuntu 24.04 ARM build host; lower versions not qualified |
| `x86_64-apple-darwin` | `macos-15-intel` | macOS 15+ Intel | `MACOSX_DEPLOYMENT_TARGET=15.0` |
| `aarch64-apple-darwin` | `macos-15` | macOS 15+ Apple silicon | `MACOSX_DEPLOYMENT_TARGET=15.0` |
| `x86_64-pc-windows-msvc` | `windows-2025` | Windows x64 | Built and smoked on Windows Server 2025; older Windows versions are not independently qualified |

Musl/Alpine helpers, Windows ARM64 helpers and the GPUI desktop application are
**not** prepared or implied by the runtime bundles. Each archive records the exact
glibc/deployment-target baseline it was built against; do not read a build-host
baseline as a broader compatibility promise.

Each target runs before packaging:

1. `originlm --version` must equal the workspace version.
2. `originlm scan --json --home <empty temporary home>` must return live mode with
   no connections, models or accounts.
3. A bounded stdio `runtime.handshake`, passive `discovery.refresh` and
   `snapshot.get` must complete with protocol major `1` and an empty snapshot.

No provider inference, provider account, credential or inherited provider path is
used. The same smoke suite runs again against the helper extracted from the final
archive. Both JSON records are written to the non-hidden `prepared/evidence/`
directory so the artifact upload persists them; they include target, version,
source commit and pass status.

## Archive shape and member manifest

The helper archive is installer-compatible: the service installers extract only
`helper-<target>/originlm` (`.exe` on Windows).

```text
originlm-<target>.tar.gz        # Unix
originlm-<target>.zip           # Windows
  helper-<target>/originlm[.exe]
  LICENSE
  THIRD_PARTY_NOTICES.md
  sbom.cdx.json
  licenses/<crate>-<version>/<license text>
  manifest.json
```

The per-target `manifest.json` records:

- `version`, `target`, `platform` and exact `source.commit` (`$GITHUB_SHA`);
- Rust/cargo versions, pinned channel and compiler commit;
- build profile, locked dependencies and enabled features;
- `signed: false` plus explicit `authenticode: false`, `notarized: false` and
  `updateSignature: false`;
- the build-host glibc or macOS deployment target, and unsupported combinations;
- the full license closure (package name, version, SPDX expression, source and
  license-text paths);
- `members`, mapping every other archive path to byte length and SHA-256.

`SHA256SUMS` outside the archive records the archive itself:

```text
<sha256>  originlm-<target>.tar.gz
```

Checksums detect corruption; they do not authenticate the publisher. An unsigned
alpha cannot satisfy `LAI_MINISIGN_PUBKEY` or
`LAI_WINDOWS_SIGNER_THUMBPRINT`; those pins must reject it.

## License closure

The release archive is inventoried from the **actual `originlm-cli` normal/build
dependency closure for that target**, not every workspace crate. Cargo metadata
supplies package identity, source and SPDX expressions; Cargo's feature-resolved
tree excludes optional features enabled only by unrelated workspace members.
Collection follows explicit `license_file` entries, nested license files, and
workspace-shared `LICENSE`/`NOTICE` inheritance. It copies license texts verbatim,
preserves attribution, and **fails the build** if any required text or expression
is missing. It never invents a license string or downloads an unknown executable.

The local developer bundle uses the same collector for the Rust binaries it ships
and the four packed JavaScript packages. Its SBOM component count and license-text
directory count must match; development-only dependencies are no longer listed as
if they were shipped.

## Assembly and publication boundaries

The preparation workflow uploads one private-to-the-run Actions artifact per
target. Each contains the archive, `SHA256SUMS`, `manifest.json` and
`prepared/evidence/helper-binary-<target>.json` plus
`prepared/evidence/helper-archive-<target>.json`. To publish the unsigned alpha,
the release operation must:

1. Download every successful target artifact from the final candidate SHA.
2. Confirm each archive's `manifest.json` source commit equals that SHA and its
   source tree is clean.
3. Concatenate each per-target `SHA256SUMS` line without renaming archives or
   introducing a parallel flat-binary asset scheme.
4. Publish the GitHub prerelease with the platform archives, `SHA256SUMS`, and the
   matching per-target manifests; describe it as unsigned and prerelease.

The final release assets must all come from the same candidate SHA. The existing
x64 CI bundles are useful evidence but are not final release assets if their SHA
differs.

## Official package staging

The preparation workflow stages one **canonical universal** `originlm` npm
tarball, built once on Linux x64. Its `optionalDependencies` are exactly the five
supported `originlm-cli-<platform>` packages at the same exact version. Each
native target produces only its own platform tarball; the aggregate job verifies
the canonical SDK and every platform package's native manifest, helper, license
and SBOM hashes.

A post-aggregate consumer matrix then installs only `originlm` on Linux x64,
Linux ARM64, macOS Intel, macOS Apple silicon and Windows x64 using a temporary
local registry. It proves normal optional-dependency resolution, `createLocalAI()`
with an empty home, empty `PATH`, no checkout and no `LAI_*` overrides, a private
child helper, zero provider inference and no persistent service. An Ubuntu-only
aggregate check is not treated as all-platform consumer proof.

Actual npm/crates.io/PyPI publication is a separate authorized registry operation.
No registry credential or GitHub environment currently exists in this repository,
and the preparation workflow has read-only permissions. The workspace SDK keeps
`private: true` (release staging removes it only in the publishable copy).

The Rust facade closure (the normal/build dependencies of `originlm`) is marked
publishable; the CLI, GPUI desktop app, and the unpublished update helper keep
`publish = false`. `scripts/publish_order.py` derives the dependency-first order
from `cargo metadata`, fails on cycles or intra-workspace dependencies without
`version` and `path`, reports the declared MSRV, and can verify packaging without
uploading:

```sh
python scripts/publish_order.py --check
python scripts/publish_order.py --package
```

Consumer-repository pin changes, signing credentials and public package ownership
remain maintainer/root-release-worker operations.

## Qualified Python wheels (five supported platforms)

The separate `Qualify Python wheels` workflow (`.github/workflows/python-wheels.yml`)
is the authoritative wheel qualification for five publishable platforms. Linux
x86_64/ARM64 run in the official `quay.io/pypa/manylinux_2_28_*` images on native
runners; the helper is compiled **inside** the image and the wheel is tagged
`manylinux_2_28_<arch>` only there. A wheel built on Ubuntu keeps its host-specific
`linux_<arch>` tag and cannot be retagged; `stage_python_package.py` refuses a
`manylinux_*` tag unless `/opt/python`, an official-image OS identity and a glibc
no newer than the declared baseline are all present.

The native macOS Intel, macOS Apple-silicon and Windows x64 jobs build the helper
on their own native runner and produce `py3-none-macosx_15_0_<arch>` or
`py3-none-win_amd64` wheels. macOS uses the explicit
`MACOSX_DEPLOYMENT_TARGET=15.0` baseline; this is a declared minimum, not a claim
for older macOS. Windows x64 is built and smoke-tested on Windows Server 2025;
older Windows versions are not independently qualified. No Ubuntu wheel is
retagged as manylinux, and Linux `auditwheel show` is never applied to PE or
Mach-O wheels.

Preview staging keeps the `Private :: Do Not Upload` classifier, so ordinary
wheels remain unpublishable. Only an explicit `--release` run removes that
classifier, and it also requires the helper metadata directory produced by
`prepare_helper_archive.py`. The publishable wheel contains package data for:

```text
originlm/_bin/originlm[.exe]          # the qualified native helper
originlm/LICENSE                      # repository license
originlm/THIRD_PARTY_NOTICES.md       # generated dependency notices
originlm/licenses/<crate>-<version>/  # every dependency license text
originlm/sbom.cdx.json                # CycloneDX SBOM
originlm/manifest.json                # native helper provenance + member hashes
originlm/PROVENANCE.json              # wheel provenance and metadata pins
```

The native manifest member hashes are checked again against the wheel contents,
so the helper and license texts cannot drift from the archive that was qualified.
`scripts/verify_python_wheel.py` inspects the wheel's metadata, classifier, tag,
native helper format/architecture (ELF, PE or Mach-O), provenance, license
closure and SBOM; it runs `auditwheel show` only for manylinux wheels and
`twine check` for every release wheel. `scripts/python_consumer_smoke.py` then installs the wheel into a fresh
temporary virtual environment with `--no-index --no-deps` and runs
`Client.auto()` from a directory outside the checkout with an empty `HOME`, an
empty `PATH`, no `LAI_*` overrides and no shared broker grant. The proof requires
the helper to resolve through installed package data, a private child mode, an
empty live snapshot and zero provider inference. It also checks that no
persistent service was installed. Importing the SDK still performs no download.

Local qualification path (Docker required):

```sh
docker run --rm -v "$PWD:/work" -w /work \
  quay.io/pypa/manylinux_2_28_x86_64 bash -lc '
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.98.1
    . "$HOME/.cargo/env"
    export PYTHON=/opt/python/cp312-cp312/bin/python
    "$PYTHON" -m pip install "setuptools==84.1.0" "wheel==0.48.0" build auditwheel twine
    cargo build --release --locked -p originlm-cli --bin originlm
    "$PYTHON" scripts/prepare_helper_archive.py --binary target/release/originlm \
      --target x86_64-unknown-linux-gnu --out prepared/native \
      --source-commit "$(git rev-parse HEAD)" \
      --metadata-out prepared/metadata/x86_64-unknown-linux-gnu
    "$PYTHON" scripts/stage_python_package.py --binary target/release/originlm \
      --target x86_64-unknown-linux-gnu --plat-name manylinux_2_28_x86_64 \
      --metadata prepared/metadata/x86_64-unknown-linux-gnu --release --out wheels
    (cd wheels/originlm && "$PYTHON" setup.py bdist_wheel \
      --plat-name manylinux_2_28_x86_64 --dist-dir ../../prepared)
    "$PYTHON" scripts/verify_python_wheel.py \
      --wheel prepared/originlm-0.1.0a20-py3-none-manylinux_2_28_x86_64.whl \
      --version 0.1.0a20 --target x86_64-unknown-linux-gnu \
      --plat-name manylinux_2_28_x86_64 --release
    "$PYTHON" scripts/python_consumer_smoke.py \
      --wheel prepared/originlm-0.1.0a20-py3-none-manylinux_2_28_x86_64.whl \
      --version 0.1.0a20 --python "$PYTHON"
  '
```

A machine without Docker cannot produce or verify a manylinux claim; it can build
only a host-tagged preview wheel. Use the workflow's `prepared/evidence/` JSON as
the qualification record, not a locally retagged artifact. The workflow still
does not publish to PyPI, sign anything or modify consumer repositories.

## Packaged crate closure gate

`cargo package --no-verify` validates the source worktree, not the `.crate`
contents. The crate preparation and publisher jobs therefore run
`scripts/verify_packaged_closure.py` against the exact 13 prepared archives. It
verifies the manifest source/version/order and every archive hash, safely
extracts the archives outside the checkout, rejects normalized manifests with
`path` dependencies or leaked absolute workspace paths, and checks the unpacked
workspace with the declared MSRV. Internal dependencies resolve only to the
unpacked packages through temporary `[patch.crates-io]` entries. A fresh consumer
manifest depends only on the public `originlm` crate; the same bootstrap patches
map the unpublished closure to the reviewed unpacked crates. The gate resolves
external registry dependencies online once with the declared MSRV from a new
lockfile, then fetches and compiles that exact lock with `--locked --offline` for
both the workspace and the consumer. The generated lock hashes and resolved
registry versions are recorded in the evidence JSON. No source workspace lock is
copied into the extracted tree. No provider, credential or publication action is
involved.

## Maintainer gates for a signed/stable release

Before any public signed release, Thomas must choose registry names/version and
approve removing publication guards. The manylinux 2.28 wheel qualification
workflow must pass on the final candidate SHA for all five supported platforms;
each publishable wheel must keep the helper, license closure, SBOM and provenance
it verified. Provision Developer ID signing and
notarization, Windows Authenticode signing, and a checksum signing identity. Signed
helpers must be verified **before** they are embedded in npm packages, wheels and
service archives; a separate signing job whose outputs are unused is insufficient.
The publication workflow must fail closed when signing or verification fails,
consume the exact qualified artifacts, and publish dependencies in the order
obtained from the actual Cargo graph.

See [ADR-007](../design/ADR-007-release-preparation.md) for the decision and
compatibility sources. This document describes implemented preparation and
remaining release gates, not a claim that signing or publication has run.
