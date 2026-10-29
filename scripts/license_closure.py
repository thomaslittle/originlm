#!/usr/bin/env python3
"""Resolve an exact Cargo dependency closure and collect its license texts.

The release archive is built from one binary, so the inventory is the resolved
runtime/build closure of ``originlm-cli`` for one target, not every crate in the
workspace. Cargo metadata supplies package identity, source and license
expressions. ``cargo tree`` supplies the feature-accurate reachable set for the
same manifest and target (Cargo metadata's workspace-wide resolve includes
optional features enabled only by unrelated workspace members).

Collection is deliberately strict: a component with no redistributable license
text fails the build. It never invents an SPDX string, downloads an unknown
binary, or copies a text from a different package.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

MAX_LICENSE_BYTES = 1024 * 1024
LICENSE_PREFIXES = ('LICENSE', 'COPYING', 'NOTICE')
SKIP_DIRS = {'target', 'node_modules', '.git', 'third_party', 'third-party', 'vendor'}
SAFE_NAME = re.compile(r'^[A-Za-z0-9._+-]+$')
TREE_LINE = re.compile(r'^(\S+) v([^ ]+)(?: \((.*)\))?$')


class LicenseClosureError(RuntimeError):
    pass


def _run(command, root):
    result = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        text=True,
        encoding='utf-8',
        errors='replace',
        timeout=300,
    )
    if result.returncode:
        raise LicenseClosureError(
            'command failed: ' + ' '.join(map(str, command)) + '\n'
            + result.stdout[-4000:] + result.stderr[-4000:]
        )
    return result.stdout


def cargo_metadata(root, target):
    return json.loads(_run([
        'cargo', 'metadata', '--locked', '--format-version', '1',
        '--filter-platform', target,
    ], root))


def resolved_closure(root, target, package_name):
    """Return exact reachable packages for the package's normal/build graph.

    Cargo has no JSON output for a single package's feature-resolved graph that
    excludes unrelated workspace feature unification. ``cargo tree`` is Cargo's
    own resolver view; every package it names is then looked up in metadata.
    """
    output = _run([
        'cargo', 'tree', '-p', package_name, '--target', target,
        '--edges', 'normal,build', '--prefix', 'none', '--no-dedupe',
        '--format', '{p}',
    ], root)
    selected = {}
    for line in output.splitlines():
        line = line.strip()
        if not line or line.startswith('warning:'):
            continue
        match = TREE_LINE.match(line)
        if not match:
            raise LicenseClosureError(f'unexpected cargo tree line: {line}')
        name, version, source = match.groups()
        selected[(name, version, source)] = None
    return selected


def _match_package(metadata, name, version, source_hint):
    candidates = [
        package for package in metadata['packages']
        if package['name'] == name and package['version'] == version
    ]
    if source_hint and (os.path.isabs(source_hint) or re.match(r'^[A-Za-z]:[\\/]', source_hint)):
        resolved = pathlib.Path(source_hint).resolve()
        candidates = [
            package for package in candidates
            if pathlib.Path(package['manifest_path']).parent.resolve() == resolved
        ]
    elif source_hint and ('git+' in source_hint or '://' in source_hint):
        candidates = [
            package for package in candidates
            if package.get('source') and (source_hint in package['source'] or package['source'] in source_hint)
        ]
    else:
        candidates = [
            package for package in candidates
            if (package.get('source') or '').startswith(('registry+', 'sparse+'))
        ]
    if len(candidates) != 1:
        raise LicenseClosureError(
            f'cannot map cargo tree package {name} {version} ({source_hint or "registry"}) '
            f'to exactly one metadata package; found {len(candidates)}'
        )
    return candidates[0]


def _regular_license_file(path):
    try:
        path = pathlib.Path(path)
        if path.is_symlink() or not path.is_file():
            return False
        return 0 < path.stat().st_size <= MAX_LICENSE_BYTES
    except OSError:
        return False


def _root_license_files(directory):
    return [
        path for path in sorted(pathlib.Path(directory).iterdir())
        if path.is_file() and not path.is_symlink()
        and path.name.upper().startswith(LICENSE_PREFIXES)
        and _regular_license_file(path)
    ]


def _nested_license_files(directory):
    directory = pathlib.Path(directory)
    found = []
    for path in sorted(directory.rglob('*')):
        relative = path.relative_to(directory)
        if any(part.lower() in SKIP_DIRS for part in relative.parts[:-1]):
            continue
        if len(relative.parts) > 3:
            continue
        if path.is_file() and not path.is_symlink() and path.name.upper().startswith(LICENSE_PREFIXES):
            if _regular_license_file(path):
                found.append(path)
    return found


def _workspace_license(repo_root):
    manifest = repo_root / 'Cargo.toml'
    text = manifest.read_text(encoding='utf-8')
    match = re.search(r'^\s*license\s*=\s*"([^"]+)"\s*$', text, re.MULTILINE)
    return match.group(1) if match else None


def _license_files(package, repo_root, workspace_license):
    """Find all redistributable text for this package without guessing."""
    manifest_dir = pathlib.Path(package['manifest_path']).parent
    explicit = package.get('license_file')
    if explicit:
        raw = pathlib.Path(explicit)
        for candidate in ((manifest_dir / raw).resolve(), (repo_root / raw).resolve()):
            if _regular_license_file(candidate):
                try:
                    relative = candidate.resolve().relative_to(manifest_dir.resolve())
                except ValueError:
                    relative = pathlib.Path(candidate.name)
                return [(candidate, relative)]
    root_files = _root_license_files(manifest_dir)
    if root_files:
        return [(path, pathlib.Path(path.name)) for path in root_files]
    if package.get('source') is None:
        # Workspace members inherit the repository LICENSE through workspace
        # inheritance. Only reuse it when the declared expression is exactly the
        # shared license, so a different/multi-license package cannot silently
        # receive the wrong text.
        if workspace_license and package.get('license') == workspace_license:
            shared = _root_license_files(repo_root)
            if shared:
                return [(path, pathlib.Path(path.name)) for path in shared]
    nested = _nested_license_files(manifest_dir)
    if nested:
        return [
            (path, path.relative_to(manifest_dir))
            for path in nested
        ]
    raise LicenseClosureError(
        f'no license text for {package["name"]} {package["version"]} '
        f'(license={package.get("license")!r}, source={package.get("source")!r})'
    )


def _safe_member_path(directory, relative):
    relative = pathlib.PurePosixPath(relative.as_posix())
    if relative.is_absolute() or any(part in ('', '.', '..') for part in relative.parts):
        raise LicenseClosureError(f'unsafe license member path: {relative}')
    for part in relative.parts:
        if not SAFE_NAME.match(part):
            raise LicenseClosureError(f'unsafe license member path segment: {part!r}')
    return f'{directory}/' + '/'.join(relative.parts)


def _source(package, repo_root):
    if package.get('source') is None:
        manifest_dir = pathlib.Path(package['manifest_path']).parent
        try:
            path = manifest_dir.resolve().relative_to(repo_root.resolve()).as_posix()
        except ValueError:
            path = package['name']
        return {'kind': 'workspace', 'path': path}
    source = package['source']
    if source.startswith(('registry+', 'sparse+')):
        return {
            'kind': 'registry',
            'url': f'https://crates.io/crates/{package["name"]}/{package["version"]}',
        }
    if source.startswith('git+'):
        return {'kind': 'git', 'url': source[4:]}
    return {'kind': 'other', 'url': source}


def _toolchain(root):
    rustc = _run(['rustc', '-vV'], root)
    cargo = _run(['cargo', '--version'], root).strip()
    cargo_version = cargo.split()[1] if len(cargo.split()) > 1 else cargo
    release = next(
        (line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('release: ')),
        None,
    )
    commit = next(
        (line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('commit-hash: ')),
        None,
    )
    host = next(
        (line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('host: ')),
        None,
    )
    llvm = next(
        (line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('LLVM version: ')),
        None,
    )
    return {'rustc': release, 'rustcCommit': commit, 'cargo': cargo_version, 'host': host, 'llvm': llvm}


def collect(root, target, package_name='originlm-cli'):
    root = pathlib.Path(root).resolve()
    metadata = cargo_metadata(root, target)
    matches = [
        package for package in metadata['packages']
        if package['name'] == package_name and package.get('source') is None
    ]
    if len(matches) != 1:
        raise LicenseClosureError(f'expected one workspace package named {package_name}')
    root_package = matches[0]
    workspace_license = _workspace_license(root)
    closure = resolved_closure(root, target, package_name)
    packages = [
        _match_package(metadata, name, version, source)
        for (name, version, source) in sorted(closure)
    ]
    components = []
    missing = []
    texts = {}
    for package in sorted(packages, key=lambda item: (item['name'], item['version'], item.get('source') or '')):
        expression = package.get('license')
        if not expression:
            missing.append({'name': package['name'], 'version': package['version'], 'reason': 'no license expression'})
            continue
        try:
            files = _license_files(package, root, workspace_license)
        except LicenseClosureError as error:
            missing.append({'name': package['name'], 'version': package['version'], 'reason': str(error)})
            continue
        component_dir = 'licenses/' + re.sub(
            r'[^A-Za-z0-9._+-]', '_', f'{package["name"]}-{package["version"]}'
        )
        records = []
        for path, relative in files:
            data = path.read_bytes()
            try:
                text = data.decode('utf-8')
            except UnicodeDecodeError:
                raise LicenseClosureError(
                    f'license text is not UTF-8 for {package["name"]} {package["version"]}: {relative}'
                ) from None
            member = _safe_member_path(component_dir, relative)
            digest = hashlib.sha256(data).hexdigest()
            records.append({
                'path': member,
                'bytes': len(data),
                'sha256': digest,
                'sourcePath': relative.as_posix(),
                '_source': str(path),
            })
            texts.setdefault(digest, {'digest': digest, 'text': text, 'paths': []})
            texts[digest]['paths'].append(member)
        components.append({
            'name': package['name'],
            'version': package['version'],
            'purl': f'pkg:cargo/{package["name"]}@{package["version"]}',
            'license': expression,
            'source': _source(package, root),
            'licenseTexts': records,
        })
    if missing:
        raise LicenseClosureError(
            'required license material is missing:\n'
            + '\n'.join(f'- {item["name"]} {item["version"]}: {item["reason"]}' for item in missing)
        )
    return {
        'package': package_name,
        'version': root_package['version'],
        'target': target,
        'workspaceLicense': workspace_license,
        'components': components,
        'texts': [texts[digest] for digest in sorted(texts)],
        'toolchain': _toolchain(root),
    }


def collect_many(root, target, package_names):
    """Collect a union closure for several shipped binaries in one bundle."""
    root = pathlib.Path(root).resolve()
    merged = None
    for package_name in package_names:
        current = collect(root, target, package_name)
        if merged is None:
            merged = current
            continue
        components = {component['purl']: component for component in merged['components']}
        for component in current['components']:
            if component['purl'] in components:
                existing = components[component['purl']]
                if existing['license'] != component['license']:
                    raise LicenseClosureError(f'conflicting license expressions for {component["purl"]}')
                known = {record['path'] for record in existing['licenseTexts']}
                existing['licenseTexts'].extend(
                    record for record in component['licenseTexts'] if record['path'] not in known
                )
            else:
                components[component['purl']] = component
        merged['components'] = sorted(components.values(), key=lambda item: item['purl'])
        texts = {item['digest']: item for item in merged['texts']}
        for item in current['texts']:
            key = item['digest']
            if key in texts:
                texts[key]['paths'].extend(path for path in item['paths'] if path not in texts[key]['paths'])
            else:
                texts[key] = item
        merged['texts'] = [texts[key] for key in sorted(texts)]
    return merged


def notices(collection, source_commit):
    version = collection['version']
    target = collection['target']
    lines = [
        '# Third-party notices',
        '',
        f'OriginLM `{version}` native helper for `{target}`.',
        '',
        'This file is generated from the resolved Cargo normal/build dependency graph of',
        '`originlm-cli` for the exact target above. It is not a claim that these',
        'components are vulnerability-free. Source commit:',
        f'`{source_commit}`.',
        '',
        'The helper binary, the repository `LICENSE`, every dependency license text',
        'under `licenses/`, this notice file and `sbom.cdx.json` are all covered by the',
        'archive member manifest.',
        '',
        '## Components',
        '',
        '| Component | Version | License | Source | License texts |',
        '| --- | --- | --- | --- | --- |',
    ]
    for component in collection['components']:
        source = component['source']
        source_text = source.get('url') or source.get('path') or ''
        source_text = source_text.replace('|', '\\|')
        files = '<br>'.join(f'`{item["path"]}`' for item in component['licenseTexts'])
        lines.append(
            f'| `{component["name"]}` | `{component["version"]}` | '
            f'`{component["license"]}` | {source_text} | {files} |'
        )
    lines += [
        '',
        '## Complete license texts',
        '',
        'Each text is reproduced verbatim from the packaging source. Identical files',
        'shared by several components are shown once and referenced by every path',
        'that contains them.',
        '',
    ]
    for item in collection['texts']:
        lines += [f'### {item["paths"][0]}', '']
        if len(item['paths']) > 1:
            lines += ['Also included as: ' + ', '.join(f'`{path}`' for path in item['paths'][1:]), '']
        fence = '~~~~'
        lines += [fence + 'text', item['text'].rstrip('\n'), fence, '']
    return '\n'.join(lines).rstrip('\n') + '\n'


def sbom(collection, source_commit):
    root = next(
        component for component in collection['components']
        if component['name'] == collection['package']
    )
    return {
        'bomFormat': 'CycloneDX',
        'specVersion': '1.5',
        'version': 1,
        'metadata': {
            'component': {
                'type': 'application',
                'name': root['name'],
                'version': root['version'],
                'purl': root['purl'],
                'licenses': [{'expression': root['license']}],
            },
            'tools': {
                'components': [
                    {
                        'type': 'application',
                        'name': 'cargo',
                        'version': collection['toolchain']['cargo'],
                    },
                    {
                        'type': 'application',
                        'name': 'rustc',
                        'version': collection['toolchain']['rustc'],
                    },
                ],
            },
            'properties': [
                {'name': 'originlm:target', 'value': collection['target']},
                {'name': 'originlm:sourceCommit', 'value': source_commit},
                {'name': 'originlm:licenseTextsComplete', 'value': 'true'},
            ],
        },
        'components': [
            {
                'type': 'library',
                'name': component['name'],
                'version': component['version'],
                'purl': component['purl'],
                'licenses': [{'expression': component['license']}],
                'externalReferences': [
                    {
                        'type': 'vcs' if component['source']['kind'] in ('workspace', 'git') else 'distribution',
                        'url': component['source'].get('url')
                        or component['source'].get('path')
                        or '',
                    }
                ],
                'properties': [
                    {
                        'name': 'originlm:licenseTexts',
                        'value': ','.join(item['path'] for item in component['licenseTexts']),
                    }
                ],
            }
            for component in collection['components']
        ],
        'dependencies': [],
    }


def write_assets(destination, collection, source_commit):
    destination = pathlib.Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    for component in collection['components']:
        if not component.get('licenseTexts'):
            raise LicenseClosureError(f'no license text for {component["purl"]}')
        for record in component['licenseTexts']:
            source = record.pop('_source', None)
            if source is None:
                raise LicenseClosureError(f'missing source path for {record["path"]}')
            data = pathlib.Path(source).read_bytes()
            if len(data) != record['bytes'] or hashlib.sha256(data).hexdigest() != record['sha256']:
                raise LicenseClosureError(f'license text changed while staging {record["path"]}')
            target = destination / record['path']
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
    (destination / 'THIRD_PARTY_NOTICES.md').write_text(
        notices(collection, source_commit), encoding='utf-8'
    )
    (destination / 'sbom.cdx.json').write_text(
        json.dumps(sbom(collection, source_commit), indent=2, sort_keys=True) + '\n',
        encoding='utf-8',
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument('--package', default='originlm-cli')
    parser.add_argument('--target', required=True)
    parser.add_argument('--source-commit', default=None)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    source_commit = args.source_commit
    if not source_commit:
        source_commit = _run(['git', 'rev-parse', 'HEAD'], root).strip()
    collection = collect(root, args.target, args.package)
    write_assets(args.out, collection, source_commit)
    print(json.dumps({
        'package': collection['package'],
        'version': collection['version'],
        'target': collection['target'],
        'components': len(collection['components']),
        'licenseTexts': sum(len(item['paths']) for item in collection['texts']),
        'sourceCommit': source_commit,
    }, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (LicenseClosureError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
