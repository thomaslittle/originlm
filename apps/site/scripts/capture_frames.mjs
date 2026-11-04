/** Capture review frames of the landing page at every scroll stage, desktop and mobile.
 *
 *   node scripts/capture_frames.mjs <url> <outDir> [--gpu] [--video] [--reduced] [--only=desktop|mobile]
 *
 * Headless Chromium has no GPU; WebGL runs on SwiftShader, which is slow and is not a real
 * GPU. Frame-rate numbers from this script are a floor, not a measurement of a laptop.
 * CHROME_PATH overrides the browser binary.
 */
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright-core');
const [url = 'http://127.0.0.1:5178/', out = 'frames'] = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const flags = new Set(process.argv.slice(2).filter((a) => a.startsWith('--')));
const only = [...flags].find((f) => f.startsWith('--only='))?.split('=')[1];
await mkdir(out, { recursive: true });

const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH,
  headless: true,
  // --gpu renders on the machine's GPU (ANGLE/D3D11); otherwise SwiftShader.
  args: flags.has('--gpu') ? ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist'] : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'],
});

/** Scroll positions for each stage, computed in the page. */
const STAGES = [
  ['01-hero', { at: 'top' }],
  ['02-discover', { story: 0.17 }],
  ['03-select', { story: 0.44 }],
  ['04-grant', { story: 0.61 }],
  ['05-run', { story: 0.93 }],
  ['06-no-server', { pull: 0.82 }],
  ['07-docs-sdk', { id: 'sdk' }],
  ['08-usage', { id: 'usage' }],
  ['09-install-cta', { cta: true }],
];

async function scrollTo(page, stage) {
  await page.evaluate((s) => {
    const vh = innerHeight;
    const top = (el) => el.getBoundingClientRect().top + scrollY;
    let y = 0;
    if (s.story !== undefined) { const el = document.getElementById('story'); y = top(el) + (el.offsetHeight - vh) * s.story; }
    else if (s.pull !== undefined) { const el = document.querySelector('#how .noserver-track'); y = top(el) + (el.offsetHeight - vh) * s.pull; }
    else if (s.id) { y = top(document.getElementById(s.id)) - 80; }
    else if (s.cta) { const el = document.querySelector('.final-cta'); y = top(el) + el.offsetHeight - vh; }
    window.scrollTo(0, y);
  }, stage);
}

for (const [name, viewport] of [['desktop', { width: 1440, height: 900 }], ['mobile', { width: 390, height: 844 }]]) {
  if (only && only !== name) continue;
  const context = await browser.newContext({
    viewport, deviceScaleFactor: 1, reducedMotion: flags.has('--reduced') ? 'reduce' : 'no-preference',
    recordVideo: flags.has('--video') ? { dir: out, size: viewport } : undefined,
  });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(`${url}${url.includes('?') ? '&' : '?'}tier=high`, { waitUntil: 'networkidle' });
  await page.mouse.move(viewport.width * 0.62, viewport.height * 0.35);
  await page.waitForTimeout(5200);
  const info = await page.evaluate(() => ({ motion: document.documentElement.className, stats: window.__originlmScene?.stats?.() }));
  console.log(name, JSON.stringify(info));
  for (const [stageName, stage] of STAGES) {
    if (stage.at !== 'top') {
      await scrollTo(page, stage);
      await page.waitForTimeout(1600);
    }
    await page.screenshot({ path: `${out}/${name}-${stageName}.jpg`, type: 'jpeg', quality: 74 });
  }
  if (flags.has('--video')) {
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.waitForTimeout(800);
    const total = await page.evaluate(() => document.documentElement.scrollHeight - innerHeight);
    for (let y = 0; y < total; y += 90) { await page.mouse.wheel(0, 90); await page.waitForTimeout(30); }
    await page.waitForTimeout(1500);
  }
  if (errors.length) console.log(name, 'errors:', errors);
  await context.close();
}
await browser.close();
