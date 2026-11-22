/** Render the production React bundle so asset URLs match the static build. */
import { readFile, writeFile, rm } from 'node:fs/promises';
import { render, VERSION } from '../.prerender/entry-server.js';

const path = new URL('../dist/index.html', import.meta.url);
const template = await readFile(path, 'utf8');
if (!template.includes('<div id="root"></div>')) throw new Error('Missing prerender mount');
const html = template
  .replace(/"softwareVersion":\s*"[^"]+"/, `"softwareVersion": ${JSON.stringify(VERSION)}`)
  .replace('<div id="root"></div>', `<div id="root">${render()}</div>`);
await writeFile(path, html);
await rm(new URL('../.prerender', import.meta.url), { recursive: true, force: true });
