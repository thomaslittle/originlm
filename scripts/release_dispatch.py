#!/usr/bin/env python3
"""Fire every registry publisher for one release, then check what is live.

A release reaches four channels: npm (``alpha`` and ``latest`` dist-tags),
crates.io, PyPI and the GitHub prerelease. The registry publishers are manual
``workflow_dispatch`` workflows, so a channel is missed whenever someone forgets
to dispatch one. This script dispatches all of them for one integrated ``main``
commit, using the successful preparation runs at exactly that commit.

    python scripts/release_dispatch.py plan              # resolve inputs, dispatch nothing
    python scripts/release_dispatch.py dispatch          # dispatch every publisher as a dry run
    python scripts/release_dispatch.py dispatch --publish  # real publish (confirm=PUBLISH)
    python scripts/release_dispatch.py verify            # compare live registries to the version

Each dispatched run still stops at its protected ``release-*`` environment until
a maintainer approves it. Channels already live at the version are skipped.
The release checklist is docs/release/RELEASE-CHECKLIST.md.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = 'thomaslittle/originlm'
SITE = 'https://originlm.pages.dev/'
USER_AGENT = 'originlm-release-dispatch'
NPM_PACKAGES = ('originlm', 'originlm-cli-linux-x64', 'originlm-cli-linux-arm64',
                'originlm-cli-windows-x64')
# The facade and its contract/CLI ends of the 14-crate closure; publish-crates
# verifies every byte of the full set itself.
CRATES = ('originlm', 'originlm-contracts', 'originlm-cli')

# Preparation workflow that produces each publisher's artifacts.
PREPARATION = {
    'npm': 'release.yml',
    'pypi': 'python-wheels.yml',
    'crates': 'prepare-crates.yml',
}


def pep440(version: str) -> str:
    """``0.1.0-alpha.29`` -> ``0.1.0a29`` (the PyPI spelling)."""
    match = re.fullmatch(r'(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.(\d+))?', version)
    if not match:
        raise SystemExit(f'unsupported version {version!r}')
    base, stage, number = match.groups()
    if not stage:
        return base
    return base + {'alpha': 'a', 'beta': 'b', 'rc': 'rc'}[stage] + number


def run(argv: list[str]) -> str:
    return subprocess.run(argv, check=True, capture_output=True, text=True).stdout


def fetch_json(url: str):
    request = urllib.request.Request(url, headers={'User-Agent': USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def fetch_text(url: str) -> str:
    request = urllib.request.Request(url, headers={'User-Agent': USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read().decode('utf-8', 'replace')


@dataclass
class Github:
    """The few ``gh`` calls this script makes; replaced by a fake in tests."""

    repo: str = REPO
    runner: callable = run

    def read(self, argv: list[str]) -> str:
        # Reads retry: the API is occasionally slow to handshake. Dispatches never
        # retry, so a slow reply cannot fire a publisher twice.
        for attempt in range(3):
            try:
                return self.runner(argv)
            except subprocess.CalledProcessError:
                if attempt == 2:
                    raise
        raise AssertionError('unreachable')

    def api(self, path: str):
        return json.loads(self.read(['gh', 'api', f'repos/{self.repo}/{path}']))

    def release_sha(self) -> str:
        """The newest main commit whose release.yml run produced artifacts.

        release.yml runs on main only when a version file changes, so this is
        the `chore(release): prepare` commit, not necessarily main's HEAD.
        """
        runs = self.api('actions/workflows/release.yml/runs?branch=main&event=push'
                        '&status=success&per_page=20')['workflow_runs']
        for candidate in runs:
            if self.api(f'actions/runs/{candidate["id"]}/artifacts?per_page=1')['total_count']:
                return candidate['head_sha']
        raise SystemExit('no successful release.yml run with artifacts on main')

    def file_at(self, sha: str, path: str) -> str:
        return self.read(['gh', 'api', f'repos/{self.repo}/contents/{path}?ref={sha}',
                            '-H', 'Accept: application/vnd.github.raw'])

    def successful_run(self, workflow: str, sha: str) -> int | None:
        runs = self.api(f'actions/workflows/{workflow}/runs?head_sha={sha}&branch=main'
                        '&status=success&per_page=20')['workflow_runs']
        # A path-filtered push can also report success with every job skipped;
        # the publishers need the run that actually produced artifacts.
        for candidate in runs:
            artifacts = self.api(f'actions/runs/{candidate["id"]}/artifacts?per_page=1')
            if artifacts['total_count']:
                return candidate['id']
        return None

    def dispatch(self, workflow: str, fields: dict[str, str]) -> None:
        argv = ['gh', 'workflow', 'run', workflow, '--repo', self.repo, '--ref', 'main']
        for key, value in fields.items():
            argv += ['-f', f'{key}={value}']
        self.runner(argv)

    def release(self, tag: str):
        try:
            return self.api(f'releases/tags/{tag}')
        except subprocess.CalledProcessError:
            return None


@dataclass
class Live:
    """Public registry reads; replaced by a fake in tests."""

    def npm_dist_tags(self, name: str) -> dict:
        return fetch_json(f'https://registry.npmjs.org/-/package/{name}/dist-tags') or {}

    def crate_version(self, name: str) -> str | None:
        data = fetch_json(f'https://crates.io/api/v1/crates/{name}')
        return data['crate']['max_version'] if data else None

    def pypi_version(self) -> str | None:
        data = fetch_json('https://pypi.org/pypi/originlm/json')
        return data['info']['version'] if data else None

    def site(self) -> str:
        return fetch_text(SITE)


@dataclass
class Status:
    channel: str
    live: bool
    detail: str


@dataclass
class Plan:
    version: str
    sha: str
    runs: dict[str, int | None]
    dispatches: list[tuple[str, dict[str, str]]] = field(default_factory=list)
    skipped: list[str] = field(default_factory=list)


def workspace_version(cargo_toml: str) -> str:
    return tomllib.loads(cargo_toml)['workspace']['package']['version']


def check(version: str, live: Live, github: Github) -> list[Status]:
    statuses = []
    tags = live.npm_dist_tags('originlm')
    statuses.append(Status('npm alpha', tags.get('alpha') == version,
                           f"originlm alpha={tags.get('alpha')}"))
    statuses.append(Status('npm latest', tags.get('latest') == version,
                           f"originlm latest={tags.get('latest')}"))
    platforms = {name: live.npm_dist_tags(name).get('alpha') for name in NPM_PACKAGES[1:]}
    statuses.append(Status('npm helpers', all(v == version for v in platforms.values()),
                           ' '.join(f'{k.removeprefix("originlm-cli-")}={v}'
                                    for k, v in platforms.items())))
    crates = {name: live.crate_version(name) for name in CRATES}
    statuses.append(Status('crates.io', all(v == version for v in crates.values()),
                           ' '.join(f'{k}={v}' for k, v in crates.items())))
    pypi = live.pypi_version()
    statuses.append(Status('PyPI', pypi == pep440(version), f'originlm={pypi}'))
    release = github.release(f'v{version}')
    published = bool(release) and not release.get('draft')
    statuses.append(Status('GitHub release', published,
                           'missing' if not release else
                           f"{'draft' if release.get('draft') else 'published'}, "
                           f"{len(release.get('assets', []))} assets"))
    site = live.site()
    statuses.append(Status('site', version in site and pep440(version) in site,
                           SITE + (' shows ' if version in site else ' lacks ') + version))
    return statuses


def plan(version: str, sha: str, statuses: list[Status], github: Github, publish: bool,
         force: bool) -> Plan:
    live = {status.channel: status.live for status in statuses}
    runs = {channel: github.successful_run(workflow, sha)
            for channel, workflow in PREPARATION.items()}
    result = Plan(version, sha, runs)
    common = {'source_commit': sha, 'dry_run': 'false' if publish else 'true',
              'confirm': 'PUBLISH' if publish else ''}

    def add(channel: str, already_live: bool, workflow: str, fields: dict[str, str]):
        if already_live and not force:
            result.skipped.append(channel)
            return
        result.dispatches.append((workflow, fields))

    npm_live = live['npm alpha'] and live['npm helpers']
    if runs['npm'] is None and not (npm_live and not force):
        raise SystemExit(f'no successful release.yml run with artifacts at {sha}')
    add('npm', npm_live, 'publish-npm.yml',
        {'artifact_run_id': str(runs['npm']), 'dist_tag': 'alpha', **common})
    # tag_only ignores the artifact inputs, but the form requires them. Its
    # promote job demands confirm=PUBLISH even for a dry run, which then only
    # prints the dist-tags.
    add('npm latest', live['npm latest'], 'publish-npm.yml',
        {'tag_only': 'true', 'tag_version': version, 'dist_tag': 'latest',
         'artifact_run_id': str(runs['npm']), **common, 'confirm': 'PUBLISH'})
    for channel, workflow, extra in (
        ('pypi', 'publish-pypi.yml', {}),
        ('crates', 'publish-crates.yml', {'auth_mode': 'token'}),
    ):
        already = live['PyPI' if channel == 'pypi' else 'crates.io']
        if runs[channel] is None and not (already and not force):
            raise SystemExit(f'no successful {PREPARATION[channel]} run with artifacts at {sha}')
        add(channel, already, workflow,
            {'artifact_run_id': str(runs[channel]), **extra, **common})
    return result


def print_statuses(statuses: list[Status]) -> None:
    for status in statuses:
        print(f"  {'live' if status.live else 'MISSING':8} {status.channel:15} {status.detail}")


def main(argv: list[str] | None = None, github: Github | None = None,
         live: Live | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('command', choices=('plan', 'dispatch', 'verify'))
    parser.add_argument('--source-commit',
                        help='integrated main SHA (default: newest prepared release commit)')
    parser.add_argument('--publish', action='store_true',
                        help='real publish: dry_run=false confirm=PUBLISH')
    parser.add_argument('--force', action='store_true',
                        help='dispatch channels that are already live')
    args = parser.parse_args(argv)
    github = github or Github()
    live = live or Live()

    sha = args.source_commit or github.release_sha()
    version = workspace_version(github.file_at(sha, 'Cargo.toml'))
    print(f'release {version} (PyPI {pep440(version)}) from main commit {sha}')
    statuses = check(version, live, github)
    print_statuses(statuses)
    if args.command == 'verify':
        return 0 if all(status.live for status in statuses) else 1

    result = plan(version, sha, statuses, github, args.publish, args.force)
    print('preparation runs: ' + ', '.join(f'{k}={v}' for k, v in result.runs.items()))
    for channel in result.skipped:
        print(f'  skip     {channel} (already live)')
    mode = 'PUBLISH' if args.publish else 'dry run'
    for workflow, fields in result.dispatches:
        line = ' '.join(f'{k}={v}' for k, v in fields.items() if v)
        print(f'  {"dispatch" if args.command == "dispatch" else "would":8} {workflow} ({mode}) {line}')
        if args.command == 'dispatch':
            github.dispatch(workflow, fields)
    if args.command == 'dispatch' and result.dispatches:
        print('Approve each run at its release-* environment, then rerun `verify`.')
    if not all(s.live for s in statuses if s.channel in ('GitHub release', 'site')):
        print('GitHub release and site are not workflows: follow RELEASE-CHECKLIST.md.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
