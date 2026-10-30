#!/usr/bin/env python3
"""Verify the packaged 13-crate closure compiles from its .crate archives.

``cargo package --no-verify`` proves the source worktree, not the packaged
contents. This gate safely unpacks the exact prepared ``.crate`` archives into a
temporary workspace outside the checkout, resolves internal dependencies only to
those unpacked packages (plus a bootstrap patch for the unpublished closure), and
runs ``cargo check`` with the declared MSRV. A fresh consumer manifest depends
only on the public ``originlm`` crate and is checked with the same local patches.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

EXPECTED_ORDER = [
    'originlm-contracts', 'originlm-platform', 'originlm-core',
    'originlm-chatgpt-auth', 'originlm-discovery', 'originlm-runtime',
    'originlm-store', 'originlm-service', 'originlm-transports',
    'originlm-control', 'originlm-adapters', 'originlm-kit', 'originlm',
]


class PackageClosureError(RuntimeError):
    pass


def _run(command, cwd, timeout=1800):
    result = subprocess.run(
        command, cwd=cwd, capture_output=True, text=True, encoding='utf-8',
        errors='replace', timeout=timeout,
    )
    if result.returncode:
        raise PackageClosureError(
            f'command failed: {" ".join(map(str, command))}\n'
            f'{result.stdout[-8000:]}\n{result.stderr[-8000:]}'
        )
    return result


def _hash(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def _lock_summary(path):
    data = tomllib.loads(pathlib.Path(path).read_text(encoding='utf-8'))
    registry = [
        {'name': package['name'], 'version': package['version']}
        for package in data.get('package', [])
        if str(package.get('source', '')).startswith('registry+')
    ]
    return {'sha256': _hash(path), 'registryPackages': registry}


def _resolve_and_check(cwd, toolchain, workspace=False):
    """Resolve registry dependencies online, then compile only that fresh lock.

    A copied workspace lock would hide a cold-cache/yanked candidate. A new lock
    proves what a clean registry consumer on the declared MSRV resolves today;
    the subsequent offline check proves the exact generated lock compiles.
    """
    _run(['cargo', f'+{toolchain}', 'generate-lockfile'], cwd)
    _run(['cargo', f'+{toolchain}', 'fetch', '--locked'], cwd)
    command = ['cargo', f'+{toolchain}', 'check', '--locked', '--offline']
    if workspace:
        command.insert(3, '--workspace')
    _run(command, cwd)
    lock = pathlib.Path(cwd) / 'Cargo.lock'
    if not lock.is_file():
        raise PackageClosureError('fresh dependency resolution produced no lockfile')
    return _lock_summary(lock)


def _safe_members(archive, crate, name, version):
    expected_root = f'{name}-{version}'
    for member in archive.getmembers():
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or any(part in ('', '.', '..') for part in path.parts):
            raise PackageClosureError(f'unsafe crate member: {member.name}')
        if not path.parts or path.parts[0] != expected_root:
            raise PackageClosureError(f'crate member is outside {expected_root}: {member.name}')
        if member.issym() or member.islnk() or not (member.isfile() or member.isdir()):
            raise PackageClosureError(f'crate member must be a regular file or directory: {member.name}')
    return expected_root


def _extract(crate, destination, name, version):
    with tarfile.open(crate, 'r:gz') as archive:
        _safe_members(archive, crate, name, version)
        for member in archive.getmembers():
            path = destination.joinpath(*pathlib.PurePosixPath(member.name).parts)
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                raise PackageClosureError(f'crate member is unreadable: {member.name}')
            path.write_bytes(source.read())
    package = destination / f'{name}-{version}'
    if not (package / 'Cargo.toml').is_file():
        raise PackageClosureError(f'crate lacks Cargo.toml: {crate.name}')
    return package


def _check_source_path_leakage(package):
    with (package / 'Cargo.toml').open('rb') as stream:
        manifest = tomllib.load(stream)
    tables = [
        manifest.get('dependencies', {}),
        manifest.get('dev-dependencies', {}),
        manifest.get('build-dependencies', {}),
    ]
    for target in (manifest.get('target') or {}).values():
        if not isinstance(target, dict):
            continue
        for key in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            tables.append(target.get(key, {}))
    for table in tables:
        for dependency, spec in table.items():
            if isinstance(spec, dict) and 'path' in spec:
                raise PackageClosureError(
                    f'normalized packaged Cargo.toml contains a path dependency: {package.name} -> {dependency}'
                )
    for candidate in package.rglob('*'):
        if not candidate.is_file() or candidate.suffix not in ('.toml', '.json', '.md', '.txt', '.rs'):
            continue
        text = candidate.read_text(encoding='utf-8', errors='replace')
        if '/home/tomlit/' in text or '/github/workspace/' in text or 'D:\\a\\' in text:
            raise PackageClosureError(f'packaged file leaks an absolute workspace path: {candidate.name}')


def verify(release_dir, toolchain, source_commit=None, out=None):
    release = pathlib.Path(release_dir).resolve()
    manifest_path = release / 'manifest.json'
    if not manifest_path.is_file():
        raise PackageClosureError('crate release manifest is missing')
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    packages = manifest.get('packages')
    if not isinstance(packages, list) or len(packages) != 13:
        raise PackageClosureError('crate release set is not the 13-crate closure')
    order = [item.get('name') for item in packages]
    if order != EXPECTED_ORDER or manifest.get('order') != EXPECTED_ORDER:
        raise PackageClosureError('crate release order is not the reviewed dependency-first order')
    if source_commit and manifest.get('sourceCommit') != source_commit:
        raise PackageClosureError('crate release source commit does not match')
    records = {}
    for item in packages:
        name = item.get('name')
        version = item.get('version')
        path = release / item.get('file', '')
        if not name or not version or path.is_symlink() or not path.is_file():
            raise PackageClosureError(f'invalid crate release record: {item!r}')
        digest = _hash(path)
        if digest != item.get('sha256') or path.stat().st_size != item.get('bytes'):
            raise PackageClosureError(f'crate payload changed: {path.name}')
        records[name] = (version, path, digest)
    temporary = tempfile.mkdtemp(prefix='originlm-packaged-closure-')
    base = pathlib.Path(temporary)
    try:
        for name in EXPECTED_ORDER:
            version, path, _ = records[name]
            package = _extract(path, base, name, version)
            _check_source_path_leakage(package)
            target = base / name
            package.rename(target)
        workspace = base / 'Cargo.toml'
        workspace.write_text(
            '[workspace]\nresolver = "3"\nmembers = [\n'
            + ''.join(f'  "{name}",\n' for name in EXPECTED_ORDER)
            + ']\n\n[patch.crates-io]\n'
            + ''.join(f'{name} = {{ path = "{base / name}" }}\n' for name in EXPECTED_ORDER),
            encoding='utf-8',
        )
        if (base / 'Cargo.lock').exists():
            raise PackageClosureError('a pre-existing workspace lock would hide fresh resolution')
        workspace_resolution = _resolve_and_check(base, toolchain, workspace=True)
        consumer = base / 'consumer'
        (consumer / 'src').mkdir(parents=True)
        (consumer / 'Cargo.toml').write_text(
            '[package]\nname = "originlm-packaged-consumer"\nversion = "0.0.0"\n'
            'edition = "2024"\n\n[workspace]\n\n[dependencies]\noriginlm = "=0.1.0-alpha.20"\n\n'
            '[patch.crates-io]\n'
            + ''.join(f'{name} = {{ path = "{base / name}" }}\n' for name in EXPECTED_ORDER),
            encoding='utf-8',
        )
        (consumer / 'src' / 'main.rs').write_text(
            'fn main() { let _ = originlm::LocalAi::start; }\n', encoding='utf-8'
        )
        consumer_resolution = _resolve_and_check(consumer, toolchain)
        evidence = {
            'schemaVersion': 2,
            'kind': 'originlm-packaged-crate-closure',
            'status': 'pass',
            'sourceCommit': manifest.get('sourceCommit'),
            'version': records['originlm'][0],
            'toolchain': toolchain,
            'order': EXPECTED_ORDER,
            'packages': [
                {'name': name, 'version': records[name][0], 'sha256': records[name][2]}
                for name in EXPECTED_ORDER
            ],
            'resolution': {
                'sourceLockfileCopied': False,
                'workspace': workspace_resolution,
                'consumer': consumer_resolution,
            },
            'checks': {
                'archiveHashes': True,
                'safeExtraction': True,
                'sourcePathLeakage': False,
                'freshRegistryResolution': True,
                'workspaceLockedCheck': True,
                'singleFacadeConsumerCheck': True,
                'consumerLockedCheck': True,
            },
            'limits': [
                'Registry dependencies are resolved online once with the declared MSRV, then compiled with --locked --offline from that generated lock.',
                'The fresh consumer bootstrap patch maps the unpublished closure to the unpacked reviewed crates.',
                'No runtime/provider action, publication or credential use occurs.',
            ],
        }
        if out:
            pathlib.Path(out).parent.mkdir(parents=True, exist_ok=True)
            pathlib.Path(out).write_text(json.dumps(evidence, indent=2, sort_keys=True) + '\n', encoding='utf-8')
        return evidence
    finally:
        shutil.rmtree(base, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release-dir', type=pathlib.Path, required=True)
    parser.add_argument('--toolchain', default='1.88.0')
    parser.add_argument('--source-commit')
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(args.release_dir, args.toolchain, args.source_commit, args.out)
    print(json.dumps(evidence, indent=2, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (PackageClosureError, OSError, ValueError, KeyError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
