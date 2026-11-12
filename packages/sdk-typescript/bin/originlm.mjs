#!/usr/bin/env node
// The `originlm` command of the npm package: run the native helper that npm
// installed as the matching `originlm-cli-<platform>` optional dependency,
// with every argument forwarded, stdio inherited and the exit status kept.
// The same pattern esbuild and Biome use: no install script, no download and
// no dependencies. `LAI_HELPER_PATH` overrides the packaged binary, as it does
// for the SDK.
import {spawnSync} from 'node:child_process';
import {existsSync, readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {dirname, isAbsolute, join} from 'node:path';

const PLATFORM_PACKAGES = {
  'linux-x64': 'originlm-cli-linux-x64',
  'linux-arm64': 'originlm-cli-linux-arm64',
  'darwin-arm64': 'originlm-cli-darwin-arm64',
  'darwin-x64': 'originlm-cli-darwin-x64',
  'win32-x64': 'originlm-cli-windows-x64',
};

function fail(message) {
  process.stderr.write(`originlm: ${message}\n`);
  process.exit(1);
}

function packagedBinary() {
  const target = `${process.platform}-${process.arch}`;
  const name = PLATFORM_PACKAGES[target];
  if (!name) {
    fail(
      `no OriginLM helper is published for ${target}. ` +
        `Supported: ${Object.keys(PLATFORM_PACKAGES).join(', ')}. ` +
        'Build it from source (cargo install originlm-cli) and set LAI_HELPER_PATH to it.',
    );
  }
  let manifestPath;
  try {
    manifestPath = createRequire(import.meta.url).resolve(`${name}/package.json`);
  } catch {
    fail(
      `the platform package ${name} is not installed. npm installs it as an optional ` +
        'dependency of originlm. Reinstall without --omit=optional / --no-optional ' +
        `(npm i -g originlm), or install it directly: npm i -g ${name}`,
    );
  }
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const relative = typeof manifest.bin === 'string' ? manifest.bin : manifest.bin?.originlm;
  if (typeof relative !== 'string' || !relative) fail(`${name} declares no originlm binary`);
  const binary = join(dirname(manifestPath), relative);
  if (!existsSync(binary)) fail(`${name} is installed but its binary is missing: ${binary}`);
  return binary;
}

const override = process.env.LAI_HELPER_PATH;
if (override !== undefined && (!isAbsolute(override) || !existsSync(override))) {
  fail('LAI_HELPER_PATH must be the absolute path of an existing helper binary');
}
const binary = override || packagedBinary();
const result = spawnSync(binary, process.argv.slice(2), {stdio: 'inherit', windowsHide: true});
if (result.error) fail(`could not start ${binary}: ${result.error.message}`);
if (result.signal) {
  // Re-raise so a parent sees the same termination, then fall back to 128+n.
  process.kill(process.pid, result.signal);
  process.exit(1);
}
process.exit(result.status ?? 1);
