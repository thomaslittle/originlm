#!/usr/bin/env python3
"""Verify the declared MSRV with a real toolchain and write durable evidence.

The Evidence JSON records the exact command, toolchain versions, tested commit,
exit code and complete log, so a current-toolchain success cannot substitute for
the declared MSRV.
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


class MsrvError(RuntimeError):
    pass


def _run(command):
    completed = subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding='utf-8',
        errors='replace',
        env={**os.environ, 'RUSTUP_AUTO_INSTALL': '0'},
    )
    return completed


def _declared(root):
    manifest = (root / 'Cargo.toml').read_text(encoding='utf-8')
    match = re.search(r'^\s*rust-version\s*=\s*"([^"]+)"\s*$', manifest, re.MULTILINE)
    if not match:
        raise MsrvError('workspace Cargo.toml has no rust-version')
    return match.group(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolchain', default='1.88.0')
    parser.add_argument('--out', type=pathlib.Path, default=ROOT / '.verification' / 'msrv-1.88.json')
    args = parser.parse_args()
    root = ROOT
    commit = _run(['git', 'rev-parse', 'HEAD']).stdout.strip()
    rustc = _run(['rustc', f'+{args.toolchain}', '-vV'])
    cargo = _run(['cargo', f'+{args.toolchain}', '--version'])
    command = [
        'cargo', f'+{args.toolchain}', 'check', '--workspace',
        '--exclude', 'originlm-desktop', '--all-targets', '--locked',
    ]
    check = _run(command)
    evidence = {
        'schemaVersion': 1,
        'kind': 'originlm-msrv-verification',
        'status': 'pass' if check.returncode == 0 else 'fail',
        'testedCommit': commit,
        'declaredMsrv': _declared(root),
        'toolchain': {
            'requested': args.toolchain,
            'rustc': rustc.stdout.strip(),
            'cargo': cargo.stdout.strip(),
        },
        'command': command,
        'exitCode': check.returncode,
        'log': (check.stdout + check.stderr)[-200000:],
        'generatedAt': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(evidence, indent=2, sort_keys=True) + '\n', encoding='utf-8')
    print(json.dumps({
        'status': evidence['status'],
        'testedCommit': commit,
        'declaredMsrv': evidence['declaredMsrv'],
        'rustc': rustc.stdout.splitlines()[0] if rustc.stdout else None,
        'cargo': cargo.stdout.strip(),
        'exitCode': check.returncode,
        'out': str(args.out),
    }, indent=2))
    if check.returncode:
        print((check.stdout + check.stderr)[-8000:], file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (MsrvError, OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
