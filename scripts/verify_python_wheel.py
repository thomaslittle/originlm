#!/usr/bin/env python3
"""Inspect a built originlm wheel before publication.

The verifier is deliberately independent of the staging code: it opens the
wheel, checks the native binary, package-data license closure, provenance and
manylinux tag, and then runs ``auditwheel show`` and ``twine check`` when those
tools are installed. A host-specific ``linux_*`` wheel is never accepted as a
manylinux wheel.
"""
from __future__ import annotations

import argparse
import email.parser
import email.policy
import hashlib
import json
import pathlib
import re
import stat
import subprocess
import sys
import zipfile

CLASSIFIER = 'Private :: Do Not Upload'
MANYLINUX = re.compile(r'manylinux_(\d+)_(\d+)_(x86_64|aarch64)')
ELF_MACHINES = {'x86_64': 62, 'aarch64': 183}


class VerifyError(RuntimeError):
    pass


def normalize_version(value):
    return re.sub(
        r'[-_.]?(alpha|beta|preview)[-_.]?(\d*)',
        lambda match: {'alpha': 'a', 'beta': 'b', 'preview': 'rc'}[match.group(1).lower()]
        + match.group(2),
        str(value),
        flags=re.IGNORECASE,
    )


def _safe_entries(archive):
    entries = {}
    for info in archive.infolist():
        name = info.filename
        if not name or name.startswith('/') or '\\' in name or '\x00' in name:
            raise VerifyError(f'unsafe wheel member: {name!r}')
        parts = name.split('/')
        if any(part in ('', '.', '..') for part in parts):
            raise VerifyError(f'unsafe wheel member: {name!r}')
        mode = (info.external_attr >> 16) & 0xFFFF
        if stat.S_ISLNK(mode):
            raise VerifyError(f'wheel member cannot be a symlink: {name}')
        if name in entries:
            raise VerifyError(f'duplicate wheel member: {name}')
        entries[name] = info
    return entries


def _dist_info(entries):
    directories = {
        name.split('/', 1)[0]
        for name in entries
        if name.split('/', 1)[0].endswith('.dist-info')
    }
    if len(directories) != 1:
        raise VerifyError(f'expected one .dist-info directory, found {sorted(directories)}')
    directory = next(iter(directories))
    if not re.fullmatch(r'originlm-[^-]+\.dist-info', directory):
        raise VerifyError(f'unexpected wheel distribution: {directory}')
    return directory


def _header(text, name, required=True):
    message = email.parser.Parser(policy=email.policy.compat32).parsestr(text)
    values = message.get_all(name) or []
    if required and not values:
        raise VerifyError(f'wheel metadata lacks {name}')
    return values


def _elf(bytes_):
    if bytes_[:4] != b'\x7fELF':
        raise VerifyError('bundled helper is not an ELF executable')
    if len(bytes_) < 20 or bytes_[4] != 2 or bytes_[5] != 1:
        raise VerifyError('bundled helper is not a 64-bit little-endian ELF executable')
    kind = int.from_bytes(bytes_[16:18], 'little')
    machine = int.from_bytes(bytes_[18:20], 'little')
    if kind not in (2, 3):
        raise VerifyError('bundled helper is not an executable or shared object')
    return machine


def _member_map(manifest, target):
    """Map native archive members to their wheel package-data paths."""
    members = manifest.get('members')
    if not isinstance(members, dict) or not members:
        raise VerifyError('native manifest has no member checksums')
    helper = f'helper-{target}/originlm'
    mapping = {}
    for name, record in members.items():
        if not isinstance(record, dict) or type(record.get('bytes')) is not int \
                or not re.fullmatch(r'[0-9a-f]{64}', str(record.get('sha256', ''))):
            raise VerifyError(f'invalid native member record: {name!r}')
        if name == helper:
            mapping['originlm/_bin/originlm'] = record
        elif name == helper + '.exe':
            mapping['originlm/_bin/originlm.exe'] = record
        elif name.startswith('licenses/') or name in (
            'LICENSE', 'THIRD_PARTY_NOTICES.md', 'sbom.cdx.json', 'COPYING', 'NOTICE'
        ):
            mapping['originlm/' + name] = record
        else:
            raise VerifyError(f'unknown native archive member: {name!r}')
    return mapping


