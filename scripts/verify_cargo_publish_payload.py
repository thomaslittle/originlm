#!/usr/bin/env python3
"""Check the exact archive cargo publish produced before uploading it.

``cargo publish`` writes its upload archive under
``<target-dir>/package/tmp-crate/<name>-<version>.crate``. The publisher runs
``cargo publish --dry-run`` first, checks that this archive matches the prepared
and repacked qualified hash, and only then runs the real publish. This accounts
for Cargo's own packaging step instead of validating an earlier archive only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys


class PayloadError(RuntimeError):
    pass


def verify(target_dir, name, version, expected_sha256):
    path = pathlib.Path(target_dir) / 'package' / 'tmp-crate' / f'{name}-{version}.crate'
    if path.is_symlink() or not path.is_file():
        raise PayloadError(f'cargo publish did not produce the expected upload archive: {path}')
    data = path.read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    if actual.lower() != expected_sha256.lower():
        raise PayloadError(
            f'cargo publish upload archive differs from the qualified payload: '
            f'expected {expected_sha256}, got {actual} ({path})'
        )
    return {
        'status': 'pass',
        'name': name,
        'version': version,
        'sha256': actual,
        'bytes': len(data),
        'path': str(path),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=pathlib.Path, required=True)
    parser.add_argument('--name', required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--expected-sha256', required=True)
    args = parser.parse_args()
    print(json.dumps(
        verify(args.target_dir, args.name, args.version, args.expected_sha256),
        indent=2, sort_keys=True,
    ))


if __name__ == '__main__':
    try:
        main()
    except (PayloadError, OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
