/** Load and frame-time measurements for the landing page.
 *
 *   node scripts/measure_perf.mjs <url> [--gpu] [--cpu=4] [--width=1440] [--height=900] [--dpr=1]
 *
 * LCP and CLS come from PerformanceObserver (buffered); TBT is the sum of long-task time
 * over 50ms between first contentful paint and 6s. Frame times are requestAnimationFrame
 * deltas measured in the page while the hero idles and while the story is scrubbed by
 * wheel input. --gpu uses the machine's GPU through ANGLE/D3D11; without it Chromium uses
 * SwiftShader (software). Neither is a mid-range laptop: report which one you used.
 */
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright-core');
const args = process.argv.slice(2);
const url = args.find((a) => !a.startsWith('--')) ?? 'http://127.0.0.1:5178/';
const opt = (name, fallback) => Number(args.find((a) => a.startsWith(`--${name}=`))?.split('=')[1] ?? fallback);
const gpu = args.includes('--gpu');
const cpu = opt('cpu', 1), width = opt('width', 1440), height = opt('height', 900), dpr = opt('dpr', 1);

const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH, headless: true,
  args: gpu ? ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist'] : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
});
const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: dpr });
const page = await context.newPage();
const cdp = await context.newCDPSession(page);
if (cpu > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: cpu });
await page.addInitScript(() => {
  window.__perf = { lcp: 0, cls: 0, longTasks: [], fcp: 0 };
  new PerformanceObserver((list) => { for (const e of list.getEntries()) window.__perf.lcp = e.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true });
  new PerformanceObserver((list) => { for (const e of list.getEntries()) if (!e.hadRecentInput) window.__perf.cls += e.value; }).observe({ type: 'layout-shift', buffered: true });
  new PerformanceObserver((list) => { for (const e of list.getEntries()) window.__perf.longTasks.push([e.startTime, e.duration]); }).observe({ type: 'longtask', buffered: true });
  new PerformanceObserver((list) => { for (const e of list.getEntries()) if (e.name === 'first-contentful-paint') window.__perf.fcp = e.startTime; }).observe({ type: 'paint', buffered: true });
});
await page.goto(url, { waitUntil: 'load' });
await page.waitForTimeout(6000);
const load = await page.evaluate(() => {
  const p = window.__perf;
  const tbt = p.longTasks.filter(([start]) => start >= p.fcp && start <= 6000).reduce((sum, [, d]) => sum + Math.max(0, d - 50), 0);
  const lcpEl = performance.getEntriesByType('largest-contentful-paint').at(-1)?.element;
  return {
    fcp: Math.round(p.fcp), lcp: Math.round(p.lcp), cls: Number(p.cls.toFixed(4)), tbt: Math.round(tbt),
    lcpElement: lcpEl ? `${lcpEl.tagName.toLowerCase()}.${lcpEl.className}` : null,
    sceneLiveAt: Math.round(performance.getEntriesByName('scene-live')[0]?.startTime ?? -1),
    renderer: window.__originlmScene?.stats().renderer, tier: window.__originlmScene?.stats().tier,
  };
});

async function frameWindow(ms, during) {
  await page.evaluate((duration) => {
    window.__frames = [];
    let last = performance.now();
    const end = last + duration;
    const tick = (now) => { window.__frames.push(now - last); last = now; if (now < end) requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  }, ms);
  if (during) await during();
  await page.waitForTimeout(ms + 200);
  return page.evaluate(() => {
    const f = window.__frames.slice(2).sort((a, b) => a - b);
    const avg = f.reduce((a, b) => a + b, 0) / f.length;
    return { frames: f.length, avgMs: Number(avg.toFixed(2)), fps: Number((1000 / avg).toFixed(1)), p95Ms: Number(f[Math.floor(f.length * 0.95)].toFixed(2)), over33ms: f.filter((x) => x > 33.4).length };
  });
}
await page.mouse.move(width * 0.6, height * 0.4);
const idle = await frameWindow(3000, async () => { for (let i = 0; i < 20; i++) { await page.mouse.move(width * (0.3 + i * 0.03), height * (0.3 + (i % 5) * 0.08)); await page.waitForTimeout(120); } });
const scrub = await frameWindow(4000, async () => { for (let i = 0; i < 40; i++) { await page.mouse.wheel(0, 120); await page.waitForTimeout(90); } });
console.log(JSON.stringify({ url, gpu, cpuThrottle: cpu, viewport: `${width}x${height}@${dpr}`, load, idle, scrub }, null, 2));
await browser.close();