def _check_archive_members(archive, entries, manifest, target):
    verified = 0
    for name, record in _member_map(manifest, target).items():
        if name not in entries:
            raise VerifyError(f'wheel lacks native member {name}')
        data = archive.read(name)
        if len(data) != record['bytes']:
            raise VerifyError(f'{name} byte length differs from the native manifest')
        if hashlib.sha256(data).hexdigest() != record['sha256']:
            raise VerifyError(f'{name} SHA-256 differs from the native manifest')
        verified += 1
    return verified


def _sbom(data):
    try:
        document = json.loads(data.decode('utf-8'))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise VerifyError('sbom.cdx.json is not valid JSON') from error
    if document.get('bomFormat') != 'CycloneDX' or not isinstance(document.get('components'), list):
        raise VerifyError('sbom.cdx.json is not a CycloneDX component inventory')
    paths = []
    for component in document['components']:
        if not isinstance(component, dict) or not isinstance(component.get('name'), str):
            raise VerifyError('SBOM component is malformed')
        properties = component.get('properties') or []
        recorded = [
            item.get('value') for item in properties
            if isinstance(item, dict) and item.get('name') == 'originlm:licenseTexts'
        ]
        if not recorded:
            raise VerifyError(f'SBOM component {component["name"]} has no license text record')
        paths.extend(part for part in recorded[0].split(',') if part)
    return document, paths


