/** Static posters from the same scene, including a tighter mobile composition. */
import { chromium } from '../../../node_modules/playwright-core/index.mjs';
import { execFileSync } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '../../..');
const scratch = resolve(root, '.verification');
await mkdir(scratch, {recursive:true});
const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH ?? '/usr/bin/google-chrome', headless:true, args:['--enable-unsafe-swiftshader'] });
try {
  const page = await browser.newPage({viewport:{width:1440,height:1000},deviceScaleFactor:1});
  await page.goto('http://127.0.0.1:5177/scene-render.html?light', {waitUntil:'domcontentloaded'});
  await page.waitForFunction(()=>window.originStudioReady, {timeout:60000});
  await page.evaluate(()=>window.renderOriginFrame({seconds:5,spread:1,intro:1}));
  for (const [name, clip] of [['poster', undefined], ['poster-mobile', {x:280,y:80,width:960,height:850}]]) {
    const file = resolve(scratch, `${name}.png`);
    await page.screenshot({path:file, ...(clip ? {clip} : {})});
    execFileSync(process.env.FFMPEG ?? 'ffmpeg', ['-y','-i',file,'-c:v','libwebp','-quality','90',resolve(root,`apps/site/public/scene/${name}.webp`)], {stdio:'ignore'});
  }
} finally { await browser.close(); }
