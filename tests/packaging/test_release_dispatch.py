import contextlib
import importlib.util
import io
import json
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / '.github' / 'workflows'
spec = importlib.util.spec_from_file_location('release_dispatch', ROOT / 'scripts' / 'release_dispatch.py')
release_dispatch = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = release_dispatch
spec.loader.exec_module(release_dispatch)

VERSION = '0.1.0-alpha.29'
SHA = 'b' * 40
RUNS = {'release.yml': 101, 'python-wheels.yml': 102, 'prepare-crates.yml': 103}


def workflow_inputs(name):
    """Input names and required flags of a workflow_dispatch form."""
    text = (WORKFLOWS / name).read_text(encoding='utf-8')
    block = text.split('    inputs:\n', 1)[1].split('\npermissions:', 1)[0]
    inputs = {}
    for key, body in re.findall(r'^      (\w+):\n((?:        .*\n?)*)', block, re.M):
        inputs[key] = 'required: true' in body
    return inputs


class FakeGh:
    def __init__(self):
        self.dispatched = []

    def __call__(self, argv):
        if argv[:3] == ['gh', 'workflow', 'run']:
            values = [argv[i + 1] for i, arg in enumerate(argv) if arg == '-f']
            fields = dict(value.split('=', 1) for value in values)
            self.dispatched.append((argv[3], fields))
            return ''
        path = argv[2].split('/', 3)[3]
        if path.startswith('contents/Cargo.toml'):
            return f'[workspace.package]\nversion = "{VERSION}"\n'
        if path.startswith('releases/tags/'):
            raise release_dispatch.subprocess.CalledProcessError(1, argv)
        if path.startswith('actions/workflows/'):
            workflow = path.split('/')[2]
            return json.dumps({'workflow_runs': [{'id': RUNS[workflow], 'head_sha': SHA}]})
        if path.startswith('actions/runs/'):
            return json.dumps({'total_count': 4})
        raise AssertionError(path)


class FakeLive:
    def __init__(self, npm='0.1.0-alpha.28', latest='0.1.0-alpha.20', crate=VERSION,
                 pypi='0.1.0a28'):
        self.npm, self.latest, self.crate, self.pypi = npm, latest, crate, pypi

    def npm_dist_tags(self, name):
        return {'alpha': self.npm, 'latest': self.latest}

    def crate_version(self, name):
        return self.crate

    def pypi_version(self):
        return self.pypi

    def site(self):
        return '0.1.0-alpha.28 0.1.0a28'


def run_main(argv, live):
    gh = FakeGh()
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = release_dispatch.main(argv, release_dispatch.Github(runner=gh), live)
    return code, gh.dispatched, out.getvalue()


class ReleaseDispatch(unittest.TestCase):
    def test_pep440(self):
        self.assertEqual(release_dispatch.pep440('0.1.0-alpha.29'), '0.1.0a29')
        self.assertEqual(release_dispatch.pep440('1.2.3-rc.1'), '1.2.3rc1')
        self.assertEqual(release_dispatch.pep440('1.2.3'), '1.2.3')

    def test_dispatches_only_missing_channels_as_dry_runs(self):
        code, dispatched, out = run_main(['dispatch'], FakeLive())
        self.assertEqual(code, 0)
        self.assertEqual([workflow for workflow, _ in dispatched],
                         ['publish-npm.yml', 'publish-npm.yml', 'publish-pypi.yml'])
        self.assertIn('skip     crates', out)
        npm, latest, pypi = (fields for _, fields in dispatched)
        self.assertEqual(npm, {'artifact_run_id': '101', 'dist_tag': 'alpha',
                               'source_commit': SHA, 'dry_run': 'true', 'confirm': ''})
        self.assertEqual(latest['tag_only'], 'true')
        self.assertEqual(latest['tag_version'], VERSION)
        self.assertEqual(latest['dist_tag'], 'latest')
        self.assertEqual(latest['dry_run'], 'true')
        self.assertEqual(pypi['artifact_run_id'], '102')

    def test_publish_sets_confirm_and_force_includes_live_channels(self):
        _, dispatched, _ = run_main(['dispatch', '--publish', '--force'], FakeLive())
        self.assertEqual(len(dispatched), 4)
        for _, fields in dispatched:
            self.assertEqual((fields['dry_run'], fields['confirm']), ('false', 'PUBLISH'))
        crates = dispatched[3][1]
        self.assertEqual((crates['artifact_run_id'], crates['auth_mode']), ('103', 'token'))

    def test_plan_dispatches_nothing(self):
        _, dispatched, out = run_main(['plan'], FakeLive())
        self.assertEqual(dispatched, [])
        self.assertIn('would    publish-pypi.yml', out)

    def test_verify_fails_until_every_channel_is_live(self):
        code, _, out = run_main(['verify'], FakeLive())
        self.assertEqual(code, 1)
        self.assertIn('MISSING  npm latest', out)
        self.assertIn('MISSING  GitHub release  missing', out)

    def test_dispatch_fields_match_the_real_workflow_forms(self):
        _, dispatched, _ = run_main(['dispatch', '--force'], FakeLive())
        for workflow, fields in dispatched:
            inputs = workflow_inputs(workflow)
            self.assertLessEqual(set(fields), set(inputs), workflow)
            required = {name for name, needed in inputs.items() if needed}
            self.assertLessEqual(required, set(fields), workflow)


if __name__ == '__main__':
    unittest.main()
