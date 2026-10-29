#!/usr/bin/env python3
"""Fresh-consumer proof for an installed originlm wheel.

The wheel is installed into a temporary virtual environment, then imported from
a directory outside any checkout with an empty HOME, an empty PATH, no
``LAI_*`` overrides and no shared broker grant. ``Client.auto()`` must find the
bundled helper through package data, start a private child, and return an empty
live snapshot without provider inference.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

from verify_python_wheel import normalize_version


class ConsumerError(RuntimeError):
    pass


CONSUMER = r'''
import json
import os
import pathlib
import sys

import originlm
from originlm import Client, resolve_helper

package_root = pathlib.Path(originlm.__file__).resolve().parent
if not package_root.is_relative_to(pathlib.Path(sys.prefix).resolve()):
    raise SystemExit(f"originlm was imported outside the venv: {package_root}")
helper = pathlib.Path(resolve_helper()).resolve()
if not helper.is_relative_to(package_root / "_bin"):
    raise SystemExit(f"helper was not found through package data: {helper}")
if not helper.is_file() or not os.access(helper, os.X_OK):
    raise SystemExit(f"bundled helper is not executable: {helper}")
for name in ("LAI_HELPER_PATH", "LAI_BROKER_ADDRESS", "LAI_BROKER_TOKEN_FILE"):
    if name in os.environ:
        raise SystemExit(f"ambient override must not be present: {name}")

with Client.auto() as ai:
    mode = ai.mode
    if mode != "child":
        raise SystemExit(f"Client.auto used {mode!r}, expected a private child")
    if ai.fallback_reason is not None:
        raise SystemExit(f"unexpected fallback: {ai.fallback_reason}")
    before = ai.snapshot.get()
    if before.get("mode") != "live":
        raise SystemExit("snapshot is not live")
    if before.get("connections") or before.get("models") or before.get("accounts"):
        raise SystemExit("empty-home snapshot contains provider data")
    ai.discovery.refresh({"level": "passive"})
    after = ai.snapshot.get()
    if after.get("mode") != "live":
        raise SystemExit("passive refresh did not stay live")
    if after.get("connections") or after.get("models") or after.get("accounts"):
        raise SystemExit("passive refresh returned provider data")
    if ai.models.list() != []:
        raise SystemExit("models.list returned provider models")
    protocol = ai.handshake.get("protocolVersion")

home = pathlib.Path(os.environ["HOME"]).resolve()
services = [path for path in home.rglob("*") if path.name.endswith(".service")]
if services:
    raise SystemExit(f"private helper installed a persistent service: {services}")
print(json.dumps({
    "mode": mode,
    "protocolVersion": protocol,
    "connections": len(after.get("connections") or []),
    "models": len(after.get("models") or []),
    "accounts": len(after.get("accounts") or []),
    "helper": str(helper),
    "packageRoot": str(package_root),
    "persistentServices": 0,
    "inferenceRequests": 0,
}))
'''


def _clean_env(home):
    home = pathlib.Path(home)
    empty_bin = home / 'empty-bin'
    empty_bin.mkdir(exist_ok=True)
    state = home / 'state'
    state.mkdir(exist_ok=True)
    for name in ('config', 'data', 'appdata', 'localappdata', 'tmp'):
        (home / name).mkdir(exist_ok=True)
    env = {
        'HOME': str(home),
        'USERPROFILE': str(home),
        'APPDATA': str(home / 'appdata'),
        'LOCALAPPDATA': str(home / 'localappdata'),
        'XDG_CONFIG_HOME': str(home / 'config'),
        'XDG_DATA_HOME': str(home / 'data'),
        'XDG_STATE_HOME': str(state),
        'TMPDIR': str(home / 'tmp'),
        'TEMP': str(home / 'tmp'),
        'TMP': str(home / 'tmp'),
        'LANG': 'C.UTF-8',
        'LC_ALL': 'C.UTF-8',
        # No provider or helper executable can be discovered through PATH.
        'PATH': str(empty_bin),
        'Path': str(empty_bin),
        'LOCAL_AI_KIT_STATE_DIR': str(state),
        'ORIGINLM_STATE_DIR': str(state),
        'PIP_CONFIG_FILE': os.devnull,
        'PIP_NO_INDEX': '1',
        'PIP_DISABLE_PIP_VERSION_CHECK': '1',
        'PYTHONNOUSERSITE': '1',
        'PYTHONDONTWRITEBYTECODE': '1',
    }
    for name in ('SYSTEMROOT', 'WINDIR', 'COMSPEC', 'PATHEXT'):
        if os.environ.get(name):
            env[name] = os.environ[name]
    for name in ('LAI_HELPER_PATH', 'LAI_BROKER_ADDRESS', 'LAI_BROKER_TOKEN_FILE',
                 'XDG_RUNTIME_DIR', 'PYTHONPATH', 'PYTHONHOME'):
        env.pop(name, None)
    return env


def _run(args, cwd, env, timeout, label):
    try:
        result = subprocess.run(
            [str(value) for value in args], cwd=cwd, env=env,
            capture_output=True, text=True, encoding='utf-8', errors='replace',
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise ConsumerError(f'{label} timed out') from error
    except OSError as error:
        raise ConsumerError(f'{label} could not start: {error}') from error
    if result.returncode:
        raise ConsumerError(
            f'{label} failed with exit {result.returncode}\n'
            f'{(result.stdout or "")[-4000:]}\n{(result.stderr or "")[-4000:]}'
        )
    return result.stdout


def _venv_python(directory):
    candidate = pathlib.Path(directory) / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
    if not candidate.is_file():
        raise ConsumerError(f'virtual environment python is missing: {candidate}')
    return candidate


def smoke(wheel, version, python=None, timeout=240, keep=False):
    wheel = pathlib.Path(wheel).resolve()
    if wheel.is_symlink() or not wheel.is_file():
        raise ConsumerError(f'wheel is unavailable: {wheel}')
    match = re.fullmatch(r'originlm-(.+)-py3-none-.+\.whl', wheel.name)
    if not match or normalize_version(match.group(1)) != normalize_version(version):
        raise ConsumerError(f'wheel version does not match the expected release: {wheel.name}')
    python = pathlib.Path(python or sys.executable).resolve()
    temporary = tempfile.mkdtemp(prefix='originlm-python-consumer-')
    base = pathlib.Path(temporary)
    try:
        home = base / 'home'
        home.mkdir()
        venv = base / 'venv'
        consumer = base / 'consumer'
        consumer.mkdir()
        install_env = _clean_env(home)
        install_env['PATH'] = os.pathsep.join(
            [str(venv / ('Scripts' if os.name == 'nt' else 'bin')), install_env['PATH']]
        )
        _run([python, '-m', 'venv', venv], base, install_env, timeout, 'venv creation')
        venv_python = _venv_python(venv)
        _run([
            venv_python, '-m', 'pip', 'install', '--no-index', '--no-deps',
            '--no-cache-dir', '--disable-pip-version-check', wheel,
        ], consumer, install_env, timeout, 'wheel installation')
        installed = _run([
            venv_python, '-c',
            'import json, originlm; print(json.dumps({"file": originlm.__file__, "version": getattr(originlm, "__version__", None)}))',
        ], consumer, install_env, timeout, 'installed import probe')
        probe = json.loads([line for line in installed.splitlines() if line.strip().startswith('{')][-1])
        if not pathlib.Path(probe['file']).resolve().is_relative_to(venv.resolve()):
            raise ConsumerError('originlm was imported from outside the virtual environment')
        (consumer / 'consumer.py').write_text(CONSUMER, encoding='utf-8')
        environment = _clean_env(home)
        output = _run(
            [venv_python, '-I', str(consumer / 'consumer.py')],
            consumer, environment, timeout, 'fresh consumer',
        )
        lines = [line for line in output.splitlines() if line.strip().startswith('{')]
        if not lines:
            raise ConsumerError('consumer did not emit a JSON result')
        result = json.loads(lines[-1])
        if result.get('mode') != 'child':
            raise ConsumerError('consumer did not use the packaged private helper')
        if result.get('connections') or result.get('models') or result.get('accounts'):
            raise ConsumerError('consumer observed provider data in an empty home')
        if result.get('persistentServices'):
            raise ConsumerError('private helper installed a persistent service')
        helper = pathlib.Path(result['helper']).resolve()
        if not helper.is_relative_to(venv.resolve()):
            raise ConsumerError('bundled helper is outside the virtual environment')
        return {
            'status': 'pass',
            'wheel': wheel.name,
            'version': version,
            'pep440Version': normalize_version(version),
            'mode': result['mode'],
            'protocolVersion': result.get('protocolVersion'),
            'connections': result['connections'],
            'models': result['models'],
            'accounts': result['accounts'],
            'helperThroughPackageData': True,
            'emptyHome': True,
            'emptyPath': True,
            'noOverrides': True,
            'noCheckout': True,
            'sharedGrant': False,
            'persistentServices': 0,
            'inferenceRequests': 0,
        }
    finally:
        if not keep:
            shutil.rmtree(base, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wheel', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--python', default=sys.executable)
    parser.add_argument('--timeout', type=int, default=240)
    parser.add_argument('--keep', action='store_true')
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = smoke(
        args.wheel, args.version, python=args.python,
        timeout=args.timeout, keep=args.keep,
    )
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (ConsumerError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
