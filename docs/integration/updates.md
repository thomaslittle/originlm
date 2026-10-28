# Signed runtime updates

The runtime is a per-user executable shared by several applications. An update
must be authenticated, compatible, staged, coordinated with active sessions,
and reversible. A checksum detects corruption; it is **not** publisher
authentication. Nothing in this repository publishes, signs, installs, or
downloads an update.

## Pipeline stages

1. **Upstream detection.** A watcher reports provider-relevant changes. It
   never edits adapters, moves the pin, or opens an unreviewed merge.
2. **Reviewed adaptation.** A maintainer ports facts into our normalized
   adapters, with source revision, path, license, provider/platform, rule, and
   fixture evidence.
3. **Qualification.** Cross-platform gates run; known-good pins stay until the
   replacement passes.
4. **Release preparation.** `python scripts/build_release.py --profile release`
   creates a checksummed bundle, `manifest.json`, `update-manifest.unsigned.json`,
   and `latest.json`. The update manifest points at a separate single helper
   executable (`originlm-<version>-<platform>[.exe]`), not the full bundle
   archive; a future extractor can stage more complex payloads, but the current
   verifier/activation contract is intentionally a single executable.
   `python scripts/prepare_update.py ARCHIVE --version ...` can prepare a
   manifest for an existing single-executable archive.
5. **Approval and signing.** An approved external signer signs the canonical
   manifest. Signing credentials are not created or stored by this repository.
6. **Installation.** `originlm-update` verifies the signature, platform, protocol
   family, version monotonicity, archive size, SHA-256, and the exact copied executable bytes before
   activation. It never installs a second, unsigned extracted file.
7. **Activation and rollback.** Activation holds an exclusive session lease and
   an activation lock, backs up the previous executable, replaces atomically,
   and keeps the backup for explicit rollback. A session marker older than 24
   hours is treated as crash residue, renamed to `*.stale.json`, and no longer
   blocks activation; the file is preserved as evidence.
8. **Live refresh.** Provider data is refreshed by the new runtime; the update
   itself does not read credentials or run inference.

## Manifest contract

Canonical signing input is compact JSON with sorted keys from
`originlm_update::canonical_manifest`, excluding the `signature` field. The manifest
includes `schemaVersion`, `version`, `channel`, `platform`, `protocolVersion`,
`archive`, `archiveBytes`, `archiveSha256`, `publishedAt`, optional `notesUrl`,
and a Base64 Ed25519 `signature`. A public key is pinned by the installing host,
not fetched with the update.

## Current gates and blockers

- No signing key, certificate, release signature, registry publication, or
  persistent update endpoint has been created.
- `build_release.py` writes an **unsigned** manifest and marks it as such.
- The verifier, staging, session lease, activation, and rollback are
  implemented and fixture-tested. A real signed update has not been installed.
- Deploying an update endpoint and approving a publisher key are separate
  maintainer decisions.

## Rollback

`originlm-update::rollback(target, backup)` restores the exact backup and retains that
backup file. It replaces the current target; it does not preserve the failed version. If activation fails
before replacement, the current executable is untouched. If the replacement
fails, the previous binary is still present and the caller can roll back
without re-downloading anything.

## Compatibility

Protocol 1.x remains additive. Provider-adapter fixes that do not change the
wire contract do not require simultaneous consumer releases. A schema-version
bump in SQLite fails closed for older runtimes and requires a tested migration.
