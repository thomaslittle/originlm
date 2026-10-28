/** Real app captures for the site/film. Refuses live mode; runs without a window. */
import { chromium } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const scenes = resolve(root, 'apps/site/public/scene');
const scratch = resolve(root, '.verification/ui-captures');
await mkdir(scenes, { recursive: true }); await mkdir(scratch, { recursive: true });
const encode = (source, target) => execFileSync(process.env.FFMPEG ?? 'ffmpeg', ['-y', '-i', source, '-c:v', 'libwebp', '-quality', '90', target], { stdio: 'ignore' });
const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH ?? '/usr/bin/google-chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const open = async (route) => {
    await page.goto(`http://127.0.0.1:5176/${route}`, { waitUntil: 'domcontentloaded' });
    await page.locator('#mode-banner.synthetic').waitFor({ state: 'visible' });
    await page.evaluate(() => document.fonts.ready);
    if (route === 'overview') await page.getByRole('heading', { name: 'Cost trend — example data', exact: true }).waitFor({ state: 'visible', timeout: 10000 });
  };
  await open('overview');
  const ring = await page.locator('section[aria-labelledby="allowances-title"] svg').first().boundingBox();
  if (!ring) throw Error('Example quota ring missing');
  const app = resolve(scratch, 'app.png');
  await page.screenshot({ path: app }); encode(app, resolve(scenes, 'app.webp'));
  await page.locator('[data-capture="usage-and-costs"]').screenshot({ path: resolve(root, 'media/video/media/dashboard-detail.png') });
  await writeFile(resolve(scenes, 'manifest.json'), JSON.stringify({ source: 'apps/demo /overview, example mode', width: 1440, height: 1000, dpr: 2, focus: { u: (ring.x + ring.width / 2) / 1440, v: (ring.y + ring.height / 2) / 1000 } }, null, 2) + '\n');
  const provider = resolve(scratch, 'providers.png');
  await page.locator('[data-capture="providers"]').screenshot({ path: provider }); encode(provider, resolve(scenes, 'providers.webp'));
  await open('diagnostics');
  await page.getByRole('button', { name: 'Refresh diagnostics', exact: true }).click();
  for (const name of ['sdk', 'transport', 'runtime']) {
    const file = resolve(scratch, `${name}.png`);
    await page.locator(`[data-capture="${name}"]`).screenshot({ path: file }); encode(file, resolve(scenes, `${name}.webp`));
    if (name === 'runtime') {
      await page.setViewportSize({ width: 800, height: 1000 });
      await page.locator('[data-capture="runtime"]').screenshot({ path: resolve(root, 'media/video/media/runtime.png') });
      await page.setViewportSize({ width: 1440, height: 1000 });
    }
  }
  for (const route of ['diagnostics', 'playground']) {
    await open(route);
    if (route === 'diagnostics') await page.getByRole('button', { name: 'Refresh diagnostics', exact: true }).click();
    await page.evaluate(() => window.scrollTo(0, 0));
    const file = resolve(scratch, `${route}.png`);
    await page.screenshot({ path: file });
    encode(file, resolve(root, `apps/site/public/screens/control-center-${route}.webp`));
  }
  await open('connections');
  const connections = resolve(scratch, 'connections.png');
  await page.screenshot({ path: connections }); encode(connections, resolve(root, 'apps/site/public/screens/control-center-connections.webp'));
} finally { await browser.close(); }
