import hashlib
import importlib.util
import json
import pathlib
import stat
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


stage_module = load('stage_python_package')
verify_module = load('verify_python_wheel')
consumer_module = load('python_consumer_smoke')

VERSION = '0.1.0a20'
NATIVE_VERSION = '0.1.0-alpha.20'
COMMIT = 'a' * 40
TARGET = 'x86_64-unknown-linux-gnu'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def elf_bytes(machine=62):
    data = bytearray(64)
    data[:4] = b'\x7fELF'
    data[4] = 2
    data[5] = 1
    data[6] = 1
    data[16:18] = (2).to_bytes(2, 'little')
    data[18:20] = machine.to_bytes(2, 'little')
    return bytes(data)


def add(archive, name, data, mode=0o644):
    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.create_system = 3
    info.external_attr = (stat.S_IFREG | mode) << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    archive.writestr(info, data)


def make_wheel(directory, *, platform='manylinux_2_28_x86_64', release=False,
               classifier=None, helper=elf_bytes(), helper_mode=0o755,
               helper_record=None, omit=None, target=TARGET):
    omit = set(omit or ())
    directory = pathlib.Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    helper_member = f'helper-{target}/originlm'
    sbom = {
        'bomFormat': 'CycloneDX', 'specVersion': '1.5', 'version': 1,
        'components': [{
            'type': 'library', 'name': 'fixture', 'version': '1.0.0',
            'licenses': [{'expression': 'MIT'}],
            'properties': [{'name': 'originlm:licenseTexts',
                            'value': 'licenses/fixture-1.0/LICENSE'}],
        }],
    }
    sbom_bytes = json.dumps(sbom).encode()
    members = {
        helper_member: helper_record or {'bytes': len(helper), 'sha256': sha(helper)},
        'LICENSE': {'bytes': 4, 'sha256': sha(b'MIT\n')},
        'THIRD_PARTY_NOTICES.md': {'bytes': 8, 'sha256': sha(b'notices\n')},
        'sbom.cdx.json': {'bytes': len(sbom_bytes), 'sha256': sha(sbom_bytes)},
        'licenses/fixture-1.0/LICENSE': {'bytes': 9, 'sha256': sha(b'MIT dep\n\n')},
    }
    native = {
        'schemaVersion': 1, 'kind': 'originlm-helper-archive', 'name': 'originlm',
        'version': NATIVE_VERSION, 'target': target, 'source': {'commit': COMMIT},
        'license': {'spdx': 'MIT', 'path': 'LICENSE', 'complete': True},
        'dependencies': {'count': 1, 'licenseTextsComplete': True},
        'members': members,
    }
    native_bytes = json.dumps(native, sort_keys=True).encode()
    provenance = {
        'schemaVersion': 1, 'kind': 'originlm-python-wheel', 'name': 'originlm',
        'version': VERSION, 'target': target, 'platform': platform,
        'release': release, 'publishable': release,
        'source': {'commit': COMMIT, 'workingTreeDirty': False},
        'native': {'manifestSha256': sha(native_bytes), 'version': NATIVE_VERSION,
                   'target': target},
        'dependencies': {'complete': True, 'count': 1},
    }
    if not release:
        classifier = stage_module.CLASSIFIER if classifier is None else classifier
    classifiers = '' if classifier is None else f'Classifier: {classifier}\n'
    metadata = (
        'Metadata-Version: 2.1\n'
        f'Name: originlm\nVersion: {VERSION}\n'
        'Requires-Python: >=3.11\n' + classifiers + '\n'
    )
    wheel_file = (
        'Wheel-Version: 1.0\nGenerator: fixture\nRoot-Is-Purelib: false\n'
        f'Tag: py3-none-{platform}\n'
    )
    path = directory / f'originlm-{VERSION}-py3-none-{platform}.whl'
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        files = {
            'originlm/LICENSE': b'MIT\n',
            'originlm/THIRD_PARTY_NOTICES.md': b'notices\n',
            'originlm/sbom.cdx.json': sbom_bytes,
            'originlm/manifest.json': native_bytes,
            'originlm/PROVENANCE.json': json.dumps(provenance).encode(),
            'originlm/licenses/fixture-1.0/LICENSE': b'MIT dep\n\n',
            'originlm/_bin/originlm': helper,
            f'originlm-{VERSION}.dist-info/METADATA': metadata.encode(),
            f'originlm-{VERSION}.dist-info/WHEEL': wheel_file.encode(),
            f'originlm-{VERSION}.dist-info/RECORD': b'',
        }
        for name, data in files.items():
            if name in omit:
                continue
            mode = helper_mode if name == 'originlm/_bin/originlm' else 0o644
            add(archive, name, data, mode)
    return path


