// The `originlm` npm bin forwards to the installed platform helper.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {copyFileSync, linkSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

const launcher = new URL('../bin/originlm.mjs', import.meta.url);
const PLATFORMS = {'linux-x64':'linux-x64','linux-arm64':'linux-arm64','darwin-arm64':'darwin-arm64','darwin-x64':'darwin-x64','win32-x64':'windows-x64'};
const platform = PLATFORMS[`${process.platform}-${process.arch}`];
const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));

/** A global-install layout: <prefix>/node_modules/originlm with the platform package nested. */
function install({withPlatform}) {
  const root = mkdtempSync(join(tmpdir(), 'originlm-bin-'));
  const pkg = join(root, 'node_modules', 'originlm');
  mkdirSync(join(pkg, 'bin'), {recursive: true});
  copyFileSync(launcher, join(pkg, 'bin', 'originlm.mjs'));
  writeFileSync(join(pkg, 'package.json'), JSON.stringify({name: 'originlm', type: 'module'}));
  if (withPlatform) {
    const name = `originlm-cli-${platform}`;
    const dir = join(pkg, 'node_modules', name);
    const executable = process.platform === 'win32' ? 'originlm.exe' : 'originlm';
    mkdirSync(join(dir, 'bin'), {recursive: true});
    writeFileSync(join(dir, 'package.json'), JSON.stringify({name, bin: {originlm: `bin/${executable}`}}));
    // The fake helper is this Node binary, so the test can observe argv/stdio/exit.
    try { linkSync(process.execPath, join(dir, 'bin', executable)); }
    catch { copyFileSync(process.execPath, join(dir, 'bin', executable)); }
  }
  return {root, bin: join(pkg, 'bin', 'originlm.mjs')};
}

function run(bin, args, options = {}) {
  const env = {...process.env};
  delete env.LAI_HELPER_PATH;
  return spawnSync(process.execPath, [bin, ...args], {encoding: 'utf8', env: {...env, ...options.env}, input: options.input});
}

test('the package declares the originlm command and ships the launcher', () => {
  // Both names run the same launcher, so `olm usage` and `originlm usage` are identical.
  assert.deepEqual(manifest.bin, {originlm: 'bin/originlm.mjs', olm: 'bin/originlm.mjs'});
  assert.ok(manifest.files.includes('bin'));
  assert.equal(readFileSync(launcher, 'utf8').split('\n')[0], '#!/usr/bin/env node');
  assert.equal(manifest.dependencies, undefined, 'the launcher must stay dependency-free');
});

test('arguments, stdio and the exit status pass through to the platform binary', {skip: !platform}, () => {
  const {root, bin} = install({withPlatform: true});
  try {
    const code = 'console.log(JSON.stringify(process.argv.slice(1)));process.stdin.pipe(process.stdout);process.stdin.on("end",()=>process.exit(7))';
    const result = run(bin, ['-e', code, 'mcp', 'two words', '--flag'], {input: 'from-stdin'});
    assert.equal(result.status, 7, result.stderr);
    assert.equal(result.stdout, `${JSON.stringify(['mcp', 'two words', '--flag'])}\nfrom-stdin`);
  } finally { rmSync(root, {recursive: true, force: true}); }
});

test('a missing platform package is a clear error, not a crash', {skip: !platform}, () => {
  const {root, bin} = install({withPlatform: false});
  try {
    const result = run(bin, ['--version']);
    assert.equal(result.status, 1);
    assert.match(result.stderr, new RegExp(`originlm-cli-${platform} is not installed`));
    assert.match(result.stderr, /--omit=optional/);
    assert.doesNotMatch(result.stderr, /at .*\.mjs:\d+/, 'no stack trace');
  } finally { rmSync(root, {recursive: true, force: true}); }
});

test('LAI_HELPER_PATH overrides the packaged binary and must be absolute', () => {
  const {root, bin} = install({withPlatform: false});
  try {
    const ok = run(bin, ['-e', 'process.exit(3)'], {env: {LAI_HELPER_PATH: process.execPath}});
    assert.equal(ok.status, 3, ok.stderr);
    const bad = run(bin, ['--version'], {env: {LAI_HELPER_PATH: 'relative/originlm'}});
    assert.equal(bad.status, 1);
    assert.match(bad.stderr, /LAI_HELPER_PATH must be the absolute path/);
  } finally { rmSync(root, {recursive: true, force: true}); }
});
