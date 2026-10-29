#!/usr/bin/env python3
"""Verify exactly five qualified Python wheel artifacts before PyPI upload.

The publishable set is the five supported targets: manylinux_2_28 x86_64/ARM64,
macOS 15 Intel/Apple-silicon and Windows x64. Each wheel must have matching
wheel and fresh-consumer evidence, the correct native platform tag/kind, native
binary proof (ELF/PE/Mach-O), twine metadata, license/SBOM/provenance closure and
the exact source commit. Upload inputs are wheels only; reports stay separate.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import sys

import verify_python_wheel
from release_targets import TARGETS


class PublishSetError(RuntimeError):
    pass


EXPECTED = {
    target: info for target, info in TARGETS.items()
    if target in {
        'x86_64-unknown-linux-gnu',
        'aarch64-unknown-linux-gnu',
        'x86_64-apple-darwin',
        'aarch64-apple-darwin',
        'x86_64-pc-windows-msvc',
    }
}


def _evidence(path):
    if path.is_symlink() or not path.is_file():
        raise PublishSetError(f'missing evidence: {path}')
    try:
        return json.loads(path.read_text(encoding='utf-8'))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise PublishSetError(f'invalid evidence: {path}') from error


def validate_evidence(target, wheel_sha256, wheel_evidence, consumer_evidence,
                      version, source_commit):
    info = EXPECTED[target]
    platform = info['releaseWheelPlatform']
    kind = info['releaseWheelKind']
    problems = []
    if wheel_evidence.get('status') != 'pass':
        problems.append('wheel evidence is not pass')
    if wheel_evidence.get('target') != target:
        problems.append('wheel target mismatch')
    if wheel_evidence.get('sourceCommit') != source_commit:
        problems.append('wheel source mismatch')
    if wheel_evidence.get('version') != version:
        problems.append('wheel version mismatch')
    if wheel_evidence.get('platform') != platform:
        problems.append(f'wheel platform is not {platform}')
    if wheel_evidence.get('release') is not True or wheel_evidence.get('uploadGuard') != 'removed':
        problems.append('wheel is not release-staged with the private guard removed')
    if kind == 'manylinux':
        if wheel_evidence.get('manylinux') is not True:
            problems.append('Linux wheel is not manylinux-qualified')
        if not (wheel_evidence.get('auditwheel') or {}).get('passed'):
            problems.append('auditwheel proof is missing')
    else:
        if wheel_evidence.get('manylinux') is not False:
            problems.append('native wheel incorrectly claims manylinux')
        native = wheel_evidence.get('nativeBinary') or {}
        expected_format = 'pe' if kind == 'windows' else 'macho'
        if native.get('passed') is not True or native.get('format') != expected_format:
            problems.append(f'native {expected_format} binary proof is missing')
    if not (wheel_evidence.get('twine') or {}).get('passed'):
        problems.append('twine check is missing')
    if consumer_evidence.get('status') != 'pass':
        problems.append('consumer evidence is not pass')
    if consumer_evidence.get('target') != target:
        problems.append('consumer target mismatch')
    if consumer_evidence.get('sourceCommit') != source_commit:
        problems.append('consumer source mismatch')
    if consumer_evidence.get('version') != version:
        problems.append('consumer version mismatch')
    if consumer_evidence.get('wheelSha256') != wheel_sha256:
        problems.append('consumer wheel hash mismatch')
    if consumer_evidence.get('mode') != 'child':
        problems.append('consumer did not use the packaged private helper')
    if consumer_evidence.get('connections') or consumer_evidence.get('models') or consumer_evidence.get('accounts'):
        problems.append('consumer observed provider data in an empty home')
    if consumer_evidence.get('inferenceRequests') != 0:
        problems.append('consumer performed inference')
    if problems:
        raise PublishSetError(f'{target}: ' + '; '.join(problems))
    return {
        'target': target,
        'platform': platform,
        'kind': kind,
        'wheelSha256': wheel_sha256,
    }


def verify_set(artifacts, version, source_commit, dist, reports):
    artifacts = pathlib.Path(artifacts)
    dist = pathlib.Path(dist)
    reports = pathlib.Path(reports)
    directories = sorted(artifacts.glob('python-wheel-*'))
    found = {directory.name.removeprefix('python-wheel-'): directory for directory in directories if directory.is_dir()}
    if set(found) != set(EXPECTED):
        raise PublishSetError(
            f'expected exactly the five supported wheel targets; '
            f'missing={sorted(set(EXPECTED) - set(found))} extra={sorted(set(found) - set(EXPECTED))}'
        )
    for directory in (dist, reports):
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True)
    records = []
    for target in sorted(EXPECTED):
        directory = found[target]
        wheels = sorted(directory.rglob('originlm-*.whl'))
        if len(wheels) != 1:
            raise PublishSetError(f'{target}: expected exactly one wheel tarball, found {len(wheels)}')
        wheel = wheels[0]
        if wheel.is_symlink() or not wheel.is_file():
            raise PublishSetError(f'{target}: wheel is not a regular file')
        wheel_sha256 = hashlib.sha256(wheel.read_bytes()).hexdigest()
        wheel_evidence = _evidence(directory / 'prepared' / 'evidence' / f'python-wheel-{target}.json')
        consumer_evidence = _evidence(directory / 'prepared' / 'evidence' / f'python-consumer-{target}.json')
        record = validate_evidence(
            target, wheel_sha256, wheel_evidence, consumer_evidence, version, source_commit
        )
        verified = verify_python_wheel.verify(
            wheel, version, target=target,
            plat_name=EXPECTED[target]['releaseWheelPlatform'],
            source_commit=source_commit, release=True,
        )
        if verified.get('sha256') != wheel_sha256:
            raise PublishSetError(f'{target}: verifier changed the wheel identity')
        (reports / f'python-wheel-{target}.json').write_text(
            json.dumps(verified, indent=2, sort_keys=True) + '\n', encoding='utf-8'
        )
        shutil.copy2(wheel, dist / wheel.name)
        record['wheelFile'] = wheel.name
        records.append(record)
    sums = reports / 'python-wheel-SHA256SUMS'
    sums.write_text(
        ''.join(f'{record["wheelSha256"]}  {record["wheelFile"]}\n' for record in sorted(records, key=lambda item: item['wheelFile'])),
        encoding='utf-8',
    )
    (reports / 'python-wheel-release-set.json').write_text(
        json.dumps({
            'schemaVersion': 1,
            'kind': 'originlm-python-wheel-release-set',
            'version': version,
            'sourceCommit': source_commit,
            'signed': False,
            'platforms': records,
        }, indent=2, sort_keys=True) + '\n', encoding='utf-8'
    )
    return {'status': 'pass', 'version': version, 'sourceCommit': source_commit, 'platforms': records}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit', required=True)
    parser.add_argument('--dist', type=pathlib.Path, default=pathlib.Path('dist'))
    parser.add_argument('--reports', type=pathlib.Path, default=pathlib.Path('reports'))
    args = parser.parse_args()
    print(json.dumps(verify_set(
        args.artifacts, args.version, args.source_commit, args.dist, args.reports
    ), indent=2, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (PublishSetError, verify_python_wheel.VerifyError, OSError, ValueError, KeyError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
