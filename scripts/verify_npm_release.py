#!/usr/bin/env python3
"""Verify packed npm release tarballs before publication.

The release SDK must be one universal `originlm` tarball whose
`optionalDependencies` contain exactly the five supported platform packages at
the same exact version. Each platform tarball must carry the qualified native
manifest, SBOM, notices and full license texts from the helper archive, and its
binary/license bytes must match the hashes in that native manifest.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import tarfile

PLATFORM_PACKAGES = (
    'originlm-cli-linux-x64',
    'originlm-cli-linux-arm64',
    'originlm-cli-darwin-arm64',
    'originlm-cli-darwin-x64',
    'originlm-cli-windows-x64',
)


class VerifyNpmError(RuntimeError):
    pass


def _package(path):
    files = {}
    with tarfile.open(path, 'r:gz') as archive:
        for item in archive.getmembers():
            if item.isdir():
                continue
            if not item.isfile():
                raise VerifyNpmError(f'{path.name} has a non-regular member: {item.name}')
            source = archive.extractfile(item)
            if source is None:
                raise VerifyNpmError(f'{path.name} has an unreadable member: {item.name}')
            files[item.name] = source.read()
    try:
        manifest = json.loads(files['package/package.json'].decode('utf-8'))
    except KeyError:
        raise VerifyNpmError(f'{path.name} has no package/package.json') from None
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise VerifyNpmError(f'{path.name} has invalid package.json') from error
    return manifest, files


def _hash(data):
    return hashlib.sha256(data).hexdigest()


def _native_member(files, record, path):
    if path not in files:
        raise VerifyNpmError(f'npm package lacks native manifest member {path}')
    data = files[path]
    if len(data) != record.get('bytes') or _hash(data) != record.get('sha256'):
        raise VerifyNpmError(f'npm package member does not match native manifest: {path}')


def verify(packages, version, source_commit=None, require_all=False):
    packages = pathlib.Path(packages)
    tarballs = sorted(packages.glob('*.tgz'))
    sdk = [path for path in tarballs if not path.name.startswith('originlm-cli-')]
    platforms = [path for path in tarballs if path.name.startswith('originlm-cli-')]
    if len(sdk) != 1:
        raise VerifyNpmError(f'expected exactly one universal originlm SDK tarball, found {len(sdk)}')
    if len(platforms) < 1:
        raise VerifyNpmError('expected at least one platform tarball')
    manifest, files = _package(sdk[0])
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
        if required not in files:
            raise VerifyNpmError(f'universal SDK tarball lacks {required}')

    report = []
    seen = set()
    for path in platforms:
        platform, files = _package(path)
        name = platform.get('name')
        if name not in PLATFORM_PACKAGES:
            raise VerifyNpmError(f'unknown platform package {name!r}')
        if name in seen:
            raise VerifyNpmError(f'duplicate platform package {name!r}')
        seen.add(name)
        if platform.get('version') != version:
            raise VerifyNpmError(f'{name} version is {platform.get("version")!r}, expected {version!r}')
        if not isinstance(platform.get('os'), list) or not isinstance(platform.get('cpu'), list):
            raise VerifyNpmError(f'{name} has no os/cpu constraints')
        for required in ('package/manifest.json', 'package/sbom.cdx.json', 'package/THIRD_PARTY_NOTICES.md', 'package/LICENSE'):
            if required not in files:
                raise VerifyNpmError(f'{name} lacks {required}')
        if not any(member.startswith('package/licenses/') for member in files):
            raise VerifyNpmError(f'{name} lacks dependency license texts')
        try:
            native = json.loads(files['package/manifest.json'].decode('utf-8'))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise VerifyNpmError(f'{name} native manifest is invalid') from error
        target = native.get('target')
        if native.get('version') != version:
            raise VerifyNpmError(f'{name} native manifest version does not match')
        if source_commit and (native.get('source') or {}).get('commit') != source_commit:
            raise VerifyNpmError(f'{name} native manifest source commit does not match')
        members = native.get('members')
        if not isinstance(members, dict) or not members:
            raise VerifyNpmError(f'{name} native manifest has no member checksums')
        executable = 'originlm.exe' if str(target).endswith('windows-msvc') else 'originlm'
        helper_record = members.get(f'helper-{target}/{executable}')
        if not isinstance(helper_record, dict):
            raise VerifyNpmError(f'{name} native manifest lacks the helper member')
        _native_member(files, helper_record, f'package/bin/{executable}')
        for member in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'sbom.cdx.json'):
            _native_member(files, members.get(member) or {}, f'package/{member}')
        license_texts = 0
        for member, record in sorted(members.items()):
            if not member.startswith('licenses/'):
                continue
            _native_member(files, record, f'package/{member}')
            license_texts += 1
        if license_texts == 0:
            raise VerifyNpmError(f'{name} native manifest carries no license-text hashes')
        report.append({
            'name': name,
            'version': version,
            'tarball': path.name,
            'sha256': _hash(path.read_bytes()),
            'nativeTarget': target,
            'licenseTexts': license_texts,
            'helperSha256': helper_record.get('sha256'),
            'helperBytes': helper_record.get('bytes'),
        })
    if require_all and seen != set(PLATFORM_PACKAGES):
        missing = sorted(set(PLATFORM_PACKAGES) - seen)
        raise VerifyNpmError(f'missing platform packages: {missing}')
    targets = sorted({item['nativeTarget'] for item in report})
    return {
        'status': 'pass',
        'target': targets[0] if len(targets) == 1 else None,
        'targets': targets,
        'version': version,
        'sdk': sdk[0].name,
        'sdkSha256': _hash(sdk[0].read_bytes()),
        'optionalDependencies': optional,
        'platformPackages': sorted(report, key=lambda item: item['name']),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packages', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit')
    parser.add_argument('--require-all', action='store_true')
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(args.packages, args.version, args.source_commit, args.require_all)
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
