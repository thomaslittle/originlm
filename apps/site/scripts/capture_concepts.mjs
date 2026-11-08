/** Screenshots of the landing concepts, and a 390px overflow check.
 *
 *   npm run dev            (port 5177)
 *   node scripts/capture_concepts.mjs [baseUrl] [--only=c1,c2] [--out=concept-shots]
 *
 * Per concept: 1440x900 hero (live motion, after it settles), 1440 full page and
 * 390x844 hero. Full pages load with ?still, the concept's static reduced-motion
 * layout, so every section is visible in one stitched image. Same Chromium flags as
 * capture_council.mjs. Exits non-zero if any page scrolls horizontally at 390px.
 */
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright-core');
const args = process.argv.slice(2);
const base = (args.find((a) => !a.startsWith('--')) ?? 'http://127.0.0.1:5177').replace(/\/$/, '');
const only = args.find((a) => a.startsWith('--only='))?.split('=')[1]?.split(',');
const out = args.find((a) => a.startsWith('--out='))?.split('=')[1] ?? 'concept-shots';
await mkdir(out, { recursive: true });

const PAGES = ['c1', 'c2', 'c3', 'c4', 'c5', 'concepts'].filter((id) => !only || only.includes(id));
/* Hero shots use SwiftShader ANGLE so WebGL concepts render live. Its 8192px texture
   limit makes Chromium repeat the top of any taller full-page capture, so full pages use
   a second browser with the default backend (WebGL still falls back to software). */
const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH, headless: true,
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--force-color-profile=srgb', '--hide-scrollbars'],
});
const tallBrowser = await chromium.launch({
  executablePath: process.env.CHROME_PATH, headless: true,
  args: ['--enable-unsafe-swiftshader', '--force-color-profile=srgb', '--hide-scrollbars'],
});

let overflow = 0;
async function shot(id, { width, height, scale = 1, still = false, full = false, name, settle = 2600 }) {
  const page = await (full ? tallBrowser : browser).newPage({ viewport: { width, height }, deviceScaleFactor: scale, reducedMotion: still ? 'reduce' : 'no-preference' });
  await page.goto(`${base}/${id}${still ? '?still' : ''}`, { waitUntil: 'networkidle' });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(settle);
  if (full) {
    // Walk the page once so lazy sections and observers settle, then return to the top.
    const h = await page.evaluate(() => document.documentElement.scrollHeight);
    for (let y = 0; y < h; y += height) { await page.evaluate((yy) => window.scrollTo(0, yy), y); await page.waitForTimeout(60); }
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.waitForTimeout(400);
  }
  if (width <= 400) {
    const o = await page.evaluate(() => ({ sw: document.documentElement.scrollWidth, cw: document.documentElement.clientWidth }));
    if (o.sw > o.cw) { overflow++; console.log(`OVERFLOW ${id} @${width}: scrollWidth ${o.sw} > ${o.cw}`); }
    else console.log(`ok ${id} @${width}: no horizontal overflow`);
  }
  await page.screenshot({ path: `${out}/${name}.png`, fullPage: full, animations: full ? 'disabled' : 'allow' });
  await page.close();
  console.log('captured', name);
}

for (const id of PAGES) {
  await shot(id, { width: 1440, height: 900, name: `${id}-1440-hero` });
  await shot(id, { width: 1440, height: 900, still: true, full: true, name: `${id}-1440-full`, settle: 900 });
  await shot(id, { width: 390, height: 844, scale: 2, name: `${id}-390-hero` });
  // The overflow check also runs over the full mobile page in its static layout.
  await shot(id, { width: 390, height: 844, still: true, full: true, name: `${id}-390-full`, settle: 900 });
}
await browser.close();
await tallBrowser.close();
process.exit(overflow ? 1 : 0);
