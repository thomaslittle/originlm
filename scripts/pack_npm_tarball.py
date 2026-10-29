#!/usr/bin/env python3
"""Create a deterministic npm tarball from a staged package directory.

``npm pack`` embeds platform-dependent file mtimes in the gzip/tar headers, so
five matrix jobs can produce five byte-different tarballs with identical
contents. This packer writes sorted members with zero timestamps and stable
owners/modes, which makes the universal SDK byte-identical across all targets.
"""
from __future__ import annotations

import argparse
import gzip
import json
import pathlib
import sys
import tarfile


class PackError(RuntimeError):
    pass


def pack(package, out, name=None):
    package = pathlib.Path(package).resolve()
    manifest_path = package / 'package.json'
    if package.is_symlink() or not manifest_path.is_file():
        raise PackError('package directory must contain package.json')
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    package_name = manifest.get('name')
    version = manifest.get('version')
    if not isinstance(package_name, str) or not package_name or not isinstance(version, str) or not version:
        raise PackError('package.json must declare name and version')
    if name:
        filename = name
    else:
        filename = package_name.lstrip('@').replace('/', '-') + f'-{version}.tgz'
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    destination = out / (filename if filename.endswith('.tgz') else filename + '.tgz')
    files = []
    for path in sorted(package.rglob('*')):
        if path.is_symlink():
            raise PackError(f'package member cannot be a symlink: {path}')
        if path.is_dir():
            continue
        if not path.is_file():
            raise PackError(f'package member must be a regular file: {path}')
        files.append(path)
    with destination.open('wb') as raw:
        with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0, compresslevel=9) as compressed:
            with tarfile.open(fileobj=compressed, mode='w', format=tarfile.USTAR_FORMAT) as archive:
                for path in files:
                    relative = path.relative_to(package)
                    name = 'package/' + relative.as_posix()
                    info = tarfile.TarInfo(name)
                    info.size = path.stat().st_size
                    info.mtime = 0
                    info.uid = info.gid = 0
                    info.uname = info.gname = ''
                    info.mode = 0o755 if relative.parts[:1] == ('bin',) else 0o644
                    info.type = tarfile.REGTYPE
                    with path.open('rb') as source:
                        archive.addfile(info, source)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=pathlib.Path, required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    parser.add_argument('--name')
    args = parser.parse_args()
    print(pack(args.package, args.out, args.name))


if __name__ == '__main__':
    try:
        main()
    except (PackError, OSError, ValueError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
