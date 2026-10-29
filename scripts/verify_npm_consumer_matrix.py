#!/usr/bin/env python3
"""Require all five platform consumer proofs before publishing npm.

Each proof is a separate ``npm-consumer-<label>`` artifact from the same
preparation run as the canonical ``npm-release`` set. The evidence must name the
same SDK SHA-256 and source commit, resolve the expected platform package, and
show an empty private-helper consumer run. One Ubuntu proof is not sufficient.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys

LABELS = {
    'linux-x64': 'originlm-cli-linux-x64',
    'linux-arm64': 'originlm-cli-linux-arm64',
    'darwin-x64': 'originlm-cli-darwin-x64',
    'darwin-arm64': 'originlm-cli-darwin-arm64',
    'win32-x64': 'originlm-cli-win32-x64',
}


class ConsumerMatrixError(RuntimeError):
    pass


def verify(artifacts, version, source_commit):
    artifacts = pathlib.Path(artifacts)
    release = artifacts / 'npm-release'
    manifest_path = release / 'manifest.json'
    if not manifest_path.is_file():
        raise ConsumerMatrixError('npm-release/manifest.json is missing')
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    if manifest.get('version') != version:
        raise ConsumerMatrixError('npm release version does not match')
    if manifest.get('sourceCommit') != source_commit:
        raise ConsumerMatrixError('npm release source commit does not match')
    sdk = manifest.get('sdk') or {}
    sdk_sha256 = sdk.get('sha256')
    if not isinstance(sdk_sha256, str):
        raise ConsumerMatrixError('npm release manifest has no SDK SHA-256')
    expected_optional = manifest.get('optionalDependencies')
    if not isinstance(expected_optional, dict) or len(expected_optional) != 5:
        raise ConsumerMatrixError('npm release manifest has no five optional dependencies')
    checked = []
    for label, expected_package in LABELS.items():
        path = artifacts / f'npm-consumer-{label}' / f'npm-consumer-{label}.json'
        if path.is_symlink() or not path.is_file():
            raise ConsumerMatrixError(f'missing consumer evidence: {path}')
        evidence = json.loads(path.read_text(encoding='utf-8'))
        problems = []
        if evidence.get('status') != 'pass':
            problems.append('status is not pass')
        if evidence.get('install') != 'single-universal-package':
            problems.append('install mode is not the single universal package')
        if evidence.get('version') != version:
            problems.append('version does not match')
        if evidence.get('sourceCommit') != source_commit:
            problems.append('source commit does not match')
        if evidence.get('sdkSha256') != sdk_sha256:
            problems.append('SDK SHA-256 does not match the release manifest')
        if evidence.get('resolvedPlatformPackage') != expected_package:
            problems.append(f'resolved package is not {expected_package}')
        if evidence.get('optionalDependencies') != expected_optional:
            problems.append('optionalDependencies do not match the release manifest')
        if evidence.get('mode') != 'child':
            problems.append('consumer did not start a private child helper')
        if evidence.get('connections') or evidence.get('models') or evidence.get('accounts'):
            problems.append('consumer observed provider data in an empty home')
        if evidence.get('inferenceRequests') != 0:
            problems.append('consumer performed inference')
        if problems:
            raise ConsumerMatrixError(f'{label}: ' + '; '.join(problems))
        checked.append({
            'label': label,
            'resolvedPlatformPackage': expected_package,
            'sdkSha256': evidence['sdkSha256'],
            'status': evidence['status'],
            'artifact': f'npm-consumer-{label}',
        })
    return {
        'status': 'pass',
        'version': version,
        'sourceCommit': source_commit,
        'sdkSha256': sdk_sha256,
        'platforms': checked,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit', required=True)
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(args.artifacts, args.version, args.source_commit)
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (ConsumerMatrixError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
