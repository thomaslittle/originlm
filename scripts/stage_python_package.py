#!/usr/bin/env python3
"""Stage a platform wheel that bundles the qualified native helper.

Preview staging (the default) keeps the ``Private :: Do Not Upload`` classifier
and a host-specific platform tag. A publishable copy is produced only with an
explicit ``--release`` flag, which also requires the helper metadata directory
written by ``prepare_helper_archive.py`` (LICENSE, notices, SBOM, provenance
manifest and every dependency license text).

    python scripts/stage_python_package.py \
      --binary dist/originlm --target x86_64-unknown-linux-gnu \
      --plat-name linux_x86_64 --out wheels

    python scripts/stage_python_package.py \
      --binary dist/originlm --target x86_64-unknown-linux-gnu \
      --plat-name manylinux_2_28_x86_64 --metadata prepared/metadata/x86_64-unknown-linux-gnu \
      --release --out wheels

A ``manylinux_*`` tag is refused unless the script is running inside the
matching official manylinux image (AlmaLinux/Rocky/CentOS/RHEL with
``/opt/python``) and the host glibc is no newer than the declared baseline.
Ubuntu-built wheels are never retagged as manylinux.
"""
import argparse
import hashlib
import json
import pathlib
import platform
import re
import shutil
import subprocess
import sys

PLATFORMS = {
    'x86_64-unknown-linux-gnu': 'linux_x86_64',
    'aarch64-unknown-linux-gnu': 'linux_aarch64',
    'x86_64-apple-darwin': 'macosx_15_0_x86_64',
    'aarch64-apple-darwin': 'macosx_15_0_arm64',
    'x86_64-pc-windows-msvc': 'win_amd64',
}

MANYLINUX = re.compile(r'manylinux_(\d+)_(\d+)_(x86_64|aarch64)')
MANYLINUX_OS_IDS = ('almalinux', 'rocky', 'centos', 'rhel')

CLASSIFIER = 'Private :: Do Not Upload'

SETUP_PY = '''from pathlib import Path

from setuptools import Distribution, setup
from wheel.bdist_wheel import bdist_wheel as _bdist_wheel

PACKAGE = Path(__file__).parent / "src" / "originlm"


def package_data():
    """Every non-Python file, including the recursively nested license texts."""
    files = []
    for path in PACKAGE.rglob("*"):
        if not path.is_file() or "__pycache__" in path.parts:
            continue
        if path.suffix in (".py", ".pyc"):
            continue
        files.append(path.relative_to(PACKAGE).as_posix())
    return sorted(files)


class PlatformDistribution(Distribution):
    """Keep the package at the wheel root even though it has no C extension."""

    def has_ext_modules(self):
        return True


class bdist_wheel(_bdist_wheel):
    def finalize_options(self):
        super().finalize_options()
        # The wheel contains a native executable, so it is never "pure".
        self.root_is_pure = False

    def get_tag(self):
        # Python source is portable; only the bundled executable is platform-bound.
        _, _, platform = super().get_tag()
        return "py3", "none", platform


setup(
    distclass=PlatformDistribution,
    package_data={"originlm": package_data()},
    cmdclass={"bdist_wheel": bdist_wheel},
)
'''


class StageError(RuntimeError):
    pass


