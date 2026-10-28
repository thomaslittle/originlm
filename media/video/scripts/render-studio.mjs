/** Headless deterministic shot renderer. Requires the site on 127.0.0.1:5177. */
import { chromium } from '../../../node_modules/playwright-core/index.mjs';
import { mkdir } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '../../..');
const out = resolve(root, 'media/video/out/studio-frames');
await mkdir(out, { recursive: true });
if (!process.argv.includes('--encode-only')) {
const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH ?? '/usr/bin/google-chrome', headless: true, args: ['--enable-unsafe-swiftshader'] });
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on('pageerror', (error) => { throw error; });
  await page.goto('http://127.0.0.1:5177/scene-render.html', { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.originStudioReady, { timeout: 60000 });
  for (let index = 0; index < 270; index++) {
    const seconds = index / 30;
    // Macro pull-back and explosion, a legible hold, then one collapse beat.
    const intro = Math.min(1, seconds / 3.2);
    const spring = (time) => time <= 0 ? 0 : 1 - Math.exp(-9 * time) * (Math.cos(Math.sqrt(39) * time) + 9 / Math.sqrt(39) * Math.sin(Math.sqrt(39) * time));
    const spread = Math.max(0, Math.min(1, spring(seconds - 0.6) * (1 - spring(seconds - 7.4))));
    await page.evaluate((frame) => window.renderOriginFrame(frame), { seconds, intro, spread });
    await page.screenshot({ path: resolve(out, `${String(index).padStart(4, '0')}.png`) });
    if (index % 30 === 0) console.log(`Rendered ${index / 30}s / 9s`);
  }
} finally { await browser.close(); }
}
const video = resolve(root, 'media/video/out/studio.mp4');
const encoder = process.env.VIDEO_ENCODER ?? 'libx264';
const quality = encoder === 'libopenh264' ? ['-b:v', '24M'] : ['-crf', '16'];
execFileSync(process.env.FFMPEG ?? 'ffmpeg', ['-y', '-framerate', '30', '-i', resolve(out, '%04d.png'), '-c:v', encoder, ...quality, '-pix_fmt', 'yuv420p', '-movflags', '+faststart', video], { stdio: 'inherit' });
console.log(video);
