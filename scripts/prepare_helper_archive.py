#!/usr/bin/env python3
"""Prepare one unsigned installer-compatible archive; never signs or publishes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import zipfile

TARGETS = {
    'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu',
    'x86_64-apple-darwin', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--target', choices=sorted(TARGETS), required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if not args.binary.is_file() or args.binary.is_symlink():
        parser.error('binary must be a regular file')
    size = args.binary.stat().st_size
    if not 0 < size <= 40 * 1024 * 1024:
        parser.error(f'helper size {size} exceeds the 40 MiB budget or is empty')
    windows = args.target.endswith('windows-msvc')
    executable = 'originlm.exe' if windows else 'originlm'
    if args.binary.name != executable:
        parser.error(f'expected binary filename {executable}')
    args.out.mkdir(parents=True, exist_ok=True)
    name = f'originlm-{args.target}' + ('.zip' if windows else '.tar.gz')
    archive = args.out / name
    member = f'helper-{args.target}/{executable}'
    if windows:
        with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as output:
            output.write(args.binary, member)
    else:
        with tarfile.open(archive, 'w:gz') as output:
            info = output.gettarinfo(str(args.binary), arcname=member)
            info.mode, info.uid, info.gid, info.uname, info.gname = 0o755, 0, 0, '', ''
            with args.binary.open('rb') as source:
                output.addfile(info, source)
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    # Each CI target is isolated. Concatenate these manifests when assembling a release.
    (args.out / 'SHA256SUMS').write_text(f'{digest}  {name}\n', encoding='utf-8')
    root = Path(__file__).resolve().parents[1]
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    manifest = {'target': args.target, 'sourceCommit': revision, 'signed': False,
                'helperBytes': size, 'archive': name, 'sha256': digest,
                'status': 'unsigned preview; platform execution recorded by CI separately'}
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(manifest))


if __name__ == '__main__':
    main()
