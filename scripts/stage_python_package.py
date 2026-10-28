#!/usr/bin/env python3
"""Stage a platform wheel that bundles the helper binary.

    python scripts/stage_python_package.py \
      --binary dist/originlm --target x86_64-unknown-linux-gnu \
      --plat-name linux_x86_64 --out wheels

The staged copy is a normal setuptools project whose wheel is forced to be
platform-specific, because it contains a native executable. Building it needs
the same platform as the binary (the release workflow runs this per OS).
"""
import argparse
import json
import pathlib
import shutil
import sys

PLATFORMS = {
    'x86_64-unknown-linux-gnu': 'linux_x86_64',
    'aarch64-unknown-linux-gnu': 'linux_aarch64',
    'x86_64-apple-darwin': 'macosx_15_0_x86_64',
    'aarch64-apple-darwin': 'macosx_15_0_arm64',
    'x86_64-pc-windows-msvc': 'win_amd64',
}

SETUP_PY = '''from setuptools import setup
from wheel.bdist_wheel import bdist_wheel as _bdist_wheel


class bdist_wheel(_bdist_wheel):
    def finalize_options(self):
        super().finalize_options()
        # The wheel contains a native executable, so it is never "pure".
        self.root_is_pure = False

    def get_tag(self):
        # Python source is portable; only the bundled executable is platform-bound.
        _, _, platform = super().get_tag()
        return "py3", "none", platform


setup(cmdclass={"bdist_wheel": bdist_wheel})
'''


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--plat-name')
    parser.add_argument('--out', default='wheels')
    parser.add_argument('--source', default='packages/sdk-python')
    args = parser.parse_args()
    plat = args.plat_name or PLATFORMS.get(args.target)
    if not plat:
        parser.error(f'unsupported target: {args.target}')
    binary = pathlib.Path(args.binary)
    if not binary.is_file():
        parser.error(f'binary not found: {binary}')
    out = pathlib.Path(args.out) / 'originlm'
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(args.source, out, ignore=shutil.ignore_patterns('tests', '__pycache__', '*.egg-info'))
    manifest_path = out / 'pyproject.toml'
    manifest = manifest_path.read_text('utf-8')
    manifest = manifest.replace('name = "originlm"', 'name = "originlm"')
    manifest = manifest.replace('originlm = ["py.typed", "*.pyi"]', 'originlm = ["py.typed", "*.pyi", "_bin/*"]')
    manifest_path.write_text(manifest, 'utf-8')
    (out / 'setup.py').write_text(SETUP_PY, 'utf-8')
    destination = out / 'src' / 'originlm' / '_bin'
    destination.mkdir(parents=True, exist_ok=True)
    name = 'originlm.exe' if args.target.endswith('windows-msvc') else 'originlm'
    shutil.copy2(binary, destination / name)
    (destination / name).chmod(0o755)
    print(json.dumps({'package': str(out), 'plat_name': plat, 'binary': name}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
