import hashlib
import importlib.util
import io
import json
import os
import pathlib
import sys
import tarfile
import tempfile
import unittest
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


licenses = load('license_closure')
verify = load('verify_helper_archive')
smoke = load('helper_smoke')
npm_release = load('verify_npm_release')
npm_consumer = load('npm_consumer_smoke')
npm_pack = load('pack_npm_tarball')


class LicenseResolution(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nlicense = "MIT"\n', encoding='utf-8')
        (self.root / 'LICENSE').write_text('workspace MIT text\n', encoding='utf-8')

    def tearDown(self):
        self.tmp.cleanup()

    def package(self, directory, **metadata):
        directory.mkdir(parents=True, exist_ok=True)
        manifest = directory / 'Cargo.toml'
        manifest.write_text('[package]\nname = "fixture"\n', encoding='utf-8')
        return {'name': 'fixture', 'version': '1.0.0', 'manifest_path': str(manifest), 'source': None, **metadata}

    def test_workspace_member_inherits_only_the_matching_shared_license(self):
        directory = self.root / 'member'
        package = self.package(directory, license='MIT')
        files = licenses._license_files(package, self.root, 'MIT')
        self.assertEqual([path.name for path, _ in files], ['LICENSE'])
        package['license'] = 'Apache-2.0'
        with self.assertRaises(licenses.LicenseClosureError):
            licenses._license_files(package, self.root, 'MIT')

    def test_explicit_license_file_pointing_outside_the_package_is_honoured(self):
        shared = self.root / 'legal' / 'MIT.txt'
        shared.parent.mkdir()
        shared.write_text('shared text\n', encoding='utf-8')
        package = self.package(self.root / 'explicit', license='MIT', license_file='../legal/MIT.txt')
        files = licenses._license_files(package, self.root, 'MIT')
        self.assertTrue(os.path.samefile(files[0][0], shared))

    def test_nested_license_file_is_found_when_no_root_text_exists(self):
        directory = self.root / 'nested'
        package = self.package(directory, license='BSD-3-Clause')
        nested = directory / 'legal' / 'LICENSE'
        nested.parent.mkdir()
        nested.write_text('nested text\n', encoding='utf-8')
        files = licenses._license_files(package, self.root, 'MIT')
        self.assertEqual(files[0][0], nested)

    def test_missing_text_fails_instead_of_guessing(self):
        package = self.package(self.root / 'missing', license='Apache-2.0')
        with self.assertRaises(licenses.LicenseClosureError):
            licenses._license_files(package, self.root, 'MIT')

    def test_license_member_paths_reject_traversal_and_unsafe_segments(self):
        self.assertEqual(licenses._safe_member_path('licenses/a-1', pathlib.Path('LICENSE')), 'licenses/a-1/LICENSE')
        for relative in ('../LICENSE', '/LICENSE', 'legal/../../LICENSE', 'legal/bad name'):
            with self.subTest(relative=relative), self.assertRaises(licenses.LicenseClosureError):
                licenses._safe_member_path('licenses/a-1', pathlib.Path(relative))


def npm_tarball(path, manifest, files):
    with tarfile.open(path, 'w:gz') as archive:
        def add(name, data):
            info = tarfile.TarInfo(name)
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))
        add('package/package.json', json.dumps(manifest).encode('utf-8'))
        for name, data in files.items():
            add(f'package/{name}', data)


class NpmReleaseShape(unittest.TestCase):
    version = '0.1.0-alpha.20'

    def build(self, directory, sdk_optional=None):
        directory = pathlib.Path(directory)
        directory.mkdir(parents=True, exist_ok=True)
        optional = sdk_optional or {name: self.version for name in npm_release.PLATFORM_PACKAGES}
        npm_tarball(directory / 'originlm-0.1.0-alpha.20.tgz', {
            'name': 'originlm', 'version': self.version, 'optionalDependencies': optional,
        }, {
            'dist/index.js': b'export {};',
            'node/auto.mjs': b'export {};',
            'LICENSE': b'MIT\n',
            'THIRD_PARTY_NOTICES.md': b'notices\n',
        })
        helper = b'\x7fELFfixture-binary'
        license_text = b'MIT\n'
        notices = b'notices\n'
        sbom = b'{"components":[]}'
        dependency = b'MIT\n'
        target = 'x86_64-unknown-linux-gnu'
        members = {
            f'helper-{target}/originlm': {'bytes': len(helper), 'sha256': hashlib.sha256(helper).hexdigest()},
            'LICENSE': {'bytes': len(license_text), 'sha256': hashlib.sha256(license_text).hexdigest()},
            'THIRD_PARTY_NOTICES.md': {'bytes': len(notices), 'sha256': hashlib.sha256(notices).hexdigest()},
            'sbom.cdx.json': {'bytes': len(sbom), 'sha256': hashlib.sha256(sbom).hexdigest()},
            'licenses/x-1.0.0/LICENSE': {'bytes': len(dependency), 'sha256': hashlib.sha256(dependency).hexdigest()},
        }
        npm_tarball(directory / 'originlm-cli-linux-x64-0.1.0-alpha.20.tgz', {
            'name': 'originlm-cli-linux-x64', 'version': self.version,
            'os': ['linux'], 'cpu': ['x64'], 'bin': {'originlm': 'bin/originlm'},
        }, {
            'bin/originlm': helper,
            'manifest.json': json.dumps({
                'version': self.version, 'target': target,
                'source': {'commit': 'a' * 40}, 'members': members,
            }).encode('utf-8'),
            'sbom.cdx.json': sbom,
            'THIRD_PARTY_NOTICES.md': notices,
            'LICENSE': license_text,
            'licenses/x-1.0.0/LICENSE': dependency,
        })
        return directory

    def test_universal_sdk_and_platform_metadata_are_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = npm_release.verify(self.build(directory), self.version, 'a' * 40)
            self.assertEqual(evidence['status'], 'pass')
            self.assertEqual(evidence['target'], 'x86_64-unknown-linux-gnu')
            self.assertEqual(evidence['targets'], ['x86_64-unknown-linux-gnu'])
            self.assertEqual(len(evidence['optionalDependencies']), 5)

    def test_a_partial_platform_set_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            partial = {'originlm-cli-linux-x64': self.version}
            with self.assertRaises(npm_release.VerifyNpmError):
                npm_release.verify(self.build(directory, partial), self.version)

    def test_consumer_environment_has_no_ambient_override_or_provider_path(self):
        with tempfile.TemporaryDirectory() as directory:
            env = npm_consumer._clean_env(directory)
            self.assertEqual(env['HOME'], directory)
            self.assertEqual(env['PATH'], str(pathlib.Path(directory) / 'empty-bin'))
            for name in ('LAI_HELPER_PATH', 'LAI_BROKER_ADDRESS', 'LAI_BROKER_TOKEN_FILE'):
                self.assertNotIn(name, env)


