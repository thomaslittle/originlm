# Release preparation and publication

**This source preview is not published.** The `Prepare release artifacts` workflow
builds local-installable unsigned artifacts. It has read-only repository permissions,
uses no signing or registry secrets, and cannot publish. A tag does not release anything.

## Prepare

```sh
gh workflow run release.yml --ref feat/v1-principal-review
```

The workflow also runs when a PR changes packaging scripts. Its five native runners
build Linux x64/arm64, macOS x64/arm64 and Windows x64. Each uploads a private-to-the-run
Actions artifact containing an installer-compatible helper archive, SHA256SUMS,
source/size manifest, npm tarballs and a Python wheel. Actions artifact uploads are
not registry publication. Review the actual run before describing a platform as tested.

The helper budget is 40 MiB, checked against exact bytes. Archive names are
`originlm-<target>.tar.gz` (Unix) or `.zip` (Windows), with one member under
`helper-<target>/`. Checksums detect corruption; they do not authenticate the publisher.

## Compatibility and package boundaries

- npm staging builds the SDK first. `--preview` allows packing a private package but
  preserves `private: true`; ordinary staging refuses it. Install the matching helper
  tarball alongside the SDK tarball so optional dependency resolution stays local.
- Python wheels contain Python source and a native executable, so their tag is
  `py3-none-<platform>`, not a CPython-specific ABI. The private upload classifier stays.
- Linux wheels have `linux_x86_64` or `linux_aarch64` tags and are **local preview
  artifacts for compatible hosts**, not PyPI-ready manylinux wheels. Ubuntu 24.04 is
  the current build host. No glibc 2.17 compatibility is claimed.
- macOS artifacts conservatively target macOS 15; Windows artifacts target x64.
  Musl and Windows arm64 packages are not prepared by this workflow.
- Rust crates retain `publish = false`. The repository's path-dependency examples
  are the supported Rust source preview; there is no claim that `cargo add` works yet.

## Maintainer release gates

Before any public release, Thomas must choose the registry names/version and approve
removing publication guards. Qualify a Linux portability baseline in an appropriate
manylinux build environment. Provision Developer ID signing and notarization, Windows
Authenticode signing, and a checksum signing identity. Signed helpers must be verified
**before** they are embedded in npm packages, wheels and service archives; a separate
signing job whose outputs are unused is insufficient.

Registry credentials/Trusted Publishing, signing identities, domain and hosting are
maintainer-owned. No secret is needed for local development or artifact preparation.
The future publication workflow must fail closed when signing or verification fails,
consume the exact qualified artifacts, and publish dependencies in the order obtained
from the actual Cargo graph. The previous hard-coded publish loop was not valid.

See [ADR-007](../design/ADR-007-release-preparation.md) for the decision and official
compatibility sources. This document describes implemented preparation and remaining
release gates, not a claim that signing or publication has run.
