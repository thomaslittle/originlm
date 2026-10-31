/** Real pointer behavior against the built website; no provider operations. */
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {chromium} from '@playwright/test';
const url=process.env.ORIGINLM_SITE_URL ?? 'http://127.0.0.1:5178/';
test('one bounded inner opening follows the pointer, rests and honors reduced motion',async()=>{
 const browser=await chromium.launch({executablePath:process.env.CHROME_PATH,headless:true,args:['--disable-webgl']});
 try {
  const page=await browser.newPage({viewport:{width:1440,height:900},reducedMotion:'no-preference'});
  await page.goto(url);
  const logo=page.locator('.nav-logo');
  assert.equal(await logo.locator('[data-logo-eye]').count(),1,'the large O has one movable inner opening');
  const eye=logo.locator('[data-logo-eye]');
  const fixed=await logo.locator('[data-logo-fixed]').evaluateAll(nodes=>nodes.map(n=>({d:n.getAttribute('d'),transform:n.getAttribute('transform')})));
  const offset=()=>eye.evaluate(el=>{const t=el.transform.baseVal.consolidate()?.matrix;return {x:t?.e??0,y:t?.f??0};});
  await page.mouse.move(1400,850);await page.waitForTimeout(350);
  const first=await offset();assert.ok(first.x>0 && first.y>0);
  for(const [x,y] of [[0,850],[1400,0],[1400,850]]) {
   await page.mouse.move(x,y);await page.waitForTimeout(250);const p=await offset();
   assert.ok((p.x/12)**2+(p.y/10)**2<=1.001,'the inner opening stays inside its movement ellipse');
  }
  assert.deepEqual(await logo.locator('[data-logo-fixed]').evaluateAll(nodes=>nodes.map(n=>({d:n.getAttribute('d'),transform:n.getAttribute('transform')}))),fixed);
  await page.mouse.move(-10,-10);await page.waitForTimeout(450);
  assert.deepEqual(await offset(),{x:0,y:0});
  await page.mouse.move(1300,750);await page.waitForTimeout(450);
  const settled=await eye.getAttribute('transform');await page.waitForTimeout(200);
  assert.equal(await eye.getAttribute('transform'),settled,'no idle movement after settling');
  await page.emulateMedia({reducedMotion:'reduce'});await page.waitForTimeout(50);
  await page.mouse.move(1000,300);await page.waitForTimeout(100);
  assert.deepEqual(await offset(),{x:0,y:0});
 } finally {await browser.close();}
});
