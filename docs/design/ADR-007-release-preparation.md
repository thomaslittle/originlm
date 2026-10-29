# ADR-007: prepare artifacts without implicit publication

Status: accepted, 2026-10-28.

The inherited release workflow could publish on a tag, skipped missing signing
credentials, consumed unsigned binaries even after a signing job, advertised
unsupported cross targets, and labeled Ubuntu-built wheels as manylinux 2.17.
The SDK is deliberately private and the Rust crates prohibit publishing. A
workflow must not turn those facts into a successful-looking public release.

The v1 workflow prepares unsigned, checksummed artifacts on five native runners:
Linux x64/arm64, macOS x64/arm64 and Windows x64. Archive names and members match
the service installers; Windows uses ZIP and an `.exe`. SDK staging preserves
private publication guards and produces installable local tarballs. Python wheels
use py3-none plus a conservative platform tag; Linux artifacts remain host-specific
until built and qualified against a manylinux baseline. No musl or Windows arm64
artifact is implied by the supported SDK resolver.

Preparation has read-only repository permissions and no secrets. It never publishes,
signs or deploys, including on a tag. Publication is a separate maintainer release
operation after reviewing package names, compatibility evidence, certificates and
registry ownership. This removes unsafe, nonfunctional automation instead of hiding
it behind a job that can silently skip its safety conditions.

Tradeoff: this branch provides reproducible preview artifacts, not a one-click public
release. A later publication workflow must consume the final signed binaries before
packing SDKs, notarize macOS, verify Windows Authenticode, sign the checksum manifest,
and fail closed for every missing requirement. None of those guarantees is claimed
for this preview.

Sources reviewed 2026-10-28:
- [GitHub runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Python platform compatibility tags](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/)

## Amendment: `v0.1.0-alpha.20` unsigned runtime prerelease (2026-10-29)

The first official prerelease is explicitly unsigned. Its preparation workflow
adds evidence that the earlier preparation path lacked:

- native `--version`, empty-home passive scan and bounded stdio handshake gates
  before packaging and again against the extracted archive member;
- exact source commit, target, pinned Rust/cargo versions and compiler commit,
  build profile and compatibility baseline in each archive manifest;
- a per-target `originlm-cli` normal/build dependency closure derived from Cargo
  metadata and Cargo's feature-resolved tree, not every workspace crate;
- complete dependency license texts, including explicit `license_file`, nested
  license files and workspace-shared inheritance, with a hard failure when a
  required text or expression is missing;
- an archive member manifest with path, byte length and SHA-256 for the helper,
  repository `LICENSE`, notices, SBOM and license texts.

Native smoke and archive-verification evidence is persisted under the non-hidden
`prepared/evidence/` path so the Actions artifact upload includes it; hidden
directories are not relied on.

Archives remain `helper-<target>/originlm[.exe]`-compatible for the service
installers. `signed: false` and `authenticode`/`notarized`/`updateSignature:
false` are recorded; no Authenticode, notarization or trusted update signature is
claimed. npm, crates.io and PyPI publication remain disabled for this alpha.
Linux archives record the glibc baseline of their Ubuntu 24.04 build host and do
not claim manylinux or an older-glibc guarantee. macOS artifacts target macOS 15;
Windows ARM64, musl and the GPUI desktop binary are not part of the runtime
bundles.

The preparation workflow still has read-only repository permissions and cannot
publish. The release operation must use assets from the final candidate SHA and
concatenate per-target `SHA256SUMS` without introducing a parallel flat asset
scheme.

## Amendment: packaged crate closure verification (2026-10-29)

Crate preparation now verifies the packaged archives, not only the source
worktree. The exact 13 `.crate` files are hash-bound to the source commit and
order, safely unpacked outside the checkout, checked for normalized path
dependencies and source-path leakage, compiled as a temporary workspace using
only the unpacked packages, and consumed by a fresh manifest whose only public
dependency is `originlm`. The declared MSRV toolchain is used for this packaged
check. Evidence is attached to the crate artifact and publisher gate; no
registry upload, credential or provider action is involved.

## Amendment: five-platform Python wheel qualification (2026-10-29)

A separate read-only workflow (`.github/workflows/python-wheels.yml`) produces the
five publishable wheels. Linux x86_64/ARM64 compile the helper inside the official
`quay.io/pypa/manylinux_2_28_*` images and build
`py3-none-manylinux_2_28_<arch>` wheels there; Ubuntu-built host wheels keep
`linux_<arch>` and are never retagged. macOS Intel/Apple-silicon and Windows x64
build native `macosx_15_0_<arch>` and `win_amd64` wheels on their own runners.
Publishable staging requires an explicit `--release` flag and the
license/SBOM/provenance metadata from the helper archive. Every wheel passes
native helper smoke, format/architecture checks, `twine check`, and a fresh
empty-home `Client.auto()` consumer proof; Linux additionally passes `auditwheel
show`. No publication, signing or registry credential is added.
