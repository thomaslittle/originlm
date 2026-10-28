/** Render the production React bundle so asset URLs match the static build. */
import { readFile, writeFile, rm } from 'node:fs/promises';
import { render } from '../.prerender/entry-server.js';

const path = new URL('../dist/index.html', import.meta.url);
const template = await readFile(path, 'utf8');
if (!template.includes('<div id="root"></div>')) throw new Error('Missing prerender mount');
await writeFile(path, template.replace('<div id="root"></div>', `<div id="root">${render()}</div>`));
await rm(new URL('../.prerender', import.meta.url), { recursive: true, force: true });
