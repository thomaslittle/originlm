#!/usr/bin/env python3
"""Verify the exact bytes crates.io serves after publication.

The publisher repackages from the pinned source and compares those hashes to
the prepared `.crate` manifest before upload. This script closes the remaining
boundary by downloading each published `.crate` and comparing its SHA-256 to
the same manifest, so a rebuild cannot silently diverge from the qualified
payload without failing the release job.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import pathlib
import sys
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


def verify(manifest_path, attempts=12, delay=5):
    manifest = json.loads(pathlib.Path(manifest_path).read_text(encoding='utf-8'))
    packages = manifest.get('packages')
    if not isinstance(packages, list) or not packages:
        raise VerifyError('crate manifest has no packages')
    checked = []
    for package in packages:
        name = package.get('name')
        version = package.get('version')
        expected = package.get('sha256')
        if not all(isinstance(value, str) and value for value in (name, version, expected)):
            raise VerifyError('crate manifest entry is incomplete')
        data = _download(name, version, attempts, delay)
        actual = hashlib.sha256(data).hexdigest()
        if actual != expected:
            raise VerifyError(
                f'published crate {name} {version} bytes differ: expected {expected}, got {actual}'
            )
        checked.append({
            'name': name,
            'version': version,
            'sha256': actual,
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
    parser.add_argument('--attempts', type=int, default=12)
    parser.add_argument('--delay', type=float, default=5)
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(args.manifest, args.attempts, args.delay)
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
