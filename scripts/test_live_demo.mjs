import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdtemp,mkdir,rm,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,join} from 'node:path';
import {chromium} from '@playwright/test';
import {liveDemoExecutionArgs} from './live_demo_policy.mjs';

/** Sidebar navigation. Routes load lazily, so wait until the new page (not its skeleton) is on screen. */
async function nav(page,name){
 const link=page.locator('[data-slot="sidebar"]').getByRole('link',{name,exact:true});
 // Below the md breakpoint the sidebar is an off-canvas sheet; open it first.
 if(!(await link.count()))await page.getByRole('button',{name:'Toggle Sidebar'}).click();
 await link.click();
 await page.waitForFunction(title=>document.querySelector('#page-title')?.textContent===title&&!document.querySelector('[data-slot="skeleton"]'),name);
}

const root=resolve(import.meta.dirname,'..');
const binary=process.env.LAI_NATIVE_BINARY?resolve(root,process.env.LAI_NATIVE_BINARY):resolve(root,'target/debug',process.platform==='win32'?'originlm.exe':'originlm');
test('live demo authorizes every supported provider in its isolated workspace without enabling paid use',()=>{
 const args=liveDemoExecutionArgs('C:\demo\workspace');
 for(const provider of ['codex','claude_code','opencode','ollama','lmstudio','openai_api','anthropic_api','gemini_api','xai_api','compatible_api'])assert.ok(args.includes(provider),provider);
 assert.equal(args.includes('cursor'),false);
 assert.equal(args.includes('--allow-paid-options'),false);
 assert.equal(args.at(-1),'C:\demo\workspace');
 assert.throws(()=>liveDemoExecutionArgs(''),/LIVE_DEMO_WORKSPACE_REQUIRED/);
});
test('registered ACP profiles join the live demo provider grant when configured',()=>{
 const args=liveDemoExecutionArgs('/tmp/workspace',{nativeProfiles:true});
 for(const provider of ['grok','gemini_cli','antigravity'])assert.ok(args.includes(provider));
});
async function setup(){
 const home=await mkdtemp(join(tmpdir(),'originlm-browser-'));await mkdir(join(home,'.codex'));
 const child=spawn(binary,['serve','--http','--port','0','--home',home,'--assets',resolve(root,'apps/demo/dist'),'--allow-passive-discovery','--allow-metadata','codex'],{stdio:['ignore','pipe','pipe'],windowsHide:true});
 let output='';const config=await new Promise((res,rej)=>{const timer=setTimeout(()=>rej(Error('HOST_START_TIMEOUT')),10000);child.on('error',rej);child.once('exit',()=>rej(Error('HOST_EXIT')));child.stdout.on('data',b=>{output+=b;const url=output.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0],code=output.match(/minutes\): ([0-9a-f]{12})/)?.[1];if(url&&code){clearTimeout(timer);res({url,code});}});child.stderr.on('data',()=>{});});
 let browser;try{browser=await chromium.launch({headless:true,channel:process.env.LAI_BROWSER_CHANNEL||undefined});}catch(error){child.kill();await rm(home,{recursive:true,force:true});throw error;}const page=await browser.newPage();page.setDefaultTimeout(5000);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 return {...config,page,browser,errors,async close(){await browser.close();child.kill();await new Promise(r=>{if(child.exitCode!==null)return r();child.once('exit',r);});await rm(home,{recursive:true,force:true});}};
}
test('real native host requires explicit pairing, then discovers real empty-home metadata',async()=>{
 const app=await setup();try{
  await app.page.goto(app.url);await app.page.getByRole('heading',{name:'Connect your local runtime'}).waitFor({timeout:5000});
  await app.page.getByText(/pairing checks metadata only/i).waitFor();
  assert.equal(await app.page.locator('#mode-banner').count(),0);
  await app.page.getByRole('button',{name:'Show current pairing code'}).click();
  await app.page.waitForFunction(()=>document.querySelector('#pairing-code')?.value.length===12,null,{timeout:5000});
  assert.equal(await app.page.getByLabel('Pairing code').inputValue(),app.code,await app.page.locator('.pairing-card [aria-live="polite"]').innerText());
  await app.page.getByLabel('Pairing code').fill(app.code);
  await app.page.getByRole('button',{name:'Pair with this runtime'}).click();
  await app.page.locator('#mode-banner.live').waitFor();
  await app.page.locator('[data-slot="sidebar"]').getByRole('link',{name:'Connections',exact:true}).waitFor();
  const detected=app.page.getByTestId('detected-profiles');
  await detected.waitFor();
  assert.equal(await app.page.getByTestId('verified-profiles').count(),0);
  assert.equal(await detected.getByRole('heading',{name:/Codex/}).first().isVisible(),true);
  await nav(app.page,'Selector lab');
  await app.page.locator('[data-variant="compare"]').click();
  await app.page.getByText('No routes match the current filters.',{exact:true}).waitFor();
  await nav(app.page,'Overview');
  assert.equal(await app.page.getByText(/Synthetic mode remains a separate/i).count(),0);
  assert.match(await app.page.locator('#mode-banner').innerText(),/Live connection/);
  assert.equal(await app.page.getByText('Synthetic showcase',{exact:true}).count(),0);
  assert.equal(app.errors.length,0,app.errors.join('\n'));
 }finally{await app.close();}
});
test('pairing automatically checks metadata and reports an outcome for every detected connection',async()=>{
 const app=await setup();try{
  await app.page.goto(app.url);await app.page.getByLabel('Pairing code').fill(app.code);
  await app.page.getByRole('button',{name:'Pair with this runtime'}).click();
  await app.page.locator('[data-slot="sidebar"]').getByRole('link',{name:'Connections',exact:true}).waitFor();
  await app.page.waitForFunction(()=>{
   const total=document.querySelectorAll('.profile-card').length;
   return total>0
    &&document.querySelectorAll('.metadata-result').length===total
    &&document.querySelector('[data-testid="check-all-profiles"]')?.textContent==='Recheck all profiles';
  },null,{timeout:30000});
  assert.equal(app.errors.length,0,app.errors.join('\n'));
 }finally{await app.close();}
});
test('live selector empty states are useful and stay inside the viewport',async()=>{
 const app=await setup();try{
  await app.page.setViewportSize({width:1440,height:1000});
  await app.page.goto(app.url);await app.page.getByLabel('Pairing code').fill(app.code);
  await app.page.getByRole('button',{name:'Pair with this runtime'}).click();
  await app.page.locator('[data-slot="sidebar"]').getByRole('link',{name:'Connections',exact:true}).waitFor();
  await nav(app.page,'Selector lab');
  await app.page.locator('[data-variant="compare"]').click();
  await app.page.getByText('No routes match the current filters.',{exact:true}).waitFor();
  await app.page.getByText('Check account and model metadata on an authorized connection to load routes.',{exact:true}).waitFor();
  for(const width of [1440,1024,390]){
   await app.page.setViewportSize({width,height:900});
   for(const variant of ['rail','compact','palette','capability','compare','allowance','preset','mobile']){
    await app.page.locator(`[data-variant="${variant}"]`).click();
    assert.equal(await app.page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true,`${variant} has horizontal page overflow at ${width}px`);
   }
  }
  await nav(app.page,'Playground');
  const prompt=app.page.getByLabel('Prompt');
  assert.equal(await prompt.inputValue(),'');
  assert.equal(await prompt.isDisabled(),false);
  await prompt.fill('Draft a scene concept');
  assert.equal(await prompt.inputValue(),'Draft a scene concept');
  assert.equal(await app.page.getByRole('button',{name:'Run live turn'}).isDisabled(),true);
  assert.equal(app.errors.length,0,app.errors.join('\n'));
 }finally{await app.close();}
});
test('pairing is the preferred path; example data is an explicit, labeled choice that leads back to pairing',async()=>{
 const app=await setup();try{
  await app.page.goto(app.url);
  await app.page.getByRole('heading',{name:'Connect your local runtime'}).waitFor();
  assert.equal(await app.page.locator('#mode-banner').count(),0,'nothing is shown before pairing or an explicit choice');
  await app.page.getByRole('button',{name:'Explore example data'}).click();
  await app.page.locator('#mode-banner.synthetic').waitFor();
  assert.match(await app.page.locator('#mode-banner').innerText(),/pair a runtime to see yours/i);
  await app.page.getByRole('button',{name:'Pair a runtime'}).click();
  await app.page.getByRole('heading',{name:'Connect your local runtime'}).waitFor();
  assert.equal(app.errors.length,0,app.errors.join('\n'));
 }finally{await app.close();}
});
test('pairing rejects an incorrect code without revealing accounts',async()=>{
 const app=await setup();try{await app.page.goto(app.url);await app.page.getByLabel('Pairing code').fill('000000000000');await app.page.getByRole('button',{name:'Pair with this runtime'}).click();await app.page.getByRole('alert').waitFor();assert.equal(await app.page.locator('#mode-banner').count(),0);}finally{await app.close();}
});
test('pairing can recover after reload by explicitly issuing a replacement code',async()=>{
 const app=await setup();try{
  await app.page.goto(app.url);await app.page.getByRole('button',{name:'Show current pairing code'}).click();
  await app.page.waitForFunction(()=>document.querySelector('#pairing-code')?.value.length===12,null,{timeout:5000});
  const first=await app.page.getByLabel('Pairing code').inputValue();assert.equal(first,app.code);
  await app.page.getByRole('button',{name:'Pair with this runtime'}).click();await app.page.locator('#mode-banner.live').waitFor();
  await app.page.reload();await app.page.getByRole('heading',{name:'Connect your local runtime'}).waitFor();
  await app.page.getByRole('button',{name:'Show current pairing code'}).click();
  await app.page.waitForFunction(()=>document.querySelector('#pairing-code')?.value.length===12,null,{timeout:5000});
  const replacement=await app.page.getByLabel('Pairing code').inputValue();assert.notEqual(replacement,first);
  await app.page.getByText(/New code issued/i).waitFor();
  await app.page.getByRole('button',{name:'Pair with this runtime'}).click();await app.page.locator('#mode-banner.live').waitFor();
  assert.equal(app.errors.length,0,app.errors.join('\n'));
 }finally{await app.close();}
});
