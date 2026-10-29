#!/usr/bin/env python3
"""Fresh npm consumer proof for the universal SDK and one platform package.

Installs the packed tarballs into a clean temporary project, then starts
``createLocalAI()`` with an empty home, no checkout, no PATH helper and no
``LAI_*`` overrides. The SDK must resolve the installed platform package, start
its private helper, and return an empty live snapshot without provider inference.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile


class ConsumerError(RuntimeError):
    pass


def _run(args, cwd, env, timeout):
    executable = shutil.which(args[0])
    if not executable:
        raise ConsumerError(f'missing executable: {args[0]}')
    command = [executable, *map(str, args[1:])]
    if os.name == 'nt' and executable.lower().endswith(('.cmd', '.bat')):
        command = [os.environ.get('COMSPEC', 'cmd.exe'), '/d', '/c', *command]
    try:
        result = subprocess.run(
            command, cwd=cwd, env=env, capture_output=True, text=True,
            encoding='utf-8', errors='replace', timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise ConsumerError(f'command timed out: {args[0]}') from error
    if result.returncode:
        raise ConsumerError(
            f'command failed: {args[0]}\n{result.stdout[-4000:]}\n{result.stderr[-4000:]}'
        )
    return result.stdout


CONSUMER = r'''
import {createLocalAI} from 'originlm';
const ai = await createLocalAI({clientName: 'release-consumer'});
try {
  if (ai.mode !== 'child') throw new Error(`unexpected mode ${ai.mode}`);
  const before = await ai.snapshot.get();
  if (before.mode !== 'live') throw new Error('snapshot is not live');
  if (before.connections.length || before.models.length || before.accounts.length) {
    throw new Error('empty-home snapshot contains provider data');
  }
  await ai.discovery.refresh({level: 'passive'});
  const after = await ai.snapshot.get();
  if (after.mode !== 'live' || after.connections.length || after.models.length || after.accounts.length) {
    throw new Error('passive refresh returned provider data');
  }
  console.log(JSON.stringify({
    mode: ai.mode,
    protocolVersion: ai.handshake.protocolVersion,
    connections: after.connections.length,
    models: after.models.length,
    accounts: after.accounts.length,
  }));
} finally {
  await ai.close();
}
'''


def _clean_env(home):
    home = pathlib.Path(home)
    empty_bin = home / 'empty-bin'
    empty_bin.mkdir(exist_ok=True)
    state = home / 'state'
    state.mkdir(exist_ok=True)
    env = {
        'HOME': str(home),
        'USERPROFILE': str(home),
        'APPDATA': str(home / 'appdata'),
        'LOCALAPPDATA': str(home / 'localappdata'),
        'XDG_CONFIG_HOME': str(home / 'config'),
        'XDG_DATA_HOME': str(home / 'data'),
        'XDG_STATE_HOME': str(state),
        'TMPDIR': str(home),
        'TEMP': str(home),
        'TMP': str(home),
        'LANG': 'C.UTF-8',
        'LC_ALL': 'C.UTF-8',
        # No provider executable can be discovered through PATH.
        'PATH': str(empty_bin),
        'Path': str(empty_bin),
        'LOCAL_AI_KIT_STATE_DIR': str(state),
        'ORIGINLM_STATE_DIR': str(state),
    }
    for name in ('SYSTEMROOT', 'WINDIR', 'COMSPEC', 'PATHEXT'):
        if os.environ.get(name):
            env[name] = os.environ[name]
    # Explicitly prove that no developer override or ambient grant is used.
    for name in ('LAI_HELPER_PATH', 'LAI_BROKER_ADDRESS', 'LAI_BROKER_TOKEN_FILE',
                 'XDG_RUNTIME_DIR'):
        env.pop(name, None)
    return env


def smoke(sdk, platform, version, timeout=180):
    sdk = pathlib.Path(sdk).resolve()
    platform = pathlib.Path(platform).resolve()
    for path in (sdk, platform):
        if path.is_symlink() or not path.is_file():
            raise ConsumerError(f'tarball is unavailable: {path}')
    with tempfile.TemporaryDirectory(prefix='originlm-npm-consumer-') as temporary:
        base = pathlib.Path(temporary)
        home = base / 'home'
        home.mkdir()
        project = base / 'project'
        project.mkdir()
        (project / 'package.json').write_text(
            json.dumps({'private': True, 'type': 'module'}), encoding='utf-8'
        )
        install_env = {**os.environ, 'npm_config_offline': 'true', 'npm_config_audit': 'false',
                       'npm_config_fund': 'false', 'npm_config_update_notifier': 'false'}
        _run([
            'npm', 'install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund',
            '--omit=optional', '--package-lock=false', str(sdk), str(platform),
        ], project, install_env, timeout)
        installed = json.loads((project / 'node_modules' / 'originlm' / 'package.json').read_text(encoding='utf-8'))
        if installed.get('version') != version:
            raise ConsumerError('installed SDK version does not match the expected release')
        optional = installed.get('optionalDependencies') or {}
        if len(optional) != 5:
            raise ConsumerError('installed SDK does not declare all five platform packages')
        (project / 'consumer.mjs').write_text(CONSUMER, encoding='utf-8')
        output = _run(['node', 'consumer.mjs'], project, _clean_env(home), timeout)
    lines = [line for line in output.splitlines() if line.strip().startswith('{')]
    if not lines:
        raise ConsumerError('consumer did not emit a JSON result')
    result = json.loads(lines[-1])
    if result.get('mode') != 'child':
        raise ConsumerError('consumer did not use the packaged private helper')
    if result.get('connections') or result.get('models') or result.get('accounts'):
        raise ConsumerError('consumer observed provider data in an empty home')
    return {
        'status': 'pass',
        'sdk': sdk.name,
        'platformPackage': platform.name,
        'version': version,
        'mode': result['mode'],
        'protocolVersion': result.get('protocolVersion'),
        'connections': result['connections'],
        'models': result['models'],
        'accounts': result['accounts'],
        'noPathHelper': True,
        'noOverrides': True,
        'inferenceRequests': 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=pathlib.Path, required=True)
    parser.add_argument('--platform', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--out', type=pathlib.Path)
    parser.add_argument('--timeout', type=int, default=180)
    args = parser.parse_args()
    evidence = smoke(args.sdk, args.platform, args.version, args.timeout)
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (ConsumerError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
