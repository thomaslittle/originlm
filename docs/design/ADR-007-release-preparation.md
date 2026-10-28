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
