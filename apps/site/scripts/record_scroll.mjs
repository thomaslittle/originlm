/** Record a scroll-through of the page with the CDP screencast and encode it with ffmpeg.
 *
 *   FFMPEG=path/to/ffmpeg node scripts/record_scroll.mjs <url> <out.webm> [--gpu]
 *
 * The screencast delivers frames as the compositor produces them; ffmpeg re-times them to
 * 30 fps from their timestamps. CHROME_PATH overrides the browser binary.
 */
import { createRequire } from 'node:module';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright-core');
const [url = 'http://127.0.0.1:5178/', out = 'scroll-through.webm'] = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const gpu = process.argv.includes('--gpu');
const dir = join(tmpdir(), `originlm-scroll-${Date.now()}`);
await mkdir(dir, { recursive: true });

const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH, headless: true,
  args: gpu ? ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist'] : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const cdp = await page.context().newCDPSession(page);
const frames = [];
cdp.on('Page.screencastFrame', async ({ data, metadata, sessionId }) => {
  frames.push({ data, t: metadata.timestamp });
  await cdp.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
});
await page.goto(`${url}?tier=high`, { waitUntil: 'domcontentloaded' });
await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 80, everyNthFrame: 1 });
// The wake: let the eye open and scan, with the pointer drifting across the hero.
for (let i = 0; i < 40; i++) { await page.mouse.move(700 + Math.sin(i / 6) * 420, 300 + Math.cos(i / 5) * 180); await page.waitForTimeout(110); }
// Then wheel through the story, the pull-back, the docs and the closing shot.
const total = await page.evaluate(() => document.documentElement.scrollHeight - innerHeight);
for (let y = 0; y < total; y += 110) { await page.mouse.wheel(0, 110); await page.waitForTimeout(y > 22000 && y < total - 5000 ? 8 : 34); }
await page.waitForTimeout(2500);
await cdp.send('Page.stopScreencast');
await browser.close();

let list = '';
for (let i = 0; i < frames.length; i++) {
  const name = `f${String(i).padStart(5, '0')}.jpg`;
  await writeFile(join(dir, name), Buffer.from(frames[i].data, 'base64'));
  const duration = i + 1 < frames.length ? Math.max(0.001, frames[i + 1].t - frames[i].t) : 0.5;
  list += `file '${name}'\nduration ${duration.toFixed(4)}\n`;
}
await writeFile(join(dir, 'frames.txt'), list);
execFileSync(process.env.FFMPEG ?? 'ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', join(dir, 'frames.txt'),
  '-vf', 'scale=960:-2,fps=30', '-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '40', '-row-mt', '1', '-an', out], { stdio: 'inherit' });
await rm(dir, { recursive: true, force: true });
console.log(`${frames.length} frames -> ${out}`);