class NpmPackaging(unittest.TestCase):
    def test_universal_sdk_pack_is_byte_identical(self):
        with tempfile.TemporaryDirectory() as directory:
            package = pathlib.Path(directory) / 'package'
            (package / 'dist').mkdir(parents=True)
            (package / 'package.json').write_text(json.dumps({
                'name': 'originlm', 'version': '0.1.0-alpha.20',
                'files': ['dist', 'LICENSE'],
            }), encoding='utf-8')
            (package / 'dist' / 'index.js').write_text('export {};\n', encoding='utf-8')
            (package / 'LICENSE').write_text('MIT\n', encoding='utf-8')
            (package / 'src').mkdir()
            (package / 'src' / 'secret.ts').write_text('secret\n', encoding='utf-8')
            (package / 'tests').mkdir()
            (package / 'tests' / 'x.test.mjs').write_text('test\n', encoding='utf-8')
            (package / 'tsconfig.json').write_text('{}\n', encoding='utf-8')
            first = npm_pack.pack(package, pathlib.Path(directory) / 'one', require=['dist/index.js', 'LICENSE'])
            second = npm_pack.pack(package, pathlib.Path(directory) / 'two', require=['dist/index.js', 'LICENSE'])
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with tarfile.open(first, 'r:gz') as archive:
                self.assertEqual(sorted(archive.getnames()), ['package/LICENSE', 'package/dist/index.js', 'package/package.json'])
                self.assertTrue(all(member.mtime == 0 for member in archive.getmembers()))
            with self.assertRaises(npm_pack.PackError):
                npm_pack.pack(package, pathlib.Path(directory) / 'three', require=['src/secret.ts'])


class ArchiveValidation(unittest.TestCase):
    def test_member_names_are_relative_and_never_backslash_paths(self):
        self.assertEqual(verify._safe_name('helper-x/originlm'), 'helper-x/originlm')
        for name in ('../escape', '/absolute', 'C:/secret', 'bin\\helper', 'bin//helper', ''):
            with self.subTest(name=name), self.assertRaises(verify.VerifyError):
                verify._safe_name(name)

    def test_zip_symlink_members_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'bad.zip'
            with zipfile.ZipFile(path, 'w') as archive:
                info = zipfile.ZipInfo('helper-x/originlm')
                info.external_attr = (0o120777 << 16)
                archive.writestr(info, b'not-a-real-binary')
            with self.assertRaises(verify.VerifyError):
                verify._read_archive(path)


class StdioParsing(unittest.TestCase):
    def test_only_identified_json_lines_are_accepted(self):
        text = '{"id":1,"result":{"protocolVersion":"1.0"}}\nnot json\n{"method":"event"}\n{"id":2,"error":{}}\n'
        messages = smoke._stdio_messages(text)
        self.assertEqual(set(messages), {1, 2})
        self.assertEqual(messages[1]['result']['protocolVersion'], '1.0')


class WorkflowEvidence(unittest.TestCase):
    def test_smoke_evidence_is_uploaded_from_a_non_hidden_directory(self):
        workflow = (ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        self.assertNotIn('.verification/helper', workflow)
        self.assertIn('prepared/evidence/helper-archive-', workflow)
        self.assertIn('--out "$RUNNER_TEMP/helper-binary-', workflow)
        self.assertIn('shutil.copy2(source, target / source.name)', workflow)
        self.assertIn('path: prepared/', workflow)
        self.assertIn('Assert persisted evidence', workflow)


if __name__ == '__main__':
    unittest.main()
