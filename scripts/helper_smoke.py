#!/usr/bin/env python3
"""Bounded native smoke checks for one built helper executable.

The checks use an empty temporary home, never provider credentials, and never
request inference. They cover the version string, an explicit passive scan, and
the stdio JSON-RPC handshake plus an empty snapshot. Run this before packaging
and again against the helper extracted from the final archive.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

VERSION_TIMEOUT = 20
SCAN_TIMEOUT = 45
STDIO_TIMEOUT = 45


class SmokeError(RuntimeError):
    pass


def _environment(home):
    # Keep only what process startup needs. In particular, do not inherit
    # provider paths, credentials or helper overrides into the smoke process.
    environment = {
        'HOME': str(home),
        'USERPROFILE': str(home),
        'TMPDIR': str(home),
        'TEMP': str(home),
        'TMP': str(home),
        'LANG': 'C.UTF-8',
        'LC_ALL': 'C.UTF-8',
        'PATH': os.environ.get('PATH', ''),
    }
    if os.name == 'nt':
        for name in ('SYSTEMROOT', 'WINDIR', 'COMSPEC', 'PATHEXT'):
            if os.environ.get(name):
                environment[name] = os.environ[name]
    return environment


def _run(command, environment, timeout, input_text=None):
    try:
        return subprocess.run(
            command,
            input=input_text,
            capture_output=True,
            text=True,
            encoding='utf-8',
            errors='replace',
            timeout=timeout,
            env=environment,
        )
    except subprocess.TimeoutExpired as error:
        raise SmokeError(f'timed out after {timeout}s: {command[0]}') from error


def _stdio_messages(text):
    messages = {}
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(message, dict) and 'id' in message:
            messages[message['id']] = message
    return messages


def run_smoke(binary, target, expected_version, timeout_scale=1.0):
    binary = pathlib.Path(binary).resolve()
    if binary.is_symlink() or not binary.is_file():
        raise SmokeError('helper must be a regular file')
    started = time.monotonic()
    evidence = {
        'target': target,
        'version': expected_version,
        'helperBytes': binary.stat().st_size,
        'signed': False,
        'inferenceRequests': 0,
    }
    with tempfile.TemporaryDirectory(prefix='originlm-smoke-home-') as home:
        home_path = pathlib.Path(home)
        environment = _environment(home_path)

        version = _run([str(binary), '--version'], environment, VERSION_TIMEOUT * timeout_scale)
        if version.returncode:
            raise SmokeError(f'--version failed: {version.stderr.strip()}')
        expected = f'originlm {expected_version}'
        if version.stdout.strip() != expected:
            raise SmokeError(f'--version returned {version.stdout.strip()!r}, expected {expected!r}')
        evidence['versionOutput'] = version.stdout.strip()

        scan = _run(
            [str(binary), 'scan', '--json', '--home', str(home_path)],
            environment,
            SCAN_TIMEOUT * timeout_scale,
        )
        if scan.returncode:
            raise SmokeError(f'passive scan failed: {scan.stderr.strip()}')
        try:
            snapshot = json.loads(scan.stdout)
        except json.JSONDecodeError as error:
            raise SmokeError('passive scan did not emit one JSON snapshot') from error
        if not isinstance(snapshot, dict) or snapshot.get('mode') != 'live':
            raise SmokeError('passive scan did not report live mode')
        if snapshot.get('connections') or snapshot.get('models') or snapshot.get('accounts'):
            raise SmokeError('empty-home passive scan reported provider data')
        evidence['passiveScan'] = {
            'mode': snapshot.get('mode'),
            'connections': len(snapshot.get('connections') or []),
            'models': len(snapshot.get('models') or []),
            'accounts': len(snapshot.get('accounts') or []),
        }

        requests = '\n'.join(json.dumps(message) for message in (
            {'jsonrpc': '2.0', 'id': 1, 'method': 'runtime.handshake',
             'params': {'protocolVersion': '1.0', 'clientName': 'release-smoke'}},
            {'jsonrpc': '2.0', 'id': 2, 'method': 'discovery.refresh',
             'params': {'level': 'passive'}},
            {'jsonrpc': '2.0', 'id': 3, 'method': 'snapshot.get', 'params': {}},
        )) + '\n'
        stdio = _run(
            [str(binary), 'serve', '--stdio', '--allow-passive-discovery', '--home', str(home_path)],
            environment,
            STDIO_TIMEOUT * timeout_scale,
            input_text=requests,
        )
        if stdio.returncode:
            raise SmokeError(f'stdio helper failed: {stdio.stderr.strip()}')
        messages = _stdio_messages(stdio.stdout)
        handshake = messages.get(1, {}).get('result')
        if not isinstance(handshake, dict):
            raise SmokeError('stdio handshake did not return a result')
        protocol = handshake.get('protocolVersion')
        if not isinstance(protocol, str) or not protocol.startswith('1.'):
            raise SmokeError(f'unexpected stdio protocol version: {protocol!r}')
        methods = handshake.get('methods')
        if not isinstance(methods, list) or not methods:
            raise SmokeError('stdio handshake did not advertise methods')
        for identifier in (2, 3):
            if identifier not in messages:
                raise SmokeError(f'stdio request {identifier} received no response')
            if 'error' in messages[identifier]:
                raise SmokeError(f'stdio request {identifier} failed: {messages[identifier]["error"]}')
        snapshot = messages[3].get('result')
        if not isinstance(snapshot, dict) or snapshot.get('mode') != 'live':
            raise SmokeError('stdio snapshot did not report live mode')
        if snapshot.get('connections') or snapshot.get('models') or snapshot.get('accounts'):
            raise SmokeError('empty-home stdio snapshot reported provider data')
        evidence['stdio'] = {
            'protocolVersion': protocol,
            'methods': len(methods),
            'connections': len(snapshot.get('connections') or []),
            'models': len(snapshot.get('models') or []),
            'accounts': len(snapshot.get('accounts') or []),
        }
    evidence['elapsedSeconds'] = round(time.monotonic() - started, 3)
    evidence['status'] = 'pass'
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=pathlib.Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--out', type=pathlib.Path)
    parser.add_argument('--timeout-scale', type=float, default=1.0)
    args = parser.parse_args()
    evidence = run_smoke(args.binary, args.target, args.version, args.timeout_scale)
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (SmokeError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
