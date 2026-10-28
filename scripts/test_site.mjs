/** Headless landing-page contracts. Run against the production preview on 5177. */
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {chromium} from '@playwright/test';
const url=process.env.ORIGINLM_SITE_URL ?? 'http://127.0.0.1:5177/';

test('prerendered content, mobile layout and reduced-motion fallback remain usable',async()=>{
 const browser=await chromium.launch({executablePath:process.env.CHROME_PATH,headless:true});
 try {
  for(const width of [390,1440]) {
   const context=await browser.newContext({viewport:{width,height:900},reducedMotion:'reduce'});
   const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
   await page.goto(url,{waitUntil:'networkidle'});
   await page.locator('.mini-view-figure img').scrollIntoViewIfNeeded();
   assert.ok((await page.locator('.mini-view-figure').boundingBox()).height>200);
   assert.equal(await page.locator('.hero-stage canvas').count(),0);
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
   assert.deepEqual(errors,[]);
   await context.close();
  }
  const context=await browser.newContext({javaScriptEnabled:false});const page=await context.newPage();
  await page.goto(url);assert.match(await page.locator('h1').innerText(),/See your AI/);
  assert.ok(await page.locator('#install').innerText());
  await page.locator('.layer-key summary').click();assert.equal(await page.locator('.layer-key').getAttribute('open'),'');
  await context.close();
 } finally {await browser.close();}
});

test('scroll separates the five real UI planes and reverses when returning to the top',async()=>{
 const browser=await chromium.launch({executablePath:process.env.CHROME_PATH,headless:true,args:['--enable-unsafe-swiftshader']});
 try {
  const page=await browser.newPage({viewport:{width:1440,height:900},reducedMotion:'no-preference'});
  // Software WebGL must animate by default, without a hidden query override.
  await page.goto(url);
  await page.locator('.hero-stage[data-ready]').waitFor({timeout:60000});
  // Leave the camera intro before measuring separation. The test uses software
  // WebGL for behavior; it makes no hardware frame-rate claim.
  await page.waitForTimeout(2800);
  const opacity=()=>page.locator('.layer-label').first().evaluate(el=>Number(el.style.opacity));
  assert.ok(await opacity()<0.05);
  const distance=await page.locator('.hero-scroll').evaluate(el=>(el.getBoundingClientRect().height-el.querySelector('.hero').getBoundingClientRect().height)*0.8);
  await page.evaluate(y=>window.scrollTo({top:y,behavior:'instant'}),distance);
  await page.waitForTimeout(1400);assert.ok(await opacity()>0.8);
  const stage=await page.locator('.hero-stage').boundingBox();
  assert.ok(stage.y>=72 && stage.y+stage.height<=900,'The exploding stack stays in the viewport');
  await page.evaluate(()=>window.scrollTo({top:0,behavior:'instant'}));
  await page.waitForTimeout(1400);assert.ok(await opacity()<0.05);
 } finally {await browser.close();}
});

test('without WebGL the shared image planes still separate and reverse on scroll',async()=>{
 const browser=await chromium.launch({executablePath:process.env.CHROME_PATH,headless:true,args:['--disable-webgl']});
 try {
  const page=await browser.newPage({viewport:{width:1440,height:900},reducedMotion:'no-preference'});
  await page.goto(url);
  await page.locator('.hero-stage[data-ready][data-renderer="css"]').waitFor();
  assert.equal(await page.locator('.css-hero-plane').count(),5);
  const spread=()=>page.locator('.css-hero-scene').evaluate(el=>Number(el.style.getPropertyValue('--hero-spread')));
  const distance=await page.locator('.hero-scroll').evaluate(el=>(el.getBoundingClientRect().height-el.querySelector('.hero').getBoundingClientRect().height)*0.8);
  await page.evaluate(y=>window.scrollTo({top:y,behavior:'instant'}),distance);
  await page.waitForTimeout(1400);assert.ok(await spread()>0.8);
  await page.screenshot({path:'.verification/hero-css-separated.png'});
  await page.evaluate(()=>window.scrollTo({top:0,behavior:'instant'}));
  await page.waitForTimeout(1400);assert.ok(await spread()<0.05);
  await page.getByRole('button',{name:'Pause animation',exact:true}).click();
  assert.equal(await page.locator('.css-hero-plane').count(),0);
  await page.getByRole('button',{name:'Play animation',exact:true}).click();
  await page.locator('.hero-stage[data-ready][data-renderer="css"]').waitFor();
 } finally {await browser.close();}
});
