#!/usr/bin/env python3
"""Aggregate all five target npm artifacts into one verified release set.

The universal SDK is produced by every prepare job. This script requires those
tarballs to be byte-identical, requires exactly one tarball for each of the five
platform packages, copies the canonical set to a release directory, verifies the
native member hashes, and writes SHA256SUMS plus an aggregate manifest.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import sys

import verify_npm_release


class AssembleError(RuntimeError):
    pass


def _sha256(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def _package_name(path):
    manifest, _ = verify_npm_release._package(path)
    name = manifest.get('name')
    version = manifest.get('version')
    if not isinstance(name, str) or not isinstance(version, str):
        raise AssembleError(f'{path.name} has no package name/version')
    return name, version


def assemble(artifacts, out, version, source_commit=None):
    artifacts = pathlib.Path(artifacts).resolve()
    if not artifacts.is_dir():
        raise AssembleError('artifact directory is unavailable')
    tarballs = sorted(artifacts.rglob('*.tgz'))
    if not tarballs:
        raise AssembleError('no npm tarballs found in downloaded artifacts')
    sdks = []
    platforms = {}
    for path in tarballs:
        name, package_version = _package_name(path)
        if name == 'originlm':
            sdks.append((path, package_version, _sha256(path)))
        elif name in verify_npm_release.PLATFORM_PACKAGES:
            if name in platforms:
                raise AssembleError(f'duplicate platform package artifact: {name}')
            platforms[name] = (path, package_version, _sha256(path))
    if len(sdks) < 1:
        raise AssembleError('no universal originlm SDK tarball found')
    if len({digest for _, _, digest in sdks}) != 1:
        raise AssembleError(
            'universal SDK tarballs differ across target artifacts:\n'
            + '\n'.join(f'- {path}: {digest}' for path, _, digest in sdks)
        )
    if any(package_version != version for _, package_version, _ in sdks):
        raise AssembleError('universal SDK version does not match the release version')
    if set(platforms) != set(verify_npm_release.PLATFORM_PACKAGES):
        missing = sorted(set(verify_npm_release.PLATFORM_PACKAGES) - set(platforms))
        extra = sorted(set(platforms) - set(verify_npm_release.PLATFORM_PACKAGES))
        raise AssembleError(f'platform package set is incomplete: missing={missing} extra={extra}')
    if any(package_version != version for _, package_version, _ in platforms.values()):
        raise AssembleError('platform package version does not match the release version')

    out = pathlib.Path(out)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    sdk_path, _, sdk_sha = sdks[0]
    canonical_sdk = out / sdk_path.name
    shutil.copy2(sdk_path, canonical_sdk)
    for name in verify_npm_release.PLATFORM_PACKAGES:
        path, _, _ = platforms[name]
        shutil.copy2(path, out / path.name)

    evidence = verify_npm_release.verify(out, version, source_commit, require_all=True)
    sums = out / 'SHA256SUMS'
    lines = []
    for path in sorted(out.glob('*.tgz')):
        lines.append(f'{_sha256(path)}  {path.name}\n')
    sums.write_text(''.join(lines), encoding='utf-8')
    manifest = {
        'schemaVersion': 1,
        'kind': 'originlm-npm-release-set',
        'version': version,
        'sourceCommit': source_commit,
        'signed': False,
        'sdk': {
            'tarball': canonical_sdk.name,
            'sha256': sdk_sha,
            'identicalCopies': [
                {'artifactPath': str(path), 'sha256': digest}
                for path, _, digest in sdks
            ],
        },
        'optionalDependencies': evidence['optionalDependencies'],
        'platformPackages': evidence['platformPackages'],
        'sha256Sums': 'SHA256SUMS',
    }
    (out / 'manifest.json').write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + '\n', encoding='utf-8'
    )
    return {
        'status': 'pass',
        'version': version,
        'sourceCommit': source_commit,
        'sdk': canonical_sdk.name,
        'sdkSha256': sdk_sha,
        'sdkCopies': len(sdks),
        'platformPackages': len(platforms),
        'out': str(out),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', type=pathlib.Path, required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit')
    args = parser.parse_args()
    print(json.dumps(assemble(args.artifacts, args.out, args.version, args.source_commit), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (AssembleError, verify_npm_release.VerifyNpmError, OSError, ValueError, KeyError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
