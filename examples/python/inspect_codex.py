"""Inspect Codex through the public SDK. Generation is never requested.

Build the native helper first. --allow-metadata explicitly permits launching the
installed client and querying account/catalog/usage metadata with its own login.
"""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
try:
    from originlm import Client, LocalAIError
except ImportError:
    sys.path.insert(0, str(ROOT / 'packages/sdk-python/src'))
    from originlm import Client, LocalAIError


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--allow-metadata', action='store_true')
    parser.add_argument('--connection', help='Opaque connection ID from a preceding passive scan')
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error('Build the native helper and provide its executable path')
    flags = ['serve', '--stdio', '--allow-passive-discovery']
    if args.allow_metadata:
        flags.append('--allow-codex-metadata')
    with Client.from_stdio(executable=str(binary), args=flags, timeout=30) as ai:
        ai.discovery.refresh({'level': 'passive'})
        snapshot = ai.snapshot.get()
        candidates = [c for c in snapshot['connections'] if c['providerId'] == 'codex']
        if not args.allow_metadata:
            print(json.dumps({'mode': 'passive', 'connections': candidates}, indent=2))
            return
        if args.connection:
            candidates = [c for c in candidates if c['id'] == args.connection]
        else:
            defaults = {p['id'] for p in snapshot['profiles'] if p['label'] == 'Default profile'}
            candidates = [c for c in candidates if c['profileId'] in defaults and c['status'] == 'detected']
        if not candidates:
            parser.error('No matching detected Codex client. Run a passive scan and select --connection.')
        connection_id = candidates[0]['id']
        ai.connections.probe({'connectionId': connection_id})
        snapshot = ai.snapshot.get()
        routes = {r['id'] for r in snapshot['routes'] if r['connectionId'] == connection_id}
        # The runtime already masks identity; select only this connection's data.
        output = {
            'connection': next(c for c in snapshot['connections'] if c['id'] == connection_id),
            'models': [m for m in snapshot['models'] if m['routeId'] in routes],
            'quotaPools': [p for p in snapshot['quotaPools'] if connection_id in p['connectionIds']],
            'executionAvailable': False,
        }
        print(json.dumps(output, indent=2))


if __name__ == '__main__':
    try:
        main()
    except LocalAIError as exc:
        print(f'Native metadata unavailable: {exc.code}', file=sys.stderr)
        raise SystemExit(1)
