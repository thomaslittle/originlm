/** Render the reduced-motion / no-WebGL stills from the live scene.
 *
 *   npm run dev   (or preview)
 *   node scripts/capture_stills.mjs http://127.0.0.1:5177/
 *
 * Poses window.__originlmScene.state for each still, hides the HTML, screenshots the stage
 * and writes WebP files to public/scene/. Needs Python with Pillow for the WebP step.
 * CHROME_PATH overrides the browser binary.
 */
import { createRequire } from 'node:module';
import { mkdir, rm } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright-core');
const url = process.argv[2] ?? 'http://127.0.0.1:5177/';
const out = fileURLToPath(new URL('../public/scene/', import.meta.url));
const tmp = fileURLToPath(new URL('../.stills/', import.meta.url));
await mkdir(out, { recursive: true });
await mkdir(tmp, { recursive: true });

const base = { heroOut: 0, story: 0, pull: 0, sink: 0, cta: 0 };
const STILLS = [
  { name: 'still-hero', viewport: [1440, 900], scale: 1, pose: { ...base } },
  { name: 'still-hero-narrow', viewport: [390, 844], scale: 2, pose: { ...base } },
  { name: 'still-discover', viewport: [1440, 900], scale: 1, clip: [520, 46, 920, 576], pose: { ...base, heroOut: 1, story: 0.235 } },
  { name: 'still-select', viewport: [1440, 900], scale: 1, clip: [520, 46, 920, 576], pose: { ...base, heroOut: 1, story: 0.47 } },
  { name: 'still-grant', viewport: [1440, 900], scale: 1, clip: [520, 46, 920, 576], pose: { ...base, heroOut: 1, story: 0.615 } },
  { name: 'still-run', viewport: [1440, 900], scale: 1, clip: [520, 46, 920, 576], pose: { ...base, heroOut: 1, story: 0.9 } },
  { name: 'still-noserver', viewport: [1440, 900], scale: 1, pose: { ...base, heroOut: 1, story: 1, pull: 0.92 } },
  { name: 'still-cta', viewport: [1440, 900], scale: 1, pose: { ...base, heroOut: 1, story: 1, pull: 1, sink: 1, cta: 1 } },
];

const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH, headless: true,
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
for (const still of STILLS) {
  const [width, height] = still.viewport;
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: still.scale });
  await page.goto(`${url}?capture&tier=high`, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => window.__originlmScene, null, { timeout: 20000 });
  await page.waitForTimeout(1800);
  await page.addStyleTag({ content: 'main, .nav, .footer, .skip-link, .lens { visibility: hidden !important; } body::after { display: none !important; } .stage { display: block !important; }' });
  await page.evaluate((pose) => {
    const state = window.__originlmScene.state;
    Object.assign(state, pose, { pointer: null, introOverride: 14, timeOverride: 13.5, blinkAt: -1 });
  }, still.pose);
  await page.waitForTimeout(1500);
  const file = `${tmp}${still.name}.png`;
  const clip = still.clip ? { x: still.clip[0], y: still.clip[1], width: still.clip[2], height: still.clip[3] } : undefined;
  await page.screenshot({ path: file, clip });
  await page.close();
  console.log('captured', still.name);
}
await browser.close();

execFileSync('python', ['-c', `
import sys, glob, os
from PIL import Image
tmp, out = sys.argv[1], sys.argv[2]
for path in sorted(glob.glob(os.path.join(tmp, '*.png'))):
    name = os.path.splitext(os.path.basename(path))[0]
    image = Image.open(path).convert('RGB')
    image.save(os.path.join(out, name + '.webp'), 'WEBP', quality=72, method=6)
    print(name, image.size, os.path.getsize(os.path.join(out, name + '.webp')) // 1024, 'KB')
`, tmp, out], { stdio: 'inherit' });
await rm(tmp, { recursive: true, force: true });
