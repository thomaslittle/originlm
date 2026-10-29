#!/usr/bin/env python3
"""Prepare one unsigned, installer-compatible helper archive.

Archive shape (the installer extracts only ``helper-<target>/originlm[.exe]``)::

    helper-<target>/originlm[.exe]
    LICENSE
    THIRD_PARTY_NOTICES.md
    licenses/<crate>-<version>/<license text>
    sbom.cdx.json
    manifest.json

``manifest.json`` records exact source, target, toolchain and version provenance
plus byte length and SHA-256 for every other member. ``SHA256SUMS`` outside the
archive records the archive itself. Nothing is signed or published here.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import pathlib
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile

import license_closure
from release_targets import REPOSITORY, archive_name, helper_member, target_info


class PrepareError(RuntimeError):
    pass


def _run(command, root):
    result = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        text=True,
        encoding='utf-8',
        errors='replace',
        timeout=120,
    )
    if result.returncode:
        raise PrepareError(f'command failed: {" ".join(map(str, command))}\n{result.stderr[-4000:]}')
    return result.stdout


def _sha256(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def _members(root):
    root = pathlib.Path(root)
    records = {}
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise PrepareError(f'release member cannot be a symlink: {path}')
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if relative == 'manifest.json':
            continue
        if relative.startswith('/') or '..' in pathlib.PurePosixPath(relative).parts:
            raise PrepareError(f'unsafe release member: {relative}')
        records[relative] = {'bytes': path.stat().st_size, 'sha256': _sha256(path)}
    return records


def _pinned_toolchain(root):
    text = (root / 'rust-toolchain.toml').read_text(encoding='utf-8')
    match = re.search(r'^\s*channel\s*=\s*"([^"]+)"\s*$', text, re.MULTILINE)
    if not match:
        raise PrepareError('rust-toolchain.toml has no pinned channel')
    return match.group(1)


def _linux_glibc():
    result = subprocess.run(
        ['ldd', '--version'], capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30
    )
    text = result.stdout + result.stderr
    match = re.search(r'GLIBC[^0-9]*([0-9]+\.[0-9]+)', text)
    if not match:
        match = re.search(r'([0-9]+\.[0-9]+)\s*$', text.strip().splitlines()[0] if text.strip() else '')
    return match.group(1) if match else None


def _build_host():
    release = pathlib.Path('/etc/os-release')
    if release.is_file():
        for line in release.read_text(encoding='utf-8').splitlines():
            if line.startswith('PRETTY_NAME='):
                return line.split('=', 1)[1].strip().strip('"')
    return platform.platform()


def _compatibility(target):
    info = target_info(target)
    compatibility = {'os': info['os'], 'arch': info['arch'], 'support': info['support']}
    if info['os'] == 'linux':
        glibc = _linux_glibc()
        if not glibc:
            raise PrepareError('cannot determine the Linux glibc build-host baseline')
        host = _build_host()
        compatibility['glibc'] = {
            'minimum': glibc,
            'buildHost': host,
            'basis': f'{host} build host; versions below this are not qualified',
        }
    elif info['os'] == 'macos':
        compatibility['macos'] = {
            'minimum': os.environ.get('MACOSX_DEPLOYMENT_TARGET', '15.0'),
            'basis': 'MACOSX_DEPLOYMENT_TARGET at build time',
        }
    else:
        compatibility['windows'] = {
            'minimum': 'not independently qualified',
            'basis': 'built and smoke-tested on Windows Server 2025',
        }
    compatibility['excluded'] = [
        'musl/Alpine helpers',
        'Windows ARM64 helpers',
        'the GPUI desktop application (runtime bundles contain only the helper)',
    ]
    return compatibility


def _ci():
    keys = ('GITHUB_WORKFLOW', 'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'GITHUB_SERVER_URL',
            'GITHUB_REPOSITORY', 'GITHUB_REF', 'RUNNER_OS', 'RUNNER_ARCH')
    values = {key.lower(): os.environ[key] for key in keys if os.environ.get(key)}
    return values or None


def _source_state(root, source_commit, allow_dirty=False):
    status = _run(['git', 'status', '--porcelain'], root)
    dirty = bool(status.strip())
    head = _run(['git', 'rev-parse', 'HEAD'], root).strip()
    if source_commit and source_commit != head:
        raise PrepareError(f'source commit {source_commit} does not match checked out HEAD {head}')
    if dirty and not allow_dirty:
        raise PrepareError('release source tree must be clean; commit or discard local changes')
    return {
        'repository': REPOSITORY,
        'commit': source_commit or head,
        'workingTreeDirty': dirty,
        'ref': os.environ.get('GITHUB_REF'),
    }


def _write_tar_gz(stage, destination):
    with destination.open('wb') as raw:
        with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0, compresslevel=9) as compressed:
            with tarfile.open(fileobj=compressed, mode='w', format=tarfile.PAX_FORMAT) as archive:
                for path in sorted(stage.rglob('*')):
                    if not path.is_file():
                        continue
                    relative = path.relative_to(stage).as_posix()
                    info = tarfile.TarInfo(relative)
                    info.size = path.stat().st_size
                    info.mtime = 0
                    info.uid = info.gid = 0
                    info.uname = info.gname = ''
                    info.mode = 0o755 if relative.startswith('helper-') else 0o644
                    info.type = tarfile.REGTYPE
                    with path.open('rb') as source:
                        archive.addfile(info, source)


def _write_zip(stage, destination):
    with zipfile.ZipFile(
        destination, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9, allowZip64=True
    ) as archive:
        for path in sorted(stage.rglob('*')):
            if not path.is_file():
                continue
            relative = path.relative_to(stage).as_posix()
            info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            mode = 0o755 if relative.startswith('helper-') else 0o644
            info.external_attr = (stat.S_IFREG | mode) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes())


def _update_sums(out, name, digest):
    path = out / 'SHA256SUMS'
    records = {}
    if path.is_file():
        for line in path.read_text(encoding='utf-8').splitlines():
            parts = line.split('  ', 1)
            if len(parts) == 2 and re.fullmatch(r'[0-9a-f]{64}', parts[0]):
                records[parts[1]] = parts[0]
    records[name] = digest
    path.write_text(
        ''.join(f'{records[archive]}  {archive}\n' for archive in sorted(records)),
        encoding='utf-8',
    )


def prepare(binary, target, out, root, source_commit=None, allow_dirty=False):
    root = pathlib.Path(root).resolve()
    out = pathlib.Path(out).resolve()
    binary = pathlib.Path(binary).resolve()
    info = target_info(target)
    if binary.is_symlink() or not binary.is_file():
        raise PrepareError('binary must be a regular file')
    if binary.name != info['executable']:
        raise PrepareError(f'expected binary filename {info["executable"]}')
    size = binary.stat().st_size
    if not 0 < size <= 40 * 1024 * 1024:
        raise PrepareError(f'helper size {size} exceeds the 40 MiB budget or is empty')

    source = _source_state(root, source_commit, allow_dirty)
    collection = license_closure.collect(root, target)
    package_version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
    if collection['version'] != package_version:
        raise PrepareError(
            f'Cargo version {collection["version"]} does not match package.json version {package_version}'
        )
    pinned = _pinned_toolchain(root)
    toolchain = collection['toolchain']
    if toolchain['rustc'] != pinned:
        raise PrepareError(
            f'rustc {toolchain["rustc"]} does not match pinned toolchain {pinned}'
        )

    toolchain['pinnedChannel'] = pinned
    provenance = {
        'schemaVersion': 1,
        'kind': 'originlm-helper-archive',
        'name': 'originlm',
        'version': collection['version'],
        'target': target,
        'platform': info['platform'],
        'source': source,
        'toolchain': toolchain,
        'build': {'profile': 'release', 'locked': True, 'features': [], 'jobs': 2},
        'signed': False,
        'signing': {
            'authenticode': False,
            'notarized': False,
            'updateSignature': False,
            'note': 'Unsigned alpha. Checksums detect corruption; they do not authenticate the publisher.',
        },
        'compatibility': _compatibility(target),
        'license': {
            'spdx': collection['workspaceLicense'],
            'path': 'LICENSE',
            'complete': True,
        },
        'dependencies': {
            'package': collection['package'],
            'count': len(collection['components']),
            'sbom': 'sbom.cdx.json',
            'notices': 'THIRD_PARTY_NOTICES.md',
            'licenseTextsComplete': True,
        },
        'ci': _ci(),
    }

    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.prepare-', dir=out) as temporary:
        stage = pathlib.Path(temporary) / f'originlm-{target}'
        stage.mkdir()
        member = helper_member(target)
        helper_path = stage / member
        helper_path.parent.mkdir(parents=True)
        shutil.copyfile(binary, helper_path)
        helper_path.chmod(0o755)

        license_source = root / 'LICENSE'
        if not license_source.is_file():
            raise PrepareError('repository LICENSE is missing')
        shutil.copyfile(license_source, stage / 'LICENSE')
        for name in ('COPYING', 'NOTICE'):
            candidate = root / name
            if candidate.is_file():
                shutil.copyfile(candidate, stage / name)

        license_closure.write_assets(stage, collection, source['commit'])
        manifest = dict(provenance)
        manifest['members'] = _members(stage)
        manifest['archiveMemberManifest'] = 'manifest.json'
        (stage / 'manifest.json').write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + '\n', encoding='utf-8'
        )

        name = archive_name(target)
        archive = out / name
        if info['archiveSuffix'] == '.zip':
            _write_zip(stage, archive)
        else:
            _write_tar_gz(stage, archive)
        digest = _sha256(archive)
        _update_sums(out, name, digest)

        prepared_manifest = dict(manifest)
        prepared_manifest['archive'] = {
            'name': name,
            'bytes': archive.stat().st_size,
            'sha256': digest,
        }
        prepared_manifest['sha256Sums'] = 'SHA256SUMS'
        (out / 'manifest.json').write_text(
            json.dumps(prepared_manifest, indent=2, sort_keys=True) + '\n', encoding='utf-8'
        )

    return {
        'archive': name,
        'sha256': digest,
        'bytes': archive.stat().st_size,
        'version': collection['version'],
        'target': target,
        'sourceCommit': source['commit'],
        'signed': False,
        'dependencies': len(collection['components']),
        'licenseTexts': sum(len(item['licenseTexts']) for item in collection['components']),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=pathlib.Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    parser.add_argument('--root', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument('--source-commit', default=os.environ.get('GITHUB_SHA'))
    parser.add_argument('--allow-dirty', action='store_true', help='local development only')
    args = parser.parse_args()
    print(json.dumps(prepare(args.binary, args.target, args.out, args.root, args.source_commit, args.allow_dirty), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (PrepareError, license_closure.LicenseClosureError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