class WheelVerifierTests(unittest.TestCase):
    def verify(self, wheel, **options):
        options.setdefault('run_tools', False)
        return verify_module.verify(wheel, VERSION, target=TARGET, **options)

    def test_preview_wheel_keeps_the_publication_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, platform='linux_x86_64')
            evidence = self.verify(wheel, plat_name='linux_x86_64', release=False)
            self.assertEqual(evidence['uploadGuard'], stage_module.CLASSIFIER)
            self.assertFalse(evidence['manylinux'])

    def test_preview_wheel_must_keep_the_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, platform='linux_x86_64', classifier='')
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=False)

    def test_release_linux_wheel_must_be_manylinux(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, platform='linux_x86_64', release=True)
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True)

    def test_release_manylinux_wheel_is_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True)
            evidence = self.verify(wheel, release=True,
                                   plat_name='manylinux_2_28_x86_64')
            self.assertTrue(evidence['manylinux'])
            self.assertEqual(evidence['licenseTexts'], 1)

    def test_release_wheel_rejects_the_publication_classifier(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True,
                               classifier=stage_module.CLASSIFIER)
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True)

    def test_missing_dependency_license_text_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True,
                               omit={'originlm/licenses/fixture-1.0/LICENSE'})
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True)

    def test_missing_provenance_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True,
                               omit={'originlm/PROVENANCE.json'})
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True)

    def test_native_helper_checksum_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(
                directory, release=True, helper=elf_bytes() + b'changed',
                helper_record={'bytes': len(elf_bytes()), 'sha256': sha(elf_bytes())},
            )
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True)

    def test_native_helper_must_be_executable_in_the_wheel(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True, helper_mode=0o644)
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True)

    def test_manylinux_tag_cannot_be_copied_from_a_linux_wheel(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True,
                               platform='manylinux_2_28_x86_64')
            with self.assertRaises(verify_module.VerifyError):
                self.verify(wheel, release=True, plat_name='linux_x86_64')

    def test_wrapped_auditwheel_tag_is_parsed(self):
        output = 'is consistent\nwith the following platform tag: "manylinux_2_28_x86_64".\n'
        self.assertEqual(verify_module.auditwheel_tag(output), 'manylinux_2_28_x86_64')

    def test_version_normalization_matches_cargo_and_pep440(self):
        self.assertEqual(verify_module.normalize_version(NATIVE_VERSION), VERSION)
        self.assertEqual(consumer_module.normalize_version(NATIVE_VERSION), VERSION)

    def test_project_semver_accepts_pep440_wheel_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = make_wheel(directory, release=True)
            evidence = verify_module.verify(
                wheel, NATIVE_VERSION, target=TARGET, release=True, run_tools=False
            )
            self.assertEqual(evidence['status'], 'pass')

    def test_auditwheel_baseline_must_cover_the_declared_manylinux(self):
        self.assertTrue(verify_module.manylinux_accepts(
            'manylinux_2_28_x86_64', 'manylinux_2_17_x86_64'))
        self.assertTrue(verify_module.manylinux_accepts(
            'manylinux_2_28_x86_64', 'manylinux_2_28_x86_64'))
        for observed in ('manylinux_2_39_x86_64', 'linux_x86_64',
                         'manylinux_2_28_aarch64'):
            with self.subTest(observed=observed):
                self.assertFalse(verify_module.manylinux_accepts(
                    'manylinux_2_28_x86_64', observed))


class ManylinuxGuardTests(unittest.TestCase):
    environment = dict(
        machine='x86_64', os_release_id='almalinux', glibc='2.28',
        image_marker=True,
    )

    def test_official_2_28_image_is_accepted(self):
        result = stage_module.check_manylinux_environment(
            'manylinux_2_28_x86_64', TARGET, **self.environment
        )
        self.assertEqual(result['baseline'], '2.28')

    def test_ubuntu_host_cannot_claim_manylinux(self):
        for change in (
            {'os_release_id': 'ubuntu'},
            {'glibc': '2.39'},
            {'machine': 'aarch64'},
            {'image_marker': False},
        ):
            with self.subTest(change=change), self.assertRaises(stage_module.StageError):
                stage_module.check_manylinux_environment(
                    'manylinux_2_28_x86_64', TARGET,
                    **{**self.environment, **change},
                )

    def test_architecture_must_match_the_target(self):
        with self.assertRaises(stage_module.StageError):
            stage_module.check_manylinux_environment(
                'manylinux_2_28_x86_64', 'aarch64-unknown-linux-gnu',
                **self.environment,
            )


class StagingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        (self.root / 'LICENSE').write_text('MIT\n', encoding='utf-8')
        self.binary = self.root / 'originlm'
        self.binary.write_bytes(b'#!/bin/sh\nexit 0\n')

    def tearDown(self):
        self.tmp.cleanup()

    def metadata(self, target=TARGET):
        directory = self.root / 'metadata'
        (directory / 'licenses' / 'fixture-1.0').mkdir(parents=True)
        (directory / 'LICENSE').write_text('MIT\n', encoding='utf-8')
        (directory / 'THIRD_PARTY_NOTICES.md').write_text('notices\n', encoding='utf-8')
        (directory / 'sbom.cdx.json').write_text('{"components":[]}', encoding='utf-8')
        (directory / 'licenses' / 'fixture-1.0' / 'LICENSE').write_text(
            'MIT dep\n', encoding='utf-8'
        )
        (directory / 'manifest.json').write_text(json.dumps({
            'schemaVersion': 1, 'kind': 'originlm-helper-archive',
            'name': 'originlm', 'version': NATIVE_VERSION, 'target': target,
            'license': {'spdx': 'MIT'},
            'dependencies': {'count': 1, 'licenseTextsComplete': True},
        }), encoding='utf-8')
        return directory

    def stage(self, **options):
        options.setdefault('plat_name', 'linux_x86_64')
        options.setdefault('source', ROOT / 'packages/sdk-python')
        return stage_module.stage(
            self.binary, TARGET, self.root / 'wheels', self.root, **options
        )

    def test_preview_staging_keeps_the_classifier_and_host_tag(self):
        result = self.stage()
        manifest = (self.root / 'wheels/originlm/pyproject.toml').read_text('utf-8')
        self.assertIn(stage_module.CLASSIFIER, manifest)
        self.assertFalse(result['release'])
        provenance = json.loads(
            (self.root / 'wheels/originlm/src/originlm/PROVENANCE.json').read_text()
        )
        self.assertFalse(provenance['release'])
        self.assertTrue(
            (self.root / 'wheels/originlm/src/originlm/_bin/originlm').is_file()
        )

    def test_release_staging_requires_and_copies_qualified_metadata(self):
        result = self.stage(release=True, metadata=self.metadata(),
                            source_commit=COMMIT, allow_dirty=True)
        staged = self.root / 'wheels/originlm'
        manifest = (staged / 'pyproject.toml').read_text('utf-8')
        self.assertNotIn(stage_module.CLASSIFIER, manifest)
        self.assertTrue(result['release'])
        for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'sbom.cdx.json',
                     'manifest.json', 'PROVENANCE.json',
                     'licenses/fixture-1.0/LICENSE'):
            self.assertTrue((staged / 'src/originlm' / name).is_file(), name)
        provenance = json.loads((staged / 'src/originlm/PROVENANCE.json').read_text())
        self.assertEqual(provenance['uploadGuard'], 'removed')
        self.assertTrue(provenance['dependencies']['complete'])
        self.assertEqual(provenance['source']['commit'], COMMIT)

    def test_release_staging_without_metadata_is_refused(self):
        with self.assertRaises(stage_module.StageError):
            self.stage(release=True, source_commit=COMMIT, allow_dirty=True)

    def test_preview_staging_does_not_require_metadata(self):
        result = self.stage()
        self.assertFalse(result['metadata'])

    @unittest.skipIf(
        stage_module._read_os_release() in stage_module.MANYLINUX_OS_IDS
        and pathlib.Path('/opt/python').is_dir(),
        'running inside an official manylinux image',
    )
    def test_manylinux_tag_is_refused_outside_the_image(self):
        with self.assertRaises(stage_module.StageError):
            self.stage(plat_name='manylinux_2_28_x86_64')


class ConsumerEnvironmentTests(unittest.TestCase):
    def test_clean_environment_has_no_override_or_ambient_tool_path(self):
        with tempfile.TemporaryDirectory() as directory:
            env = consumer_module._clean_env(directory)
        self.assertEqual(env['HOME'], directory)
        self.assertEqual(env['PATH'], str(pathlib.Path(directory) / 'empty-bin'))
        for name in ('LAI_HELPER_PATH', 'LAI_BROKER_ADDRESS', 'LAI_BROKER_TOKEN_FILE',
                     'XDG_RUNTIME_DIR', 'PYTHONPATH'):
            self.assertNotIn(name, env)

    def test_consumer_uses_auto_and_never_an_explicit_binary(self):
        source = consumer_module.CONSUMER
        self.assertIn('Client.auto()', source)
        self.assertNotIn('executable=', source)
        self.assertNotIn('LAI_HELPER_PATH', source.split('for name in', 1)[0])
        self.assertIn('inferenceRequests', source)


class WorkflowTests(unittest.TestCase):
    def test_wheel_workflow_uses_official_manylinux_images_and_no_publication(self):
        text = (ROOT / '.github/workflows/python-wheels.yml').read_text('utf-8')
        self.assertIn('quay.io/pypa/manylinux_2_28_x86_64', text)
        self.assertIn('quay.io/pypa/manylinux_2_28_aarch64', text)
        self.assertIn('--release', text)
        self.assertIn('auditwheel', text)
        self.assertIn('twine', text)
        self.assertIn('scripts/python_consumer_smoke.py', text)
        self.assertNotIn('twine upload', text)
        self.assertNotIn('pypa/gh-action-pypi-publish', text)
        self.assertIn('permissions:\n  contents: read', text)


if __name__ == '__main__':
    unittest.main()
