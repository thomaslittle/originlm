#!/usr/bin/env python3
"""Verify a prepared helper archive and smoke-test the extracted executable.

This checks the exact member manifest (paths, byte lengths and SHA-256), the
repository LICENSE and complete dependency notices, then runs the bounded native
smoke gates from ``helper_smoke.py`` against the helper inside the archive.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import stat
import sys
import tarfile
import tempfile
import zipfile

import helper_smoke
from release_targets import helper_member, target_info

MAX_MEMBER_BYTES = 64 * 1024 * 1024
MAX_TOTAL_BYTES = 256 * 1024 * 1024


class VerifyError(RuntimeError):
    pass


def _sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def _sha256_file(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def simple_parts(name):
    return name.split('/')


def _safe_name(name):
    if not isinstance(name, str) or not name or '\0' in name or '\\' in name or ':' in name or '//' in name or name.endswith('/'):
        raise VerifyError(f'unsafe archive member: {name!r}')
    pure = pathlib.PurePosixPath(name)
    if pure.is_absolute() or any(part in ('', '.', '..') for part in simple_parts(name)):
        raise VerifyError(f'unsafe archive member: {name!r}')
    return pure.as_posix()


def _read_archive(path):
    members = {}
    total = 0
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                name = _safe_name(info.filename)
                if info.is_dir():
                    continue
                file_type = (info.external_attr >> 16) & 0o170000
                if file_type == stat.S_IFLNK:
                    raise VerifyError(f'archive member cannot be a symlink: {name}')
                data = archive.read(info)
                if len(data) > MAX_MEMBER_BYTES:
                    raise VerifyError(f'archive member is too large: {name}')
                total += len(data)
                if total > MAX_TOTAL_BYTES:
                    raise VerifyError('archive exceeds its total size budget')
                if name in members:
                    raise VerifyError(f'duplicate archive member: {name}')
                members[name] = data
    else:
        with tarfile.open(path, 'r:gz') as archive:
            for info in archive.getmembers():
                name = _safe_name(info.name)
                if info.isdir():
                    continue
                if info.islnk() or info.issym() or not info.isfile():
                    raise VerifyError(f'archive member must be a regular file: {name}')
                source = archive.extractfile(info)
                if source is None:
                    raise VerifyError(f'archive member is unreadable: {name}')
                data = source.read()
                if len(data) > MAX_MEMBER_BYTES:
                    raise VerifyError(f'archive member is too large: {name}')
                total += len(data)
                if total > MAX_TOTAL_BYTES:
                    raise VerifyError('archive exceeds its total size budget')
                if name in members:
                    raise VerifyError(f'duplicate archive member: {name}')
                members[name] = data
    return members


def verify(archive, target, expected_version, source_commit=None, root=None, timeout_scale=1.0, allow_dirty=False):
    archive = pathlib.Path(archive).resolve()
    if archive.is_symlink() or not archive.is_file():
        raise VerifyError('archive must be a regular file')
    info = target_info(target)
    members = _read_archive(archive)
    if 'manifest.json' not in members:
        raise VerifyError('archive has no manifest.json')
    manifest = json.loads(members['manifest.json'].decode('utf-8'))
    if not isinstance(manifest, dict) or manifest.get('schemaVersion') != 1:
        raise VerifyError('unsupported archive manifest schema')
    if manifest.get('target') != target:
        raise VerifyError(f'archive target is {manifest.get("target")!r}, expected {target!r}')
    if manifest.get('version') != expected_version:
        raise VerifyError(f'archive version is {manifest.get("version")!r}, expected {expected_version!r}')
    if manifest.get('signed') is not False:
        raise VerifyError('archive must be explicitly unsigned')
    signing = manifest.get('signing') or {}
    for key in ('authenticode', 'notarized', 'updateSignature'):
        if signing.get(key) is not False:
            raise VerifyError(f'archive claims {key} signing')
    source = manifest.get('source') or {}
    commit = source.get('commit')
    if not isinstance(commit, str) or len(commit) != 40:
        raise VerifyError('archive has no exact source commit')
    if source_commit and commit != source_commit:
        raise VerifyError(f'archive source commit {commit} does not match {source_commit}')
    if source.get('workingTreeDirty') is not False and not allow_dirty:
        raise VerifyError('archive records a dirty source tree')

    records = manifest.get('members')
    if not isinstance(records, dict) or not records:
        raise VerifyError('archive has no member manifest')
    actual = {
        name: {'bytes': len(data), 'sha256': _sha256_bytes(data)}
        for name, data in members.items() if name != 'manifest.json'
    }
    if actual != records:
        missing = sorted(set(actual) - set(records))
        extra = sorted(set(records) - set(actual))
        changed = sorted(name for name in set(actual) & set(records) if actual[name] != records[name])
        raise VerifyError(
            'archive member manifest does not match contents: '
            f'missing={missing[:5]} extra={extra[:5]} changed={changed[:5]}'
        )

    helper = helper_member(target)
    if helper not in members:
        raise VerifyError(f'helper member is missing: {helper}')
    helper_files = [name for name in members if name.startswith(f'helper-{target}/')]
    if helper_files != [helper]:
        raise VerifyError(f'unexpected helper members: {helper_files}')
    for required in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'sbom.cdx.json'):
        if required not in members:
            raise VerifyError(f'required archive member is missing: {required}')
    license_files = [name for name in members if name.startswith('licenses/')]
    if not license_files:
        raise VerifyError('archive has no dependency license texts')

    if root:
        root_license = pathlib.Path(root).resolve() / 'LICENSE'
        if not root_license.is_file():
            raise VerifyError('repository LICENSE is missing')
        if members['LICENSE'] != root_license.read_bytes():
            raise VerifyError('archive LICENSE differs from the repository LICENSE')

    notices = members['THIRD_PARTY_NOTICES.md'].decode('utf-8')
    sbom = json.loads(members['sbom.cdx.json'].decode('utf-8'))
    components = sbom.get('components')
    if not isinstance(components, list) or not components:
        raise VerifyError('SBOM has no components')
    dependency_record = manifest.get('dependencies') or {}
    if dependency_record.get('licenseTextsComplete') is not True:
        raise VerifyError('archive does not claim a complete license text closure')
    if dependency_record.get('count') != len(components):
        raise VerifyError('manifest dependency count does not match the SBOM')
    for component in components:
        name = component.get('name')
        version = component.get('version')
        if not isinstance(name, str) or not isinstance(version, str):
            raise VerifyError('SBOM component has no name or version')
        if f'`{name}`' not in notices or f'`{version}`' not in notices:
            raise VerifyError(f'notices omit {name} {version}')
        for path in (component.get('properties') or []):
            if path.get('name') == 'originlm:licenseTexts':
                for member in filter(None, str(path.get('value', '')).split(',')):
                    if member not in members:
                        raise VerifyError(f'notices reference a missing license text: {member}')

    archive_digest = _sha256_file(archive)
    with tempfile.TemporaryDirectory(prefix='originlm-archive-smoke-') as temporary:
        helper_path = pathlib.Path(temporary) / info['executable']
        helper_path.write_bytes(members[helper])
        helper_path.chmod(0o755)
        smoke = helper_smoke.run_smoke(
            helper_path, target, expected_version, timeout_scale=timeout_scale
        )
        smoke['versionOutput'] = smoke.get('versionOutput')

    return {
        'status': 'pass',
        'archive': {
            'name': archive.name,
            'bytes': archive.stat().st_size,
            'sha256': archive_digest,
        },
        'target': target,
        'version': expected_version,
        'sourceCommit': commit,
        'signed': False,
        'members': len(records),
        'dependencies': len(components),
        'licenseTexts': len(license_files),
        'manifest': 'verified',
        'license': 'verified',
        'notices': 'verified',
        'smoke': smoke,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=pathlib.Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit', default=os.environ.get('GITHUB_SHA'))
    parser.add_argument('--root', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument('--out', type=pathlib.Path)
    parser.add_argument('--timeout-scale', type=float, default=1.0)
    parser.add_argument('--allow-dirty', action='store_true', help='local development only')
    args = parser.parse_args()
    evidence = verify(
        args.archive, args.target, args.version, args.source_commit, args.root,
        args.timeout_scale, args.allow_dirty,
    )
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    out = args.out or pathlib.Path('.verification') / f'helper-smoke-{args.target}.json'
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (VerifyError, helper_smoke.SmokeError, OSError, ValueError, KeyError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
