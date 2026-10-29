import importlib.util
import pathlib
import sys
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
        self.assertEqual(files[0][0], shared)

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
