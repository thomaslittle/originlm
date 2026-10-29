import hashlib
import importlib.util
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / '.github' / 'workflows'


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PublisherWorkflows(unittest.TestCase):
    def test_pypi_uses_qualified_manylinux_platform(self):
        workflow = (WORKFLOWS / 'publish-pypi.yml').read_text(encoding='utf-8')
        self.assertIn("target_info('$target')['manylinuxPlatform']", workflow)
        self.assertNotIn("['wheelPlatform']", workflow)
        self.assertIn("*linux-gnu)", workflow)

    def test_pypi_upload_directory_contains_only_wheels(self):
        workflow = (WORKFLOWS / 'publish-pypi.yml').read_text(encoding='utf-8')
        self.assertIn('mkdir -p dist reports', workflow)
        self.assertIn('--out "reports/python-wheel-$target.json"', workflow)
        self.assertIn('cp "$wheel" dist/', workflow)
        self.assertIn('packages-dir: dist', workflow)
        self.assertIn('name: python-wheel-verification-reports', workflow)
        self.assertIn('sha256sum dist/*.whl > python-wheel-SHA256SUMS', workflow)
        self.assertIn("find dist -type f ! -name '*.whl'", workflow)
        upload = workflow.split('Publish to PyPI with trusted publishing', 1)[1]
        self.assertNotIn('python-wheel-SHA256SUMS', upload.split('packages-dir:', 1)[0])

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
