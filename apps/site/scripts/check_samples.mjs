/** Type-check the TypeScript samples shown on the page against the SDK sources.
 *
 *   node scripts/check_samples.mjs          # from apps/site
 *
 * Extracts the quickstart chunks (src/data/story.ts) and the SDK tab's summarize.ts
 * (src/sections/Sdk.tsx) verbatim, adds a typed prelude for the names the fragments
 * assume, and runs `tsc --strict` with `originlm` mapped to packages/sdk-typescript/src.
 * A negative control (a misspelt method) must fail, or the check itself is broken.
 * Python and Rust samples are checked as described in docs/design/landing-audit.md.
 */
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const site = fileURLToPath(new URL('..', import.meta.url));
const dir = fileURLToPath(new URL('../.samples-check/', import.meta.url));
const sdk = fileURLToPath(new URL('../../../packages/sdk-typescript/src/index.ts', import.meta.url));
await rm(dir, { recursive: true, force: true });
await mkdir(dir, { recursive: true });

/** Template literals from an object literal `NAME = { key: `...` }` or a `code: `...`` field. */
const literals = (source, pattern) => [...source.matchAll(pattern)].map((m) => m[1].replace(/\\`/g, '`').replace(/\\\$/g, '$'));

const story = await readFile(`${site}src/data/story.ts`, 'utf8');
const codeBlock = story.slice(story.indexOf('export const CODE'), story.indexOf('export const REPLY'));
const chunks = literals(codeBlock, /\w+: `([\s\S]*?)`,/g);
if (chunks.length !== 4) throw new Error(`expected 4 quickstart chunks, found ${chunks.length}`);
const sdkTab = await readFile(`${site}src/sections/Sdk.tsx`, 'utf8');
const summarize = literals(sdkTab, /id: "ts",[\s\S]*?code: `([\s\S]*?)`,/g)[0];
if (!summarize) throw new Error('summarize.ts sample not found');

const prelude = `import { createLocalAI } from "originlm";\nconst ai = await createLocalAI();\ndeclare const requestId: string;\ndeclare const prompt: string;\n`;
const files = {
  'quickstart.ts': `${prelude}${chunks.join('\n')}\nexport {};\n`,
  'summarize.ts': `${summarize}\nexport {};\n`,
  'negative.ts': `${prelude}await ai.snapshot.gett();\nexport {};\n`,
};
for (const [name, text] of Object.entries(files)) await writeFile(`${dir}${name}`, text);
const config = (include) => JSON.stringify({
  compilerOptions: {
    strict: true, target: 'ES2022', module: 'NodeNext', moduleResolution: 'NodeNext', noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true,
    types: ['node'], typeRoots: [`${site}node_modules/@types`], baseUrl: '.', paths: { originlm: [sdk] },
  },
  include,
});
await writeFile(`${dir}package.json`, '{"type":"module"}');
const tsc = `${site}node_modules/typescript/bin/tsc`;
const run = (project) => { try { execFileSync(process.execPath, [tsc, '-p', project], { cwd: dir, stdio: 'pipe' }); return ''; } catch (error) { return String(error.stdout || error.message); } };

await writeFile(`${dir}tsconfig.json`, config(['quickstart.ts', 'summarize.ts']));
const result = run(`${dir}tsconfig.json`);
await writeFile(`${dir}tsconfig.negative.json`, config(['negative.ts']));
const negative = run(`${dir}tsconfig.negative.json`);
await rm(dir, { recursive: true, force: true });

if (result) { console.error(result); process.exit(1); }
if (!/gett/.test(negative)) { console.error('negative control did not fail:\n', negative); process.exit(1); }
console.log('TypeScript samples: quickstart.ts (4 chunks) and summarize.ts compile; the negative control fails as expected.');
