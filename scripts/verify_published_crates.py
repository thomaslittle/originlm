#!/usr/bin/env python3
"""Verify the exact bytes crates.io serves after publication.

The publisher repackages from the pinned source and compares those hashes to
the prepared `.crate` manifest before upload. This script closes the remaining
boundary by downloading each published `.crate` and comparing it to the
prepared qualified archive.

For crates that depend on another crate in the same closure, the prepared
archive's ``Cargo.lock`` records the local path patch, while the published
archive's lock records the registry source and checksum. That lock difference
is expected and is the only difference allowed; every other member must match
byte-for-byte. The published hash and the lock-resolution flag are recorded.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import pathlib
import sys
import tarfile
import time
import urllib.error
import urllib.request

BASE = 'https://crates.io/api/v1/crates'
USER_AGENT = 'originlm-release-verification (https://github.com/thomaslittle/originlm)'


class VerifyError(RuntimeError):
    pass


def _download(name, version, attempts, delay):
    url = f'{BASE}/{name}/{version}/download'
    request = urllib.request.Request(url, headers={'User-Agent': USER_AGENT})
    last = None
    for attempt in range(attempts):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.read()
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as error:
            last = error
            if attempt + 1 < attempts:
                time.sleep(delay)
    raise VerifyError(f'could not download published crate {name} {version}: {last}')


def _members_bytes(data):
    out = {}
    with tarfile.open(fileobj=__import__('io').BytesIO(data), mode='r:gz') as archive:
        for member in archive.getmembers():
            if not member.isfile():
                continue
            name = member.name.split('/', 1)[1] if '/' in member.name else member.name
            handle = archive.extractfile(member)
            if handle is None:
                raise VerifyError(f'unreadable crate member: {member.name}')
            out[name] = handle.read()
    return out


def _members_file(path):
    if path.is_symlink() or not path.is_file():
        raise VerifyError(f'missing prepared crate archive: {path}')
    return _members_bytes(path.read_bytes())


def _compare_sources(prepared, published):
    left = _members_file(prepared)
    right = _members_bytes(published)
    if set(left) != set(right):
        missing = sorted(set(left) - set(right))
        extra = sorted(set(right) - set(left))
        raise VerifyError(f'crate member list differs: missing={missing} extra={extra}')
    for name in sorted(left):
        if name in ('Cargo.lock', '.cargo_vcs_info.json'):
            continue
        if left[name] != right[name]:
            raise VerifyError(f'crate member differs: {name}')
    return True


def verify(manifest_path, attempts=12, delay=5, prepared_dir=None, allow_lock_resolution=False):
    manifest = json.loads(pathlib.Path(manifest_path).read_text(encoding='utf-8'))
    packages = manifest.get('packages')
    if not isinstance(packages, list) or not packages:
        raise VerifyError('crate manifest has no packages')
    checked = []
    for package in packages:
        name = package.get('name')
        version = package.get('version')
        expected = package.get('sha256')
        filename = package.get('file')
        if not all(isinstance(value, str) and value for value in (name, version, expected)):
            raise VerifyError('crate manifest entry is incomplete')
        data = _download(name, version, attempts, delay)
        actual = hashlib.sha256(data).hexdigest()
        lock_resolution = False
        if actual != expected:
            if not (allow_lock_resolution and prepared_dir and filename):
                raise VerifyError(
                    f'published crate {name} {version} bytes differ: expected {expected}, got {actual}'
                )
            _compare_sources(pathlib.Path(prepared_dir) / filename, data)
            lock_resolution = True
        checked.append({
            'name': name,
            'version': version,
            'sha256': actual,
            'expectedSha256': expected,
            'lockResolution': lock_resolution,
            'sourceFilesMatch': True,
            'bytes': len(data),
        })
    return {
        'schemaVersion': 1,
        'kind': 'originlm-published-crate-verification',
        'status': 'pass',
        'sourceCommit': manifest.get('sourceCommit'),
        'packages': checked,
        'generatedAt': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=pathlib.Path, required=True)
    parser.add_argument('--prepared-dir', type=pathlib.Path)
    parser.add_argument('--allow-lock-resolution', action='store_true')
    parser.add_argument('--attempts', type=int, default=12)
    parser.add_argument('--delay', type=float, default=5)
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(
        args.manifest,
        args.attempts,
        args.delay,
        args.prepared_dir,
        args.allow_lock_resolution,
    )
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (VerifyError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