def _tool(name, args, timeout=240):
    try:
        result = subprocess.run(
            [sys.executable, '-m', name, *map(str, args)],
            capture_output=True, text=True, encoding='utf-8', errors='replace',
            timeout=timeout,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise VerifyError(f'{name} could not run: {error}') from error
    return result


def manylinux_accepts(declared, observed):
    """A declared baseline accepts any earlier compatible manylinux baseline."""
    declared_match = MANYLINUX.fullmatch(declared or '')
    observed_match = MANYLINUX.fullmatch(observed or '')
    if not declared_match or not observed_match:
        return False
    if observed_match.group(3) != declared_match.group(3):
        return False
    declared_version = (int(declared_match.group(1)), int(declared_match.group(2)))
    observed_version = (int(observed_match.group(1)), int(observed_match.group(2)))
    return observed_version <= declared_version


def _auditwheel(wheel, expected_platform):
    probe = _tool('auditwheel', ['--version'])
    if probe.returncode:
        return {'available': False, 'skipped': 'auditwheel is not installed'}
    if not (expected_platform.startswith('manylinux') or expected_platform.startswith('linux_')):
        return {'available': True, 'skipped': 'not a Linux wheel'}
    result = _tool('auditwheel', ['show', wheel])
    output = (result.stdout or '') + (result.stderr or '')
    if result.returncode:
        raise VerifyError('auditwheel show failed:\n' + output[-4000:])
    match = re.search(r'consistent with the\s+following platform tag:\s*"([^"]+)"', output)
    if not match:
        raise VerifyError('auditwheel show did not confirm a platform tag:\n' + output[-4000:])
    observed = match.group(1)
    if expected_platform.startswith('manylinux') and not manylinux_accepts(
        expected_platform, observed
    ):
        raise VerifyError(
            f'auditwheel show reports {observed}, which does not support the declared '
            f'{expected_platform} baseline:\n' + output[-4000:]
        )
    if 'is not a manylinux wheel' in output or 'This wheel is not compliant' in output:
        raise VerifyError('auditwheel show rejected the wheel:\n' + output[-4000:])
    return {
        'available': True,
        'passed': True,
        'declaredTag': expected_platform,
        'observedTag': observed,
        'summary': output.strip()[-4000:],
    }


def _twine(wheel):
    probe = _tool('twine', ['--version'])
    if probe.returncode:
        return {'available': False, 'skipped': 'twine is not installed'}
    result = _tool('twine', ['check', wheel])
    output = (result.stdout or '') + (result.stderr or '')
    if result.returncode or 'PASSED' not in output:
        raise VerifyError('twine check failed:\n' + output[-4000:])
    return {'available': True, 'passed': True, 'summary': output.strip()[-4000:]}


def verify(wheel, version, *, target=None, plat_name=None, source_commit=None,
           release=False, allow_dirty=False, run_tools=True):
    wheel = pathlib.Path(wheel)
    if wheel.is_symlink() or not wheel.is_file():
        raise VerifyError(f'wheel is unavailable: {wheel}')
    match = re.fullmatch(r'originlm-([^-]+)-py3-none-(.+)\.whl', wheel.name)
    if not match:
        raise VerifyError(f'unexpected wheel filename: {wheel.name}')
    filename_version, filename_platform = match.groups()
    if normalize_version(filename_version) != normalize_version(version):
        raise VerifyError(f'wheel version is {filename_version}, expected {version}')
    if plat_name and filename_platform != plat_name:
        raise VerifyError(f'wheel platform is {filename_platform}, expected {plat_name}')
    if MANYLINUX.fullmatch(filename_platform):
        arch = MANYLINUX.fullmatch(filename_platform).group(3)
        if target and target.split('-', 1)[0] != arch:
            raise VerifyError('wheel manylinux architecture does not match the native target')
    elif release and target and target.endswith('linux-gnu'):
        raise VerifyError('a Linux release wheel must carry a qualified manylinux tag')

    with zipfile.ZipFile(wheel) as archive:
        entries = _safe_entries(archive)
        directory = _dist_info(entries)
        metadata_name = f'{directory}/METADATA'
        wheel_name = f'{directory}/WHEEL'
        if metadata_name not in entries or wheel_name not in entries:
            raise VerifyError('wheel lacks METADATA or WHEEL')
        metadata_text = archive.read(metadata_name).decode('utf-8')
        wheel_text = archive.read(wheel_name).decode('utf-8')
        if _header(metadata_text, 'Name')[0].strip() != 'originlm':
            raise VerifyError('wheel metadata Name is not originlm')
        if normalize_version(_header(metadata_text, 'Version')[0].strip()) != normalize_version(version):
            raise VerifyError('wheel metadata Version does not match the filename')
        classifiers = [value.strip() for value in _header(metadata_text, 'Classifier', required=False)]
        has_guard = CLASSIFIER in classifiers
        if release and has_guard:
            raise VerifyError('publishable wheel still carries the Private :: Do Not Upload classifier')
        if not release and not has_guard:
            raise VerifyError('preview wheel lost the Private :: Do Not Upload classifier')
        if _header(metadata_text, 'Requires-Python')[0].strip() != '>=3.11':
            raise VerifyError('wheel metadata Requires-Python is not >=3.11')
        tags = [value.strip() for value in _header(wheel_text, 'Tag')]
        expected_tag = f'py3-none-{filename_platform}'
        if tags != [expected_tag]:
            raise VerifyError(f'wheel Tag is {tags}, expected [{expected_tag!r}]')
        if 'Root-Is-Purelib: true' in wheel_text:
            raise VerifyError('native wheel is marked as pure Python')

        provenance_path = 'originlm/PROVENANCE.json'
        if provenance_path not in entries:
            raise VerifyError('wheel lacks originlm/PROVENANCE.json')
        try:
            provenance = json.loads(archive.read(provenance_path).decode('utf-8'))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise VerifyError('PROVENANCE.json is not valid JSON') from error
        if provenance.get('kind') != 'originlm-python-wheel':
            raise VerifyError('PROVENANCE.json is not a Python wheel provenance record')
        if normalize_version(provenance.get('version')) != normalize_version(version):
            raise VerifyError('PROVENANCE.json version does not match the wheel')
        if provenance.get('platform') != filename_platform:
            raise VerifyError('PROVENANCE.json platform does not match the wheel tag')
        if provenance.get('release') is not bool(release):
            raise VerifyError('PROVENANCE.json release flag does not match the invocation')
        if plat_name and provenance.get('platform') != plat_name:
            raise VerifyError('PROVENANCE.json platform does not match the expected platform')
        source = provenance.get('source') or {}
        if source_commit and source.get('commit') != source_commit:
            raise VerifyError('PROVENANCE.json source commit does not match')
        if release and source.get('workingTreeDirty') is not False and not allow_dirty:
            raise VerifyError('publishable wheel was staged from a dirty source tree')

        expected_helper = 'originlm/_bin/originlm.exe' if (
            target or provenance.get('target', '')
        ).endswith('windows-msvc') else 'originlm/_bin/originlm'
        if expected_helper not in entries:
            raise VerifyError(f'wheel lacks the bundled native helper {expected_helper}')
        helper = archive.read(expected_helper)
        if not helper:
            raise VerifyError('bundled native helper is empty')
        if target and target.endswith('linux-gnu'):
            machine = _elf(helper)
            expected_machine = ELF_MACHINES[target.split('-', 1)[0]]
            if machine != expected_machine:
                raise VerifyError('bundled ELF architecture does not match the target')
            mode = (entries[expected_helper].external_attr >> 16) & 0o777
            if mode & 0o111 != 0o111:
                raise VerifyError('bundled native helper is not executable in the wheel')
        license_texts = 0
        if release:
            for required in (
                'originlm/LICENSE', 'originlm/THIRD_PARTY_NOTICES.md',
                'originlm/sbom.cdx.json', 'originlm/manifest.json',
            ):
                if required not in entries:
                    raise VerifyError(f'publishable wheel lacks {required}')
            try:
                manifest = json.loads(archive.read('originlm/manifest.json').decode('utf-8'))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise VerifyError('native manifest is not valid JSON') from error
            native_target = manifest.get('target')
            if target and native_target != target:
                raise VerifyError('native manifest target does not match')
            if normalize_version(manifest.get('version')) != normalize_version(version):
                raise VerifyError('native manifest version does not match the wheel')
            if (manifest.get('source') or {}).get('commit') != source.get('commit'):
                raise VerifyError('native manifest source commit differs from provenance')
            if not isinstance((manifest.get('license') or {}).get('spdx'), str):
                raise VerifyError('native manifest has no license expression')
            native_sha = hashlib.sha256(archive.read('originlm/manifest.json')).hexdigest()
            if (provenance.get('native') or {}).get('manifestSha256') != native_sha:
                raise VerifyError('PROVENANCE.json does not pin the native manifest')
            verified = _check_archive_members(archive, entries, manifest, native_target)
            if verified < 2:
                raise VerifyError('native manifest carries no useful member checksums')
            document, paths = _sbom(archive.read('originlm/sbom.cdx.json'))
            dependencies = manifest.get('dependencies') or {}
            if dependencies.get('count') != len(document['components']):
                raise VerifyError('SBOM component count differs from the native manifest')
            for path in paths:
                if 'originlm/' + path not in entries:
                    raise VerifyError(f'SBOM references a missing license text: {path}')
            license_texts = sum(
                1 for name in entries if name.startswith('originlm/licenses/')
            )
            if license_texts == 0:
                raise VerifyError('publishable wheel has no dependency license texts')
            if license_texts != sum(
                1 for name in _member_map(manifest, native_target)
                if name.startswith('originlm/licenses/')
            ):
                raise VerifyError('license text count differs from the native manifest')
        else:
            license_texts = sum(
                1 for name in entries if name.startswith('originlm/licenses/')
            )

    auditwheel = _auditwheel(wheel, filename_platform) if run_tools else {
        'available': False, 'skipped': 'disabled by the caller'
    }
    twine = _twine(wheel) if run_tools else {
        'available': False, 'skipped': 'disabled by the caller'
    }
    return {
        'status': 'pass',
        'wheel': wheel.name,
        'sha256': hashlib.sha256(wheel.read_bytes()).hexdigest(),
        'version': version,
        'target': target or provenance.get('target'),
        'platform': filename_platform,
        'manylinux': bool(MANYLINUX.fullmatch(filename_platform)),
        'release': bool(release),
        'uploadGuard': 'removed' if release else CLASSIFIER,
        'sourceCommit': source.get('commit'),
        'nativeHelper': expected_helper,
        'licenseTexts': license_texts,
        'auditwheel': auditwheel,
        'twine': twine,
        'limitations': [
            'No registry upload was attempted.',
            'auditwheel and twine checks run only when installed in the verifying environment.',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wheel', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--target')
    parser.add_argument('--plat-name')
    parser.add_argument('--source-commit')
    parser.add_argument('--release', action='store_true')
    parser.add_argument('--allow-dirty', action='store_true')
    parser.add_argument('--skip-tools', action='store_true')
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    evidence = verify(
        args.wheel, args.version, target=args.target, plat_name=args.plat_name,
        source_commit=args.source_commit, release=args.release,
        allow_dirty=args.allow_dirty, run_tools=not args.skip_tools,
    )
    text = json.dumps(evidence, indent=2, sort_keys=True) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
    print(text, end='')


if __name__ == '__main__':
    try:
        main()
    except (VerifyError, OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
