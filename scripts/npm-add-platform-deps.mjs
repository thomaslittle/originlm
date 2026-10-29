#!/usr/bin/env node
/**
 * Copy the SDK package for publishing and attach the platform helper packages.
 *
 *   node scripts/npm-add-platform-deps.mjs \
 *     --packages npm-out --sdk packages/sdk-typescript --out npm-sdk \
 *     --release --all-platforms
 *
 * The workspace package.json deliberately has no platform dependencies so
 * `npm ci` is identical on every machine. Release staging is the single
 * universal `originlm` tarball: it declares all five exact-version
 * `@originlm/cli-<platform>` packages as optionalDependencies. All release jobs
 * produce the same universal manifest; only the built platform package differs.
 */
import {cpSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';

const PLATFORM_PACKAGES = [
  '@originlm/cli-linux-x64',
  '@originlm/cli-linux-arm64',
  '@originlm/cli-darwin-arm64',
  '@originlm/cli-darwin-x64',
  '@originlm/cli-win32-x64',
];

function argument(name) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1 || !process.argv[index + 1]) {
    console.error(`Missing --${name}`);
    process.exit(2);
  }
  return process.argv[index + 1];
}

const packages = argument('packages');
const sdk = argument('sdk');
const out = argument('out');
const release = process.argv.includes('--release');
const preview = process.argv.includes('--preview');
const allPlatforms = process.argv.includes('--all-platforms');

mkdirSync(out, {recursive: true});
cpSync(sdk, out, {
  recursive: true,
  filter: (path) => !path.includes('node_modules') && !path.endsWith('package-lock.json'),
});
const manifest = JSON.parse(readFileSync(join(out, 'package.json'), 'utf8'));
if (manifest.private && !preview && !release) {
  throw new Error('Refusing to stage a private package without --preview or --release');
}

const optional = {};
for (const directory of readdirSync(packages).sort()) {
  const path = join(packages, directory, 'package.json');
  if (!existsSync(path)) continue;
  const platform = JSON.parse(readFileSync(path, 'utf8'));
  if (typeof platform.name !== 'string' || !platform.name.startsWith('@originlm/cli-')) continue;
  if (platform.version !== manifest.version) {
    throw new Error(`Platform package ${platform.name} version ${platform.version} does not match SDK ${manifest.version}`);
  }
  optional[platform.name] = platform.version;
}

if (allPlatforms) {
  for (const name of PLATFORM_PACKAGES) optional[name] = manifest.version;
  for (const [name, version] of Object.entries(optional)) {
    if (version !== manifest.version) throw new Error(`Optional dependency ${name} is not pinned to ${manifest.version}`);
  }
}
if (Object.keys(optional).length === 0) throw new Error('No platform packages to attach');
if (release && Object.keys(optional).length !== PLATFORM_PACKAGES.length) {
  throw new Error('Release staging must attach all five platform packages');
}
if (release) {
  delete manifest.private;
  manifest.optionalDependencies = Object.fromEntries(
    Object.entries(optional).sort(([left], [right]) => left.localeCompare(right)),
  );
} else {
  manifest.optionalDependencies = optional;
}
writeFileSync(join(out, 'package.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`staged ${out} with ${Object.keys(optional).length} optional platform packages`);
