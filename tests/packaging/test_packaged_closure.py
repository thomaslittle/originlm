import importlib.util
import io
import json
import pathlib
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


closure = load('verify_packaged_closure')


def crate(path, name, version, files):
    with tarfile.open(path, 'w:gz') as archive:
        for relative, data in files.items():
            member = tarfile.TarInfo(f'{name}-{version}/{relative}')
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))


class PackagedClosureTests(unittest.TestCase):
    def test_packaged_dependency_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            package = pathlib.Path(directory)
            (package / 'Cargo.toml').write_text(
                '[package]\nname = "fixture"\nversion = "1.0.0"\n'
                '[dependencies.other]\nversion = "1.0"\npath = "../other"\n',
                encoding='utf-8',
            )
            with self.assertRaises(closure.PackageClosureError):
                closure._check_source_path_leakage(package)

    def test_absolute_workspace_path_leakage_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            package = pathlib.Path(directory)
            (package / 'Cargo.toml').write_text('[package]\nname = "fixture"\nversion = "1.0.0"\n', encoding='utf-8')
            (package / 'README.md').write_text('built at /home/tomlit/source\n', encoding='utf-8')
            with self.assertRaises(closure.PackageClosureError):
                closure._check_source_path_leakage(package)

    def test_crate_archive_traversal_and_symlinks_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'bad.crate'
            with tarfile.open(path, 'w:gz') as archive:
                member = tarfile.TarInfo('../escape')
                member.size = 1
                archive.addfile(member, io.BytesIO(b'x'))
            with self.assertRaises(closure.PackageClosureError):
                with tarfile.open(path, 'r:gz') as archive:
                    closure._safe_members(archive, path, 'fixture', '1.0.0')
            with tarfile.open(path, 'w:gz') as archive:
                member = tarfile.TarInfo('fixture-1.0.0/LICENSE')
                member.type = tarfile.SYMTYPE
                member.linkname = '/etc/passwd'
                archive.addfile(member)
            with self.assertRaises(closure.PackageClosureError):
                with tarfile.open(path, 'r:gz') as archive:
                    closure._safe_members(archive, path, 'fixture', '1.0.0')

    def test_release_set_rejects_missing_or_tampered_crates(self):
        with tempfile.TemporaryDirectory() as directory:
            release = pathlib.Path(directory)
            crate_path = release / 'originlm-contracts-0.1.0-alpha.20.crate'
            crate(crate_path, 'originlm-contracts', '0.1.0-alpha.20', {
                'Cargo.toml': b'[package]\nname = "originlm-contracts"\nversion = "0.1.0-alpha.20"\n',
            })
            manifest = {
                'sourceCommit': 'a' * 40,
                'order': closure.EXPECTED_ORDER,
                'packages': [{
                    'name': 'originlm-contracts', 'version': '0.1.0-alpha.20',
                    'file': crate_path.name, 'bytes': crate_path.stat().st_size,
                    'sha256': closure._hash(crate_path),
                }],
            }
            (release / 'manifest.json').write_text(json.dumps(manifest), encoding='utf-8')
            with self.assertRaises(closure.PackageClosureError):
                closure.verify(release, '1.88.0')
            manifest['packages'] = [dict(manifest['packages'][0], sha256='0' * 64)]
            (release / 'manifest.json').write_text(json.dumps(manifest), encoding='utf-8')
            with self.assertRaises(closure.PackageClosureError):
                closure.verify(release, '1.88.0')


    def test_gate_resolves_a_fresh_online_lock_before_locked_offline_compiles(self):
        with tempfile.TemporaryDirectory() as directory:
            release = pathlib.Path(directory)
            packages = []
            for name in closure.EXPECTED_ORDER:
                version = '0.1.0-alpha.20'
                path = release / f'{name}-{version}.crate'
                crate(path, name, version, {
                    'Cargo.toml': f'[package]\nname = "{name}"\nversion = "{version}"\n'.encode(),
                })
                packages.append({
                    'name': name, 'version': version, 'file': path.name,
                    'bytes': path.stat().st_size, 'sha256': closure._hash(path),
                })
            manifest = {
                'sourceCommit': 'a' * 40,
                'order': closure.EXPECTED_ORDER,
                'packages': packages,
            }
            (release / 'manifest.json').write_text(json.dumps(manifest), encoding='utf-8')

            calls = []

            def fake_run(command, cwd, timeout=1800):
                calls.append((list(command), pathlib.Path(cwd)))
                lock = pathlib.Path(cwd) / 'Cargo.lock'
                if 'generate-lockfile' in command:
                    lock.write_text(
                        'version = 4\n\n[[package]]\nname = "yoke"\nversion = "0.8.4"\n'
                        'source = "registry+https://github.com/rust-lang/crates.io-index"\n',
                        encoding='utf-8',
                    )
                elif '--offline' in command:
                    # Cold-cache model: the only cached yoke-derive was the
                    # yanked 0.8.3. An offline check without the fresh 0.8.4
                    # lock must fail, exactly like the original release run.
                    if not lock.is_file() or '0.8.4' not in lock.read_text(encoding='utf-8'):
                        raise closure.PackageClosureError(
                            'offline check selected the yanked cached yoke-derive 0.8.3'
                        )
                return None

            with mock.patch.object(closure, '_run', side_effect=fake_run):
                evidence = closure.verify(release, '1.88.0')

            self.assertEqual(len(calls), 6)
            self.assertEqual(calls[0][0][2:], ['generate-lockfile'])
            self.assertEqual(calls[1][0][2:], ['fetch', '--locked'])
            self.assertEqual(
                calls[2][0][2:], ['check', '--workspace', '--locked', '--offline']
            )
            self.assertEqual(calls[3][0][2:], ['generate-lockfile'])
            self.assertEqual(calls[4][0][2:], ['fetch', '--locked'])
            self.assertEqual(calls[5][0][2:], ['check', '--locked', '--offline'])
            self.assertTrue(evidence['checks']['freshRegistryResolution'])
            self.assertTrue(evidence['checks']['workspaceLockedCheck'])
            self.assertTrue(evidence['checks']['consumerLockedCheck'])
            self.assertFalse(evidence['resolution']['sourceLockfileCopied'])
            resolved = {
                package['name']: package['version']
                for package in evidence['resolution']['workspace']['registryPackages']
            }
            self.assertEqual(resolved.get('yoke'), '0.8.4')


if __name__ == '__main__':
    unittest.main()
