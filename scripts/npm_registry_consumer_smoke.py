#!/usr/bin/env python3
"""Registry-equivalent proof for the single universal npm SDK.

Unlike the bootstrap smoke (which installs both tarballs explicitly), this test
installs only ``originlm`` into a clean project and lets npm resolve its five
optional platform dependencies normally from a temporary local registry that
serves the actual assembled tarballs. On the current platform exactly one
platform package must resolve, and ``createLocalAI()`` must start its bundled
helper with an empty home/PATH and no overrides or existing grants.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import http.server
import json
import os
import pathlib
import platform
import re
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading
import urllib.parse

import npm_consumer_smoke

PLATFORM_PACKAGES = {
    ('linux', 'x86_64'): 'originlm-cli-linux-x64',
    ('linux', 'aarch64'): 'originlm-cli-linux-arm64',
    ('darwin', 'x86_64'): 'originlm-cli-darwin-x64',
    ('darwin', 'arm64'): 'originlm-cli-darwin-arm64',
    ('win32', 'x64'): 'originlm-cli-windows-x64',
}


class RegistryError(RuntimeError):
    pass


def _package(path):
    with tarfile.open(path, 'r:gz') as archive:
        member = archive.getmember('package/package.json')
        return json.loads(archive.extractfile(member).read().decode('utf-8'))


def _hashes(data):
    return {
        'shasum': hashlib.sha1(data).hexdigest(),
        'integrity': 'sha512-' + base64.b64encode(hashlib.sha512(data).digest()).decode('ascii'),
    }


def _current_platform_package():
    system = {'linux': 'linux', 'darwin': 'darwin', 'win32': 'win32'}.get(sys.platform)
    machine = platform.machine().lower()
    machine = {'amd64': 'x86_64', 'x86_64': 'x86_64', 'arm64': 'arm64', 'aarch64': 'aarch64'}.get(machine, machine)
    if system == 'win32' and machine == 'x86_64':
        return PLATFORM_PACKAGES[('win32', 'x64')]
    return PLATFORM_PACKAGES.get((system, machine))


class _Handler(http.server.BaseHTTPRequestHandler):
    registry = None

    def log_message(self, *_args):
        return

    def do_GET(self):
        path = urllib.parse.unquote(self.path.split('?', 1)[0])
        if path in ('/', '/-/ping'):
            self._json({})
            return
        if path.endswith('.tgz'):
            filename = path.rsplit('/', 1)[-1]
            entry = self.registry.tarballs.get(filename)
            if entry is None:
                self.send_error(404)
                return
            data = entry['data']
            self.send_response(200)
            self.send_header('Content-Type', 'application/octet-stream')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        name = path.lstrip('/')
        entry = self.registry.packages.get(name)
        if entry is None:
            self.send_error(404)
            return
        self._json(self.registry.metadata(name))

    def _json(self, value):
        data = json.dumps(value).encode('utf-8')
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


class Registry:
    def __init__(self, release_dir):
        self.release_dir = pathlib.Path(release_dir)
        self.base_url = None
        self.packages = {}
        self.tarballs = {}
        for path in sorted(self.release_dir.glob('*.tgz')):
            try:
                manifest = _package(path)
            except (KeyError, tarfile.TarError, json.JSONDecodeError) as error:
                raise RegistryError(f'invalid npm tarball: {path.name}') from error
            name = manifest.get('name')
            version = manifest.get('version')
            if not isinstance(name, str) or not isinstance(version, str):
                raise RegistryError(f'tarball has no package name/version: {path.name}')
            data = path.read_bytes()
            self.tarballs[path.name] = {'path': path, 'data': data, 'manifest': manifest}
            self.packages[name] = {'version': version, 'filename': path.name, 'data': data, 'manifest': manifest}

    def metadata(self, name):
        package = self.packages[name]
        manifest = dict(package['manifest'])
        manifest['dist'] = {
            'tarball': f'{self.base_url}/{urllib.parse.quote(name, safe="")}/-/{package["filename"]}',
            **_hashes(package['data']),
        }
        return {
            'name': name,
            'dist-tags': {'latest': package['version']},
            'versions': {package['version']: manifest},
        }


def _start_registry(release_dir):
    registry = Registry(release_dir)
    handler = type('Handler', (_Handler,), {'registry': registry})
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
    registry.base_url = f'http://127.0.0.1:{server.server_port}'
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return registry, server


def _run(args, cwd, env, timeout, label):
    executable = shutil.which(args[0])
    if not executable:
        raise RegistryError(f'missing executable: {args[0]}')
    command = [executable, *map(str, args[1:])]
    if os.name == 'nt' and executable.lower().endswith(('.cmd', '.bat')):
        command = [os.environ.get('COMSPEC', 'cmd.exe'), '/d', '/c', *command]
    try:
        result = subprocess.run(
            command, cwd=cwd, env=env, capture_output=True, text=True,
            encoding='utf-8', errors='replace', timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise RegistryError(f'{label} timed out') from error
    if result.returncode:
        raise RegistryError(
            f'{label} failed\n{result.stdout[-4000:]}\n{result.stderr[-4000:]}'
        )
    return result.stdout


def smoke(release_dir, version, source_commit=None, timeout=240):
    release_dir = pathlib.Path(release_dir).resolve()
    sdk = [path for path in release_dir.glob('originlm-*.tgz') if not path.name.startswith('originlm-cli-')]
    if len(sdk) != 1:
        raise RegistryError('release directory must contain exactly one universal originlm tarball')
    sdk = sdk[0]
    sdk_sha256 = hashlib.sha256(sdk.read_bytes()).hexdigest()
    manifest = _package(sdk)
    optional = manifest.get('optionalDependencies') or {}
    if set(optional) != set(PLATFORM_PACKAGES.values()) or any(value != version for value in optional.values()):
        raise RegistryError('universal SDK does not pin the five platform packages')
    expected = _current_platform_package()
    if not expected:
        raise RegistryError(f'unsupported test platform: {sys.platform}/{platform.machine()}')
    registry, server = _start_registry(release_dir)
    temporary = tempfile.mkdtemp(prefix='originlm-registry-consumer-')
    base = pathlib.Path(temporary)
    try:
        home = base / 'home'
        home.mkdir()
        project = base / 'project'
        project.mkdir()
        (project / 'package.json').write_text(
            json.dumps({'private': True, 'type': 'module'}), encoding='utf-8'
        )
        cache = base / 'npm-cache'
        install_env = {
            **os.environ,
            'HOME': str(home),
            'USERPROFILE': str(home),
            'npm_config_cache': str(cache),
            'npm_config_audit': 'false',
            'npm_config_fund': 'false',
            'npm_config_update_notifier': 'false',
        }
        _run([
            'npm', 'install', str(sdk), '--registry', registry.base_url,
            '--ignore-scripts', '--no-audit', '--no-fund', '--no-package-lock',
        ], project, install_env, timeout, 'single-package npm install')
        installed = json.loads(
            (project / 'node_modules' / 'originlm' / 'package.json').read_text(encoding='utf-8')
        )
        if installed.get('optionalDependencies') != optional:
            raise RegistryError('installed SDK optionalDependencies changed')
        resolved = project / 'node_modules' / expected.replace('/', os.sep)
        if not (resolved / 'package.json').is_file():
            raise RegistryError(f'optional platform package did not resolve: {expected}')
        if (project / 'node_modules' / '@originlm').exists():
            unexpected = sorted(
                path.name for path in (project / 'node_modules' / '@originlm').iterdir()
                if path.name != expected.split('/')[-1]
            )
            if unexpected:
                raise RegistryError(f'unexpected platform packages installed: {unexpected}')
        (project / 'consumer.mjs').write_text(npm_consumer_smoke.CONSUMER, encoding='utf-8')
        output = _run(['node', 'consumer.mjs'], project, npm_consumer_smoke._clean_env(home),
                      timeout, 'registry consumer')
    finally:
        server.shutdown()
        server.server_close()
        shutil.rmtree(base, ignore_errors=True)
    lines = [line for line in output.splitlines() if line.strip().startswith('{')]
    if not lines:
        raise RegistryError('consumer did not emit a JSON result')
    result = json.loads(lines[-1])
    if result.get('mode') != 'child':
        raise RegistryError('registry consumer did not use the packaged private helper')
    if result.get('connections') or result.get('models') or result.get('accounts'):
        raise RegistryError('registry consumer observed provider data in an empty home')
    return {
        'status': 'pass',
        'install': 'single-universal-package',
        'sourceCommit': source_commit,
        'sdkSha256': sdk_sha256,
        'registry': 'temporary-local-registry',
        'version': version,
        'sdk': sdk.name,
        'resolvedPlatformPackage': expected,
        'optionalDependencies': optional,
        'mode': result['mode'],
        'protocolVersion': result.get('protocolVersion'),
        'connections': result['connections'],
        'models': result['models'],
        'accounts': result['accounts'],
        'emptyHome': True,
        'emptyPath': True,
        'noOverrides': True,
        'noCheckout': True,
        'inferenceRequests': 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release-dir', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--source-commit')
    parser.add_argument('--out', type=pathlib.Path)
    parser.add_argument('--timeout', type=int, default=240)
    args = parser.parse_args()
    evidence = smoke(args.release_dir, args.version, args.source_commit, args.timeout)
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (RegistryError, OSError, ValueError, KeyError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
