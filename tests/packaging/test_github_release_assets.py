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


checker = load('verify_github_release')
targets = load('release_targets')


def release(directory, version='9.9.9-alpha.1', commit='a' * 40):
    """A complete synthetic asset set: tiny archives, manifests and SHA256SUMS."""
    lines = []
    for target in targets.TARGETS:
        name = targets.archive_name(target)
        data = f'archive for {target}'.encode()
        (directory / name).write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        lines.append(f'{digest}  {name}')
        (directory / checker.manifest_name(target)).write_text(json.dumps({
            'archive': {'name': name, 'bytes': len(data), 'sha256': digest},
            'target': target, 'version': version,
            'source': {'commit': commit, 'workingTreeDirty': False},
        }), encoding='utf-8')
    (directory / checker.SUMS).write_text('\n'.join(lines) + '\n', encoding='utf-8')


class GithubReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.assets = pathlib.Path(self.temp.name)
        release(self.assets)
        self.target = next(iter(targets.TARGETS))

    def tearDown(self):
        self.temp.cleanup()

    def test_complete_set_passes(self):
        self.assertEqual(checker.verify(self.assets, '9.9.9-alpha.1'), [])
        self.assertEqual(checker.main(['--assets', str(self.assets)]), 0)

    def test_archives_alone_are_refused(self):
        (self.assets / checker.SUMS).unlink()
        for target in targets.TARGETS:
            (self.assets / checker.manifest_name(target)).unlink()
        problems = checker.verify(self.assets)
        self.assertIn('SHA256SUMS is missing', problems)
        self.assertEqual(sum('manifest.json is missing' in p for p in problems), len(targets.TARGETS))
        self.assertEqual(checker.main(['--assets', str(self.assets)]), 1)

    def test_changed_archive_is_refused_by_sums_and_manifest(self):
        name = targets.archive_name(self.target)
        (self.assets / name).write_bytes(b'different bytes')
        problems = checker.verify(self.assets)
        self.assertIn(f'SHA256SUMS disagrees with the bytes of {name}', problems)
        self.assertIn(
            f'{checker.manifest_name(self.target)} disagrees with the bytes of {name}', problems)

    def test_missing_archive_and_unlisted_archive_are_refused(self):
        name = targets.archive_name(self.target)
        sums = self.assets / checker.SUMS
        sums.write_text(''.join(
            line + '\n' for line in sums.read_text(encoding='utf-8').splitlines()
            if not line.endswith(name)), encoding='utf-8')
        self.assertIn(f'SHA256SUMS does not list {name}', checker.verify(self.assets))
        (self.assets / name).unlink()
        self.assertIn(f'{name} is missing', checker.verify(self.assets))

    def test_mixed_commits_and_wrong_version_are_refused(self):
        path = self.assets / checker.manifest_name(self.target)
        manifest = json.loads(path.read_text(encoding='utf-8'))
        manifest['source']['commit'] = 'b' * 40
        path.write_text(json.dumps(manifest), encoding='utf-8')
        problems = checker.verify(self.assets, '1.0.0')
        self.assertTrue(any('several source commits' in p for p in problems))
        self.assertTrue(any('expected 1.0.0' in p for p in problems))

    def test_published_releases_are_checked_by_a_read_only_workflow(self):
        workflow = (WORKFLOWS / 'verify-release-assets.yml').read_text(encoding='utf-8')
        self.assertIn('verify_github_release.py', workflow)
        self.assertIn('contents: read', workflow)
        self.assertNotIn('contents: write', workflow)


if __name__ == '__main__':
    unittest.main()
