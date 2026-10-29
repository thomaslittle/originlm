import hashlib
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / '.github' / 'workflows'
sys.path.insert(0, str(ROOT / 'scripts'))


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PublisherWorkflows(unittest.TestCase):
    def test_pypi_uses_qualified_platform_set(self):
        workflow = (WORKFLOWS / 'publish-pypi.yml').read_text(encoding='utf-8')
        self.assertIn('verify_python_release_set.py', workflow)
        self.assertIn('--dist dist --reports reports', workflow)
        self.assertNotIn("*linux-gnu)", workflow)
        module = load('verify_python_release_set')
        self.assertEqual(set(module.EXPECTED), {
            'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu',
            'x86_64-apple-darwin', 'aarch64-apple-darwin',
            'x86_64-pc-windows-msvc',
        })
        self.assertEqual(
            {target: info['releaseWheelPlatform'] for target, info in module.EXPECTED.items()},
            {
                'x86_64-unknown-linux-gnu': 'manylinux_2_28_x86_64',
                'aarch64-unknown-linux-gnu': 'manylinux_2_28_aarch64',
                'x86_64-apple-darwin': 'macosx_15_0_x86_64',
                'aarch64-apple-darwin': 'macosx_15_0_arm64',
                'x86_64-pc-windows-msvc': 'win_amd64',
            },
        )

    def test_pypi_upload_directory_contains_only_wheels(self):
        workflow = (WORKFLOWS / 'publish-pypi.yml').read_text(encoding='utf-8')
        self.assertIn('verify_python_release_set.py', workflow)
        self.assertIn('packages-dir: dist', workflow)
        self.assertIn("find dist -type f ! -name '*.whl'", workflow)
        self.assertIn('name: python-wheel-verification-reports', workflow)
        script = (ROOT / 'scripts/verify_python_release_set.py').read_text(encoding='utf-8')
        self.assertIn("sums = reports / 'python-wheel-SHA256SUMS'", script)
        self.assertIn('shutil.copy2(wheel, dist / wheel.name)', script)

    def test_python_release_evidence_requires_native_and_manylinux_proofs(self):
        module = load('verify_python_release_set')
        version = '0.1.0-alpha.20'
        source = 'a' * 40
        wheel_sha = 'b' * 64
        consumer = {
            'status': 'pass', 'target': 'x86_64-pc-windows-msvc',
            'sourceCommit': source, 'version': version, 'wheelSha256': wheel_sha,
            'mode': 'child', 'connections': 0, 'models': 0, 'accounts': 0,
            'inferenceRequests': 0,
        }
        wheel = {
            'status': 'pass', 'target': 'x86_64-pc-windows-msvc',
            'sourceCommit': source, 'version': version, 'platform': 'win_amd64',
            'release': True, 'uploadGuard': 'removed', 'manylinux': False,
            'nativeBinary': {'format': 'pe', 'passed': True},
            'twine': {'passed': True},
        }
        result = module.validate_evidence(
            'x86_64-pc-windows-msvc', wheel_sha, wheel, consumer, version, source
        )
        self.assertEqual(result['kind'], 'windows')
        broken = dict(wheel)
        broken['nativeBinary'] = {'format': 'pe', 'passed': False}
        with self.assertRaises(module.PublishSetError):
            module.validate_evidence(
                'x86_64-pc-windows-msvc', wheel_sha, broken, consumer, version, source
            )

    def test_crate_preparation_fetches_before_offline_packaging(self):
        workflow = (WORKFLOWS / 'prepare-crates.yml').read_text(encoding='utf-8')
        self.assertIn('cargo fetch --locked', workflow)
        self.assertLess(workflow.index('cargo fetch --locked'), workflow.index('--package-dir crates-release'))

    def test_crate_publisher_fetches_and_revalidates_published_bytes(self):
        workflow = (WORKFLOWS / 'publish-crates.yml').read_text(encoding='utf-8')
        self.assertIn('cargo fetch --locked', workflow)
        self.assertLess(workflow.index('cargo fetch --locked'), workflow.index('publish_order.py'))
        self.assertIn('repacked crate payload hashes match', workflow)
        self.assertIn('verify_published_crates.py', workflow)
        self.assertLess(workflow.index('cargo publish --package'), workflow.index('verify_published_crates.py'))

    def test_cargo_publish_dry_run_payload_is_checked_before_upload(self):
        workflow = (WORKFLOWS / 'publish-crates.yml').read_text(encoding='utf-8')
        self.assertIn('cargo publish --dry-run', workflow)
        self.assertIn('verify_cargo_publish_payload.py', workflow)
        self.assertLess(workflow.index('cargo publish --dry-run'), workflow.index('cargo publish --package'))
        self.assertLess(workflow.index('verify_cargo_publish_payload.py'), workflow.index('cargo publish --package'))

    def test_cargo_publish_payload_verifier_matches_exact_bytes(self):
        module = load('verify_cargo_publish_payload')
        with tempfile.TemporaryDirectory() as directory:
            target = pathlib.Path(directory) / 'package' / 'tmp-crate'
            target.mkdir(parents=True)
            data = b'qualified-crate-bytes'
            (target / 'originlm-contracts-0.1.0-alpha.20.crate').write_bytes(data)
            expected = hashlib.sha256(data).hexdigest()
            evidence = module.verify(directory, 'originlm-contracts', '0.1.0-alpha.20', expected)
            self.assertEqual(evidence['status'], 'pass')
            with self.assertRaises(module.PayloadError):
                module.verify(directory, 'originlm-contracts', '0.1.0-alpha.20', '0' * 64)

    def test_npm_publisher_requires_five_consumer_artifacts(self):
        workflow = (WORKFLOWS / 'publish-npm.yml').read_text(encoding='utf-8')
        self.assertIn("--pattern 'npm-*'", workflow)
        self.assertIn('verify_npm_consumer_matrix.py', workflow)
        self.assertNotIn('registry-consumer.json', workflow)

    def test_npm_consumer_matrix_requires_every_platform_and_matching_sdk(self):
        module = load('verify_npm_consumer_matrix')
        version = '0.1.0-alpha.20'
        source = 'a' * 40
        sdk_sha = 'b' * 64
        optional = {name: version for name in module.LABELS.values()}
        with tempfile.TemporaryDirectory() as directory:
            artifacts = pathlib.Path(directory)
            release = artifacts / 'npm-release'
            release.mkdir()
            (release / 'manifest.json').write_text(json.dumps({
                'version': version, 'sourceCommit': source,
                'sdk': {'sha256': sdk_sha}, 'optionalDependencies': optional,
            }), encoding='utf-8')
            for label, package in module.LABELS.items():
                evidence_dir = artifacts / f'npm-consumer-{label}'
                evidence_dir.mkdir()
                (evidence_dir / f'npm-consumer-{label}.json').write_text(json.dumps({
                    'status': 'pass', 'install': 'single-universal-package',
                    'version': version, 'sourceCommit': source, 'sdkSha256': sdk_sha,
                    'resolvedPlatformPackage': package, 'optionalDependencies': optional,
                    'mode': 'child', 'connections': 0, 'models': 0, 'accounts': 0,
                    'inferenceRequests': 0,
                }), encoding='utf-8')
            evidence = module.verify(artifacts, version, source)
            self.assertEqual(len(evidence['platforms']), 5)
            (artifacts / 'npm-consumer-win32-x64' / 'npm-consumer-win32-x64.json').unlink()
            with self.assertRaises(module.ConsumerMatrixError):
                module.verify(artifacts, version, source)

    def test_npm_requires_bootstrap_before_oidc(self):
        workflow = (WORKFLOWS / 'publish-npm.yml').read_text(encoding='utf-8')
        self.assertIn('has not been bootstrapped', workflow)
        self.assertIn('originlm-cli-linux-x64', workflow)
        self.assertNotIn('@originlm/cli-', workflow)
        docs = (ROOT / 'docs/release/REGISTRY-PUBLISHING.md').read_text(encoding='utf-8')
        self.assertIn('Bootstrap the six packages first', docs)
        self.assertIn('OIDC cannot bootstrap', docs)

    def test_trusted_publisher_environment_and_workflow_names_are_stable(self):
        pypi = (WORKFLOWS / 'publish-pypi.yml').read_text(encoding='utf-8')
        npm = (WORKFLOWS / 'publish-npm.yml').read_text(encoding='utf-8')
        crates = (WORKFLOWS / 'publish-crates.yml').read_text(encoding='utf-8')
        self.assertIn('environment: release-pypi', pypi)
        self.assertIn('pypa/gh-action-pypi-publish@release/v1', pypi)
        self.assertIn('environment: release-npm', npm)
        self.assertIn('id-token: write', npm)
        self.assertIn('npm@11.5.1', npm)
        self.assertIn('environment: release-crates', crates)
        self.assertIn('rust-lang/crates-io-auth-action@v1', crates)


if __name__ == '__main__':
    unittest.main()
