/** Headless review evidence. The app capture refuses anything except example data. */
import {chromium} from '@playwright/test';
import {mkdir,writeFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {resolve} from 'node:path';
const out=resolve('docs/review/screens');
const browser=await chromium.launch({headless:true,args:['--enable-unsafe-swiftshader']});
const errors=[];
try {
 for(const width of [1440,390]) {
  const context=await browser.newContext({viewport:{width,height:width===390?844:1000},reducedMotion:'reduce'});
  const page=await context.newPage();page.on('pageerror',error=>errors.push(error.message));
  await mkdir(resolve(out,'web'),{recursive:true});
  for(const route of ['overview','connections','models','usage','playground','selector-lab','diagnostics','settings']) {
   await page.goto(`http://127.0.0.1:5176/${route}`,{waitUntil:'networkidle'});
   await page.locator('#mode-banner.synthetic').waitFor();
   await page.waitForFunction(()=>![...document.querySelectorAll('[data-slot="skeleton"]')].some(el=>el.getClientRects().length));
   if(route==='usage')await page.locator('.recharts-surface').first().waitFor({state:'visible'});
   await page.evaluate(async()=>{
    await document.fonts.ready;
    // CSS masks can be fetched before their SVG raster is ready to paint.
    await Promise.all([...document.querySelectorAll('.provider-mark')].map(async el=>{
     const match=getComputedStyle(el).maskImage.match(/^url\(["']?(.*?)["']?\)$/);
     if(!match)return;
     const image=new Image();image.src=match[1];await image.decode();
    }));
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
   });
   await page.screenshot({path:resolve(out,`web/${route}-${width}.png`)});
  }
  await mkdir(resolve(out,'site'),{recursive:true});
  await page.goto('http://127.0.0.1:5177/',{waitUntil:'networkidle'});
  await page.screenshot({path:resolve(out,`site/${width}-reduced.png`)});
  if(width===390){await page.locator('.static-layer-names').scrollIntoViewIfNeeded();await page.screenshot({path:resolve(out,'site/390-layer-names.png')});}
  await context.close();
 }
 const context=await browser.newContext({viewport:{width:1440,height:900},reducedMotion:'no-preference'});
 const page=await context.newPage();page.on('pageerror',error=>errors.push(error.message));
 await page.goto('http://127.0.0.1:5177/?3d&light');
 await page.locator('.hero-stage[data-ready]').waitFor({timeout:60000});
 await page.waitForTimeout(3000);
 await page.screenshot({path:resolve(out,'site/1440-collapsed.png')});
 const distance=await page.locator('.hero-scroll').evaluate(el=>(el.getBoundingClientRect().height-el.querySelector('.hero').getBoundingClientRect().height)*0.8);
 await page.evaluate(y=>window.scrollTo({top:y,behavior:'instant'}),distance);
 await page.waitForTimeout(1400);
 await page.screenshot({path:resolve(out,'site/1440-exploded.png')});
 await context.close();
 if(errors.length)throw Error(JSON.stringify(errors));
 await writeFile(resolve(out,'capture.json'),JSON.stringify({sourceCommit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),sourceDirty:Boolean(execFileSync('git',['diff','--name-only','HEAD','--','apps/demo/src','apps/site/src','brand'],{encoding:'utf8'}).trim()),app:'127.0.0.1:5176, example data only',site:'127.0.0.1:5177, prerendered production preview',browser:'Headless Chromium; software WebGL for the two scene captures',widths:[1440,390],errors},null,2)+'\n');
} finally {await browser.close();}
