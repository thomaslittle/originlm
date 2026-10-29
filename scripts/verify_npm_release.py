#!/usr/bin/env python3
"""Verify packed npm release tarballs before publication.

The release SDK must be one universal `originlm` tarball whose
`optionalDependencies` contain exactly the five supported platform packages at
the same exact version. Each platform tarball must carry the qualified native
manifest, SBOM, notices and full license texts from the helper archive.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import pathlib
import sys
import tarfile

PLATFORM_PACKAGES = (
    '@originlm/cli-linux-x64',
    '@originlm/cli-linux-arm64',
    '@originlm/cli-darwin-arm64',
    '@originlm/cli-darwin-x64',
    '@originlm/cli-win32-x64',
)


class VerifyNpmError(RuntimeError):
    pass


def _package(path):
    with tarfile.open(path, 'r:gz') as archive:
        try:
            member = archive.getmember('package/package.json')
        except KeyError:
            raise VerifyNpmError(f'{path.name} has no package/package.json') from None
        data = archive.extractfile(member).read()
        names = {item.name for item in archive.getmembers()}
    return json.loads(data.decode('utf-8')), names


def verify(packages, version, source_commit=None):
    packages = pathlib.Path(packages)
    tarballs = sorted(packages.glob('*.tgz'))
    sdk = [path for path in tarballs if not path.name.startswith('originlm-cli-')]
    platforms = [path for path in tarballs if path.name.startswith('originlm-cli-')]
    if len(sdk) != 1:
        raise VerifyNpmError(f'expected exactly one universal originlm SDK tarball, found {len(sdk)}')
    if len(platforms) < 1:
        raise VerifyNpmError('expected at least one platform tarball')
    manifest, names = _package(sdk[0])
    if manifest.get('name') != 'originlm':
        raise VerifyNpmError(f'SDK package name is {manifest.get("name")!r}')
    if manifest.get('version') != version:
        raise VerifyNpmError(f'SDK version is {manifest.get("version")!r}, expected {version!r}')
    if manifest.get('private') is True:
        raise VerifyNpmError('universal SDK still has private:true')
    optional = manifest.get('optionalDependencies')
    if not isinstance(optional, dict) or set(optional) != set(PLATFORM_PACKAGES):
        raise VerifyNpmError('SDK optionalDependencies are not the five supported platform packages')
    for name, pinned in optional.items():
        if pinned != version:
            raise VerifyNpmError(f'optional dependency {name} is {pinned!r}, expected {version!r}')
    for required in ('package/dist/index.js', 'package/node/auto.mjs', 'package/LICENSE', 'package/THIRD_PARTY_NOTICES.md'):
        if required not in names:
            raise VerifyNpmError(f'universal SDK tarball lacks {required}')

    report = []
    for path in platforms:
        platform, names = _package(path)
        name = platform.get('name')
        if name not in PLATFORM_PACKAGES:
            raise VerifyNpmError(f'unknown platform package {name!r}')
        if platform.get('version') != version:
            raise VerifyNpmError(f'{name} version is {platform.get("version")!r}, expected {version!r}')
        if not isinstance(platform.get('os'), list) or not isinstance(platform.get('cpu'), list):
            raise VerifyNpmError(f'{name} has no os/cpu constraints')
        for required in ('package/manifest.json', 'package/sbom.cdx.json', 'package/THIRD_PARTY_NOTICES.md', 'package/LICENSE'):
            if required not in names:
                raise VerifyNpmError(f'{name} lacks {required}')
        if not any(member.startswith('package/licenses/') for member in names):
            raise VerifyNpmError(f'{name} lacks dependency license texts')
        with tarfile.open(path, 'r:gz') as archive:
            native = json.loads(archive.extractfile('package/manifest.json').read().decode('utf-8'))
        if native.get('version') != version:
            raise VerifyNpmError(f'{name} native manifest version does not match')
        if source_commit and native.get('source', {}).get('commit') != source_commit:
            raise VerifyNpmError(f'{name} native manifest source commit does not match')
        report.append({
            'name': name,
            'version': version,
            'tarball': path.name,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'nativeTarget': native.get('target'),
            'licenseTexts': sum(1 for member in names if member.startswith('package/licenses/')),
        })
    return {
        'status': 'pass',
        'target': report[0]['nativeTarget'],
        'version': version,
        'sdk': sdk[0].name,
        'sdkSha256': hashlib.sha256(sdk[0].read_bytes()).hexdigest(),
        'optionalDependencies': optional,
        'platformPackages': report,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packages', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit')
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(args.packages, args.version, args.source_commit)
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (VerifyNpmError, OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
