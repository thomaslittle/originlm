#!/usr/bin/env node
/**
 * Copy the SDK package for publishing and add the platform packages built for
 * this release as optionalDependencies.
 *
 *   node scripts/npm-add-platform-deps.mjs \
 *     --packages npm-out --sdk packages/sdk-typescript --out npm-sdk
 *
 * The workspace package.json deliberately has no platform dependencies so
 * `npm ci` is identical on every machine; this staged copy is the published
 * artifact and resolves `@originlm/cli-<platform>` at install time.
 */
import {cpSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';

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
mkdirSync(out, {recursive: true});
cpSync(sdk, out, {
  recursive: true,
  filter: (path) => !path.includes('node_modules') && !path.endsWith('package-lock.json'),
});
const manifest = JSON.parse(readFileSync(join(out, 'package.json'), 'utf8'));
if (manifest.private && !process.argv.includes('--preview')) throw new Error('Refusing to stage a private package without --preview');
// Preview staging preserves private:true: npm pack is allowed, npm publish is not.

const optional = {};
for (const directory of readdirSync(packages)) {
  const path = join(packages, directory, 'package.json');
  if (!existsSync(path)) continue;
  const platform = JSON.parse(readFileSync(path, 'utf8'));
  if (typeof platform.name !== 'string' || !platform.name.startsWith('@originlm/cli-')) continue;
  optional[platform.name] = platform.version;
}
if (Object.keys(optional).length === 0) throw new Error('No platform packages to attach');
manifest.optionalDependencies = optional;
writeFileSync(join(out, 'package.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`staged ${out} with ${Object.keys(optional).length} optional platform packages`);
