#!/usr/bin/env python3
"""Check the exact archive cargo publish produced before uploading it.

``cargo publish`` writes its upload archive under
``<target-dir>/package/tmp-crate/<name>-<version>.crate``. The publisher runs
``cargo publish --dry-run`` first and checks that this archive matches the
prepared qualified crate.

For crates that depend on another crate in the same closure, the prepared
archive's ``Cargo.lock`` records the local path patch, while the publish
archive's lock records the already-published registry source and checksum.
That lock difference is expected and is the only difference allowed. Every
other member must match byte-for-byte. The actual upload archive hash is
recorded and later compared with the published bytes.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import pathlib
import sys
import tarfile
import tempfile


class PayloadError(RuntimeError):
    pass


def _members(path: pathlib.Path):
    if path.is_symlink() or not path.is_file():
        raise PayloadError(f'missing crate archive: {path}')
    out = {}
    with tarfile.open(path, 'r:gz') as archive:
        for member in archive.getmembers():
            if not member.isfile():
                continue
            name = member.name.split('/', 1)[1] if '/' in member.name else member.name
            handle = archive.extractfile(member)
            if handle is None:
                raise PayloadError(f'unreadable crate member: {member.name}')
            out[name] = handle.read()
    return out


def _compare_sources(prepared: pathlib.Path, published: pathlib.Path):
    left = _members(prepared)
    right = _members(published)
    if set(left) != set(right):
        missing = sorted(set(left) - set(right))
        extra = sorted(set(right) - set(left))
        raise PayloadError(f'crate member list differs: missing={missing} extra={extra}')
    for name in sorted(left):
        if name == 'Cargo.lock':
            continue
        if left[name] != right[name]:
            raise PayloadError(f'crate member differs: {name}')
    return True


def verify(target_dir, name, version, expected_sha256, prepared_crate=None, allow_lock_resolution=False):
    path = pathlib.Path(target_dir) / 'package' / 'tmp-crate' / f'{name}-{version}.crate'
    if path.is_symlink() or not path.is_file():
        raise PayloadError(f'cargo publish did not produce the expected upload archive: {path}')
    data = path.read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    lock_resolution = False
    if actual.lower() != expected_sha256.lower():
        if not (allow_lock_resolution and prepared_crate):
            raise PayloadError(
                f'cargo publish upload archive differs from the qualified payload: '
                f'expected {expected_sha256}, got {actual} ({path})'
            )
        _compare_sources(pathlib.Path(prepared_crate), path)
        lock_resolution = True
    return {
        'status': 'pass',
        'name': name,
        'version': version,
        'sha256': actual,
        'expectedSha256': expected_sha256,
        'lockResolution': lock_resolution,
        'sourceFilesMatch': True,
        'bytes': len(data),
        'path': str(path),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=pathlib.Path, required=True)
    parser.add_argument('--name', required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--expected-sha256', required=True)
    parser.add_argument('--prepared-crate', type=pathlib.Path)
    parser.add_argument('--allow-lock-resolution', action='store_true')
    args = parser.parse_args()
    print(json.dumps(
        verify(
            args.target_dir,
            args.name,
            args.version,
            args.expected_sha256,
            args.prepared_crate,
            args.allow_lock_resolution,
        ),
        indent=2, sort_keys=True,
    ))


if __name__ == '__main__':
    try:
        main()
    except (PayloadError, OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
