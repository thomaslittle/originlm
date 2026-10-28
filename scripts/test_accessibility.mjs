import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import {createServer as createHttpServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {extname,join,resolve} from 'node:path';
import { chromium } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { createFixtureServer } from '../apps/demo/server.mjs';

/** Sidebar navigation. Routes load lazily, so wait until the new page (not its skeleton) is on screen. */
async function nav(page,name){
 const link=page.locator('[data-slot="sidebar"]').getByRole('link',{name,exact:true});
 // Below the md breakpoint the sidebar is an off-canvas sheet; open it first.
 if(!(await link.count()))await page.getByRole('button',{name:'Toggle Sidebar'}).click();
 await link.click();
 await page.waitForFunction(title=>document.querySelector('#page-title')?.textContent===title&&!document.querySelector('[data-slot="skeleton"]'),name);
}

let server, browser;
before(async()=>{server=await createFixtureServer();browser=await chromium.launch({headless:true});});
after(async()=>{await browser?.close();await server?.close();});
const tags=['wcag2a','wcag2aa','wcag21a','wcag21aa'];
const details=result=>result.violations.map(v=>({id:v.id,impact:v.impact,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}));
async function mountFixture(page,query='#demo-fixture',banner='#mode-banner.synthetic'){
 // The scripted HTTP fixture mounts the public demo explicitly; every record
 // comes from the loopback fixture service, never fabricated in the browser.
 await page.goto(server.url+query);
 await page.locator(banner).waitFor();
}
for(const variant of ['rail','compact','palette','capability','compare','allowance','preset','mobile']){
 test(`showroom ${variant}: no automated WCAG A/AA violations`,async()=>{
  const context=await browser.newContext({viewport:variant==='mobile'?{width:390,height:844}:{width:1440,height:1000}});
  const page=await context.newPage();
  try{
   await mountFixture(page);
   await nav(page,'Selector lab');
   await page.locator(`[data-variant="${variant}"]`).click();
   const result=await new AxeBuilder({page}).withTags(tags).analyze();
   assert.deepEqual(details(result),[]);
  }finally{await context.close();}
 });
}
for(const section of ['Overview','Connections','Models','Usage','Playground','Diagnostics']){
 test(`showroom ${section}: no automated WCAG A/AA violations`,async()=>{
  const context=await browser.newContext();const page=await context.newPage();
  try{
   await mountFixture(page);
   await nav(page,section);
   assert.equal(await page.locator('main main').count(),0,'Each page has one main landmark');
   assert.deepEqual(details(await new AxeBuilder({page}).withTags(tags).analyze()),[]);
  }finally{await context.close();}
 });
}
test('showroom presents only authenticated profiles as connected',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page);
  await nav(page,'Connections');
  const verified=page.getByTestId('verified-profiles');
  assert.equal(await verified.locator('[data-connection-id]').count(),6);
  assert.match(await verified.innerText(),/Personal workspace/);
  assert.doesNotMatch(await verified.innerText(),/Expired sign-in/);
  const previouslyAuthenticated=page.getByTestId('previously-authenticated-profiles');
  assert.equal(await previouslyAuthenticated.locator('[data-connection-id]').count(),1);
  assert.match(await previouslyAuthenticated.innerText(),/Work workspace/);
  assert.match(await previouslyAuthenticated.innerText(),/Previously authenticated/);
  assert.match(await previouslyAuthenticated.innerText(),/stale/i);
  assert.equal(await previouslyAuthenticated.isVisible(),true);
  const detected=page.getByTestId('detected-profiles');
  assert.equal(await detected.locator('[data-connection-id]').count(),1);
  assert.equal(await detected.getByText('Expired sign-in').isVisible(),true);
  assert.match(await detected.innerText(),/Expired sign-in/);
  assert.match(await detected.innerText(),/Expired/);
 }finally{await context.close();}
});
test('overview and connections distinguish stale evidence from never-verified profiles',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page,'#demo-fixture&staleAuth=1&mode=live','#mode-banner.live');
  await nav(page,'Connections');
  const text=await page.locator('body').innerText();
  assert.match(text,/Previously authenticated/);
  assert.match(text,/Personal workspace/);
  assert.doesNotMatch(text,/No authenticated profiles yet/);
  assert.doesNotMatch(text,/No fresh authenticated profiles|No fresh authentication checks/);
  assert.equal(await page.getByRole('heading',{name:'Verified profiles',exact:true}).count(),0);
  assert.equal(await page.getByRole('heading',{name:'Authenticated profiles',exact:true}).count(),0);
  await nav(page,'Overview');
  const overviewStale=page.getByTestId('previously-authenticated-overview');
  assert.equal(await overviewStale.isVisible(),true);
  assert.match(await overviewStale.innerText(),/Personal workspace/);
  await nav(page,'Connections');
  assert.equal(await page.getByTestId('previously-authenticated-profiles').isVisible(),true);
 }finally{await context.close();}
});
test('detected client disclosure remains open across page rerenders unless the user closes it',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page);
  await nav(page,'Connections');
  let detected=page.getByTestId('detected-profiles');
  assert.equal(await detected.evaluate(e=>e.open),true);
  await detected.locator(':scope > summary').click();
  await nav(page,'Overview');
  await nav(page,'Connections');
  detected=page.getByTestId('detected-profiles');
  assert.equal(await detected.evaluate(e=>e.open),false);
 }finally{await context.close();}
});
test('local runtimes and confirmed unauthenticated profiles have separate live states',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page,'#demo-fixture&extraProfiles=1&mode=live','#mode-banner.live');
  await nav(page,'Connections');
  await page.getByRole('heading',{name:'Local runtimes',exact:true}).waitFor();
  assert.equal(await page.getByRole('heading',{name:'LM Studio · Default profile',exact:true}).isVisible(),true);
  assert.equal(await page.getByRole('button',{name:'Refresh local catalog',exact:true}).isVisible(),true);
  const signedOut=page.getByTestId('unauthenticated-profiles');
  assert.equal(await signedOut.locator('[data-connection-id="signed-out"]').count(),1);
  assert.match(await signedOut.innerText(),/Not authenticated/);
  assert.equal(await page.getByTestId('detected-profiles').locator('[data-connection-id="local-lm"]').count(),0);
 }finally{await context.close();}
});
test('live connections expose an explicit bulk metadata check',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page,'#demo-fixture&bulkProbeHook=1&mode=live','#mode-banner.live');
  await nav(page,'Connections');
  await page.getByRole('button',{name:'Check all detected profiles',exact:true}).click();
  assert.equal(await page.locator('body').getAttribute('data-bulk-probe'),'started');
  assert.match(await page.locator('#app').innerText(),/metadata-only/i);
 }finally{await context.close();}
});
test('detected profiles with completed checks are not labeled as still unchecked',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page,'#demo-fixture&probeResults=1&mode=live','#mode-banner.live');
  await nav(page,'Connections');
  const detected=page.getByTestId('detected-profiles');
  assert.match(await detected.innerText(),/checked, not confirmed/);
  assert.doesNotMatch(await detected.innerText(),/need a metadata check/);
  assert.match(await detected.innerText(),/Metadata check unavailable/);
 }finally{await context.close();}
});
test('showroom keeps revision details in the exact-payload disclosure',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page);
  await nav(page,'Selector lab');
  await page.locator('[data-variant="rail"]').click();
  const text=await page.locator('#variant-view').innerText();
  assert.doesNotMatch(text,/input\.(?:text|image)/);
  const config=await page.locator('#configuration-panel').innerText();
  assert.doesNotMatch(config,/catalog_[a-f0-9]{20}/);
  assert.equal(await page.locator('#configuration-panel details[open]').count(),0);
 }finally{await context.close();}
});
test('showroom explains why a route misses a required capability',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page);
  await nav(page,'Selector lab');
  await page.locator('[data-variant="capability"]').click();
  await page.getByRole('checkbox',{name:'Image input'}).check();
  assert.equal(await page.getByRole('checkbox',{name:'Image input'}).isChecked(),true);
  const candidates=await page.locator('#variant-view').innerText();
  assert.match(candidates,/Image input is not supported/);
  assert.doesNotMatch(candidates,/CAPABILITY_(UNKNOWN|UNSUPPORTED|CONDITIONAL)/);
  await page.getByRole('checkbox',{name:'Offline operation'}).check();
  await page.getByRole('checkbox',{name:'Only eligible routes'}).check();
  await page.getByText('No routes have confirmed all selected capabilities.',{exact:true}).waitFor();
  await page.getByText('Unknown support is not treated as supported.',{exact:true}).waitFor();
 }finally{await context.close();}
});
test('showroom selectors with a populated catalog stay within responsive viewports',async()=>{
 const context=await browser.newContext({viewport:{width:1440,height:960}});const page=await context.newPage();
 try{
  await mountFixture(page);
  await nav(page,'Selector lab');
  for(const width of [1440,1324,1024,390]){
   await page.setViewportSize({width,height:900});
   for(const variant of ['rail','compact','palette','capability','compare','allowance','preset','mobile']){
    await page.locator(`[data-variant="${variant}"]`).click();
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true,`${variant} overflows at ${width}px`);
    assert.equal(await page.evaluate(()=>document.querySelector('#configuration-panel').getBoundingClientRect().right<=window.innerWidth),true,`${variant} configuration is clipped at ${width}px`);
    assert.equal(await page.evaluate(()=>document.querySelector('[data-top-actions]').getBoundingClientRect().right<=window.innerWidth),true,`${variant} top actions are clipped at ${width}px`);
   }
  }
 }finally{await context.close();}
});
test('without any native runtime the demo falls back to labeled example data and offers pairing',async()=>{
 // A static host with no /runtime/info (and an SPA-style index fallback for unknown paths).
 const dist=resolve(import.meta.dirname,'../apps/demo/dist');
 const types={'.html':'text/html','.js':'text/javascript','.css':'text/css','.svg':'image/svg+xml','.woff2':'font/woff2'};
 const host=createHttpServer(async(request,response)=>{
  const {pathname}=new URL(request.url,'http://static.test');
  const name=extname(pathname)?pathname.slice(1):'index.html';
  try{const body=await readFile(join(dist,name));response.writeHead(200,{'Content-Type':types[extname(name)]??'application/octet-stream'});response.end(body);}
  catch{response.writeHead(404);response.end();}
 });
 await new Promise(done=>host.listen(0,'127.0.0.1',done));
 const context=await browser.newContext({viewport:{width:1440,height:1000}});const page=await context.newPage();
 try{
  await page.goto(`http://127.0.0.1:${host.address().port}/`);
  await page.locator('#mode-banner.synthetic').waitFor();
  assert.match(await page.locator('#mode-banner').innerText(),/pair a runtime to see yours/i);
  assert.equal(await page.getByRole('button',{name:'Pair a runtime'}).isVisible(),true);
  await nav(page,'Selector lab');
  await page.locator('[data-model-row]').first().waitFor();
 }finally{await context.close();await new Promise(done=>host.close(done));}
});
test('showroom raises no Content Security Policy violations on any page or popup',async()=>{
 // The served policy has no 'unsafe-inline': an injected <style> element or a style attribute would be blocked.
 const context=await browser.newContext({viewport:{width:1440,height:1000}});const page=await context.newPage();
 const violations=[];
 await page.exposeFunction('reportCsp',value=>violations.push(value));
 await page.addInitScript(()=>document.addEventListener('securitypolicyviolation',event=>window.reportCsp(`${event.violatedDirective} ${event.sourceFile}:${event.lineNumber}`)));
 try{
  await mountFixture(page);
  for(const section of ['Overview','Connections','Models','Usage','Selector lab','Playground','Diagnostics']){
   await nav(page,section);
   if(section==='Usage')await page.locator('[data-slot="chart"]').first().waitFor();
   if(section==='Playground'){await page.getByRole('button',{name:/^(Model:|Choose a model)/}).click();await page.getByRole('option').first().waitFor();await page.keyboard.press('Escape');}
  }
  await nav(page,'Selector lab');
  await page.getByTestId('model-codex-personal-0').click();
  await page.getByRole('combobox',{name:'Reasoning effort',exact:true}).click();
  await page.getByRole('option',{name:'Cosmic',exact:true}).waitFor();
  await page.keyboard.press('Escape');
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox',{name:'Search every model'}).waitFor();
  assert.deepEqual(violations,[]);
 }finally{await context.close();}
});
test('showroom rendered text has no common mojibake markers',async()=>{
 const context=await browser.newContext();const page=await context.newPage();
 try{
  await mountFixture(page);
  for(const section of ['Overview','Connections','Models','Usage','Selector lab','Playground','Diagnostics']){
   await nav(page,section);
   const text=await page.locator('body').innerText();
   assert.doesNotMatch(text,/(?:Â|Ã|â€|â€¦|ï»¿|ðŸ)/,`${section} contains garbled text`);
  }
 }finally{await context.close();}
});