def _sha256(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def _run(command, root):
    try:
        result = subprocess.run(
            command, cwd=root, capture_output=True, text=True, encoding='utf-8',
            errors='replace', timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise StageError(f'command failed: {" ".join(map(str, command))}') from error
    if result.returncode:
        raise StageError(f'command failed: {" ".join(map(str, command))}')
    return result.stdout


def _read_glibc():
    for command in (['getconf', 'GNU_LIBC_VERSION'], ['ldd', '--version']):
        try:
            result = subprocess.run(
                command, capture_output=True, text=True, encoding='utf-8',
                errors='replace', timeout=30,
            )
        except OSError:
            continue
        text = (result.stdout or '') + (result.stderr or '')
        match = re.search(r'glibc[^0-9]*([0-9]+\.[0-9]+)', text, re.IGNORECASE)
        if not match:
            match = re.search(r'GLIBC[^0-9]*([0-9]+\.[0-9]+)', text, re.IGNORECASE)
        if match:
            return match.group(1)
    return None


def _read_os_release():
    path = pathlib.Path('/etc/os-release')
    if not path.is_file():
        return None
    for line in path.read_text(encoding='utf-8', errors='replace').splitlines():
        if line.startswith('ID='):
            return line.split('=', 1)[1].strip().strip('"').lower()
    return None


def _version(value):
    try:
        return tuple(int(part) for part in str(value).split('.'))
    except (TypeError, ValueError):
        return None


def check_manylinux_environment(plat, target, *, machine, os_release_id,
                                glibc, image_marker):
    """Refuse a manylinux claim that the build environment does not support.

    Pure inputs make this testable; the CLI supplies the real host values.
    """
    match = MANYLINUX.fullmatch(plat or '')
    if not match:
        raise StageError(f'not a manylinux platform tag: {plat!r}')
    baseline = (int(match.group(1)), int(match.group(2)))
    arch = match.group(3)
    target_arch = target.split('-', 1)[0]
    if target_arch != arch:
        raise StageError(
            f'platform tag {plat} does not match target architecture {target_arch}'
        )
    normalized = {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(
        str(machine).lower(), str(machine).lower()
    )
    if normalized != arch:
        raise StageError(
            f'platform tag {plat} was requested on a {machine!r} build host'
        )
    if not image_marker:
        raise StageError(
            'manylinux tags require an official manylinux image (missing /opt/python); '
            'build in quay.io/pypa/manylinux_2_28_* instead of retagging a host wheel'
        )
    if os_release_id not in MANYLINUX_OS_IDS:
        raise StageError(
            f'manylinux tags require an official manylinux build image, not {os_release_id!r}'
        )
    actual = _version(glibc)
    if actual is None:
        raise StageError('cannot determine the build host glibc for manylinux qualification')
    if actual > baseline:
        raise StageError(
            f'build host glibc {".".join(map(str, actual))} exceeds the declared '
            f'manylinux baseline {".".join(map(str, baseline))}'
        )
    return {
        'tag': plat,
        'baseline': '.'.join(map(str, baseline)),
        'hostGlibc': '.'.join(map(str, actual)),
        'image': f'quay.io/pypa/{plat}',
    }


def verify_manylinux_environment(plat, target):
    """Fail unless the current process really is inside the matching image."""
    return check_manylinux_environment(
        plat,
        target,
        machine=platform.machine(),
        os_release_id=_read_os_release(),
        glibc=_read_glibc(),
        image_marker=pathlib.Path('/opt/python').is_dir(),
    )


def apply_upload_guard(manifest_text, release):
    """Remove the publication guard only for an explicit release staging run."""
    guard = f'"{CLASSIFIER}"'
    if release:
        if guard not in manifest_text:
            raise StageError('publishable staging expected the upload-guard classifier to remove')
        manifest_text = manifest_text.replace(f'{guard}, ', '')
        manifest_text = manifest_text.replace(f', {guard}', '')
        manifest_text = manifest_text.replace(guard, '')
    elif guard not in manifest_text:
        raise StageError('preview staging requires the Private :: Do Not Upload classifier')
    return manifest_text


def _require_regular(path, label):
    path = pathlib.Path(path)
    if path.is_symlink() or not path.is_file():
        raise StageError(f'{label} is missing or is not a regular file: {path}')
    return path


def _copy_metadata(metadata, package_root, target):
    metadata = pathlib.Path(metadata).resolve()
    if metadata.is_symlink() or not metadata.is_dir():
        raise StageError(f'metadata directory is unavailable: {metadata}')
    required = ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'sbom.cdx.json', 'manifest.json')
    for name in required:
        _require_regular(metadata / name, name)
    licenses = metadata / 'licenses'
    if licenses.is_symlink() or not licenses.is_dir():
        raise StageError('metadata directory lacks dependency license texts')
    texts = [path for path in licenses.rglob('*') if path.is_file()]
    if not texts:
        raise StageError('metadata directory contains no dependency license texts')
    if any(path.is_symlink() for path in licenses.rglob('*')):
        raise StageError('dependency license texts must not be symlinks')

    manifest = json.loads((metadata / 'manifest.json').read_text(encoding='utf-8'))
    if not isinstance(manifest, dict) or manifest.get('target') != target:
        raise StageError('native manifest target does not match the wheel target')
    if manifest.get('kind') != 'originlm-helper-archive':
        raise StageError('native manifest is not a helper archive manifest')

    for name in required:
        shutil.copyfile(metadata / name, package_root / name)
    shutil.copytree(licenses, package_root / 'licenses')
    return {
        'manifest': manifest,
        'manifestSha256': _sha256(metadata / 'manifest.json'),
        'licenseTexts': len(texts),
    }


def _source_state(root, source_commit, release, allow_dirty):
    commit = source_commit or None
    dirty = None
    try:
        commit = commit or _run(['git', 'rev-parse', 'HEAD'], root).strip()
        # Generated output directories are untracked; only tracked edits make the
        # source unsuitable for release staging.
        dirty = bool(_run(['git', 'status', '--porcelain', '--untracked-files=no'], root).strip())
    except StageError:
        if commit is None:
            if release:
                raise StageError('release staging requires a git checkout to record provenance') from None
        else:
            dirty = None
    if release and dirty is not False and not allow_dirty:
        raise StageError(
            'release staging requires a clean git source tree to prove provenance '
            '(or --allow-dirty for local development only)'
        )
    return {'commit': commit, 'workingTreeDirty': dirty}


def stage(binary, target, out, root, *, plat_name=None, metadata=None,
          release=False, source_commit=None, allow_dirty=False, source=None):
    root = pathlib.Path(root).resolve()
    out = pathlib.Path(out).resolve()
    binary = pathlib.Path(binary).resolve()
    source = pathlib.Path(source) if source else root / 'packages/sdk-python'
    plat = plat_name or PLATFORMS.get(target)
    if not plat:
        raise StageError(f'unsupported target: {target}')
    if MANYLINUX.fullmatch(plat):
        verify_manylinux_environment(plat, target)
    elif target.endswith('linux-gnu') and plat not in ('linux_x86_64', 'linux_aarch64'):
        raise StageError(f'unsupported Linux wheel platform tag: {plat}')
    _require_regular(binary, 'helper binary')
    if source.is_symlink() or not (source / 'pyproject.toml').is_file():
        raise StageError(f'SDK source is unavailable: {source}')
    if release and not metadata:
        raise StageError('--release staging requires --metadata from prepare_helper_archive.py')

    staged = out / 'originlm'
    if staged.exists():
        shutil.rmtree(staged)
    shutil.copytree(
        source, staged,
        ignore=shutil.ignore_patterns('tests', '__pycache__', '*.egg-info', '_bin',
                                      'LICENSE', 'THIRD_PARTY_NOTICES.md',
                                      'sbom.cdx.json', 'manifest.json', 'PROVENANCE.json',
                                      'licenses'),
    )
    manifest_path = staged / 'pyproject.toml'
    manifest_text = manifest_path.read_text('utf-8')
    manifest_text = apply_upload_guard(manifest_text, release)
    manifest_text = manifest_text.replace(
        'originlm = ["py.typed", "*.pyi"]',
        'originlm = ["py.typed", "*.pyi", "_bin/*", "LICENSE", '
        '"THIRD_PARTY_NOTICES.md", "sbom.cdx.json", "manifest.json", '
        '"PROVENANCE.json", "licenses/*/*"]',
    )
    manifest_path.write_text(manifest_text, 'utf-8')
    (staged / 'setup.py').write_text(SETUP_PY, 'utf-8')

    package_root = staged / 'src' / 'originlm'
    package_root.mkdir(parents=True, exist_ok=True)
    destination = package_root / '_bin'
    destination.mkdir(parents=True, exist_ok=True)
    name = 'originlm.exe' if target.endswith('windows-msvc') else 'originlm'
    shutil.copy2(binary, destination / name)
    (destination / name).chmod(0o755)

    version_match = re.search(r'^version\s*=\s*"([^"]+)"', manifest_text, re.MULTILINE)
    if not version_match:
        raise StageError('SDK pyproject.toml has no static project version')
    version = version_match.group(1)

    native = None
    if metadata:
        native = _copy_metadata(metadata, package_root, target)
    else:
        license_source = root / 'LICENSE'
        _require_regular(license_source, 'repository LICENSE')
        shutil.copyfile(license_source, package_root / 'LICENSE')

    provenance = {
        'schemaVersion': 1,
        'kind': 'originlm-python-wheel',
        'name': 'originlm',
        'version': version,
        'target': target,
        'platform': plat,
        'pythonTag': 'py3',
        'abiTag': 'none',
        'release': bool(release),
        'publishable': bool(release),
        'uploadGuard': 'removed' if release else 'Private :: Do Not Upload',
        'source': _source_state(root, source_commit, release, allow_dirty),
        'native': None if native is None else {
            'manifest': 'manifest.json',
            'manifestSha256': native['manifestSha256'],
            'version': native['manifest'].get('version'),
            'target': native['manifest'].get('target'),
            'helper': f'_bin/{name}',
        },
        'dependencies': None if native is None else {
            'complete': (native['manifest'].get('dependencies') or {}).get('licenseTextsComplete') is True,
            'count': (native['manifest'].get('dependencies') or {}).get('count'),
            'sbom': 'sbom.cdx.json',
            'notices': 'THIRD_PARTY_NOTICES.md',
            'licenseTexts': 'licenses/',
            'licenseTextCount': native['licenseTexts'],
        },
        'compatibility': {
            'manylinux': plat if MANYLINUX.fullmatch(plat) else None,
            'declaredPlatform': plat,
            'basis': (
                'staged inside the official manylinux_2_28 image; auditwheel evidence is recorded separately'
                if MANYLINUX.fullmatch(plat) else
                'host-specific platform tag; not a manylinux claim'
            ),
        },
    }
    (package_root / 'PROVENANCE.json').write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + '\n', encoding='utf-8'
    )
    return {
        'package': str(staged),
        'plat_name': plat,
        'binary': name,
        'version': version,
        'release': bool(release),
        'metadata': native is not None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--plat-name')
    parser.add_argument('--out', default='wheels')
    parser.add_argument('--source', default='packages/sdk-python')
    parser.add_argument('--root', default=None)
    parser.add_argument('--metadata', help='helper metadata directory from prepare_helper_archive.py')
    parser.add_argument('--release', action='store_true',
                        help='remove Private :: Do Not Upload and require qualified metadata')
    parser.add_argument('--source-commit', default=None)
    parser.add_argument('--allow-dirty', action='store_true', help='local development only')
    args = parser.parse_args()
    root = pathlib.Path(args.root) if args.root else pathlib.Path(__file__).resolve().parents[1]
    print(json.dumps(stage(
        args.binary, args.target, args.out, root,
        plat_name=args.plat_name, metadata=args.metadata, release=args.release,
        source_commit=args.source_commit, allow_dirty=args.allow_dirty,
        source=root / args.source if not pathlib.Path(args.source).is_absolute() else args.source,
    ), indent=2))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (StageError, OSError, ValueError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
