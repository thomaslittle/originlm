#!/usr/bin/env python3
"""Check that a GitHub release carries the complete helper asset set.

docs/release/PUBLISHING.md requires every prerelease to attach, for each
prepared target, the helper archive and its `<target>-manifest.json`, plus one
`SHA256SUMS` covering all archives. Consumers that pin a helper by checksum
depend on those files; an archive published without them cannot be verified
against anything the release itself states.

This reads a directory of downloaded release assets and fails when a file is
missing, when `SHA256SUMS` or a manifest disagrees with the archive bytes, or
when the manifests do not all describe the same version and source commit. It
does not download, publish, sign or modify anything.
"""
import argparse
import hashlib
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from release_targets import TARGETS, archive_name  # noqa: E402

SUMS = 'SHA256SUMS'


def manifest_name(target):
    return f'{target}-manifest.json'


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def read_sums(path):
    listed = {}
    for number, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        if not line.strip():
            continue
        parts = line.split()
        if len(parts) != 2 or len(parts[0]) != 64:
            raise ValueError(f'{SUMS} line {number} is not "<sha256>  <name>"')
        name = parts[1].lstrip('*')
        if name in listed:
            raise ValueError(f'{SUMS} lists {name} more than once')
        listed[name] = parts[0].lower()
    return listed


def verify(assets, version=None):
    """Return a list of problems; empty means the asset set is complete."""
    assets = pathlib.Path(assets)
    problems = []
    sums_path = assets / SUMS
    listed = {}
    if not sums_path.is_file():
        problems.append(f'{SUMS} is missing')
    else:
        try:
            listed = read_sums(sums_path)
        except ValueError as error:
            problems.append(str(error))
    expected = {archive_name(target) for target in TARGETS}
    for extra in sorted(set(listed) - expected):
        problems.append(f'{SUMS} lists {extra}, which is not a helper archive')
    versions, commits = set(), set()
    for target in TARGETS:
        archive = assets / archive_name(target)
        if not archive.is_file():
            problems.append(f'{archive.name} is missing')
            continue
        size, digest = archive.stat().st_size, sha256(archive)
        if archive.name not in listed:
            if sums_path.is_file():
                problems.append(f'{SUMS} does not list {archive.name}')
        elif listed[archive.name] != digest:
            problems.append(f'{SUMS} disagrees with the bytes of {archive.name}')
        manifest_path = assets / manifest_name(target)
        if not manifest_path.is_file():
            problems.append(f'{manifest_path.name} is missing')
            continue
        try:
            manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
            recorded = manifest['archive']
            if manifest.get('target') != target:
                problems.append(f'{manifest_path.name} describes target {manifest.get("target")!r}')
            if (recorded.get('name'), recorded.get('bytes'), recorded.get('sha256')) != (
                    archive.name, size, digest):
                problems.append(f'{manifest_path.name} disagrees with the bytes of {archive.name}')
            source = manifest.get('source') or {}
            if source.get('workingTreeDirty') is not False:
                problems.append(f'{manifest_path.name} does not record a clean source tree')
            versions.add(manifest.get('version'))
            commits.add(source.get('commit'))
        except (ValueError, KeyError, TypeError, AttributeError):
            problems.append(f'{manifest_path.name} is not a helper archive manifest')
    if len(versions) > 1:
        problems.append(f'manifests describe several versions: {sorted(map(str, versions))}')
    if len(commits) > 1:
        problems.append(f'manifests describe several source commits: {sorted(map(str, commits))}')
    if version is not None and versions and versions != {version}:
        problems.append(f'manifests describe {sorted(map(str, versions))}, expected {version}')
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets', required=True, type=pathlib.Path,
                        help='directory holding the downloaded release assets')
    parser.add_argument('--version', help='version every manifest must state (a tag without its leading v)')
    args = parser.parse_args(argv)
    problems = verify(args.assets, args.version)
    for problem in problems:
        print(f'release asset set: {problem}', file=sys.stderr)
    if problems:
        return 1
    print(f'release asset set complete: {len(TARGETS)} archives, {len(TARGETS)} manifests, {SUMS}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
