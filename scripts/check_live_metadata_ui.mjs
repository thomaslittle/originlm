import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {chromium} from '@playwright/test';

/** Sidebar navigation. Routes load lazily, so wait until the new page (not its skeleton) is on screen. */
async function nav(page,name){
 const link=page.locator('[data-slot="sidebar"]').getByRole('link',{name,exact:true});
 // Below the md breakpoint the sidebar is an off-canvas sheet; open it first.
 if(!(await link.count()))await page.getByRole('button',{name:'Toggle Sidebar'}).click();
 await link.click();
 await page.waitForFunction(title=>document.querySelector('#page-title')?.textContent===title&&!document.querySelector('[data-slot="skeleton"]'),name);
}

if(!process.argv.includes('--allow-metadata'))throw Error('Explicit --allow-metadata is required; no generation is ever requested.');
const root=resolve(import.meta.dirname,'..');
const child=spawn(resolve(root,'target/debug',process.platform==='win32'?'originlm.exe':'originlm'),['serve','--http','--port','0','--assets',resolve(root,'apps/demo/dist'),'--allow-passive-discovery','--allow-codex-metadata'],{stdio:['ignore','pipe','pipe'],windowsHide:true});
let browser;
try{
 let output='';const host=await new Promise((res,rej)=>{const timer=setTimeout(()=>rej(Error('HOST_TIMEOUT')),10000);child.on('error',rej);child.stdout.on('data',b=>{output+=b;const url=output.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0],code=output.match(/minutes\): ([0-9a-f]{12})/)?.[1];if(url&&code){clearTimeout(timer);res({url,code});}});child.stderr.on('data',()=>{});});
 browser=await chromium.launch({headless:true});const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(host.url);await page.getByLabel('Pairing code').fill(host.code);await page.getByRole('button',{name:'Pair with this runtime'}).click();await page.locator('#mode-banner.live').waitFor();
 await page.getByRole('button',{name:'Scan local clients'}).click();
 const card=page.locator('section.card').filter({has:page.getByRole('heading',{name:/Codex/})}).first();await card.waitFor();
 const probe=page.waitForResponse(r=>r.url().endsWith('/v1/rpc')&&r.request().postDataJSON()?.method==='connections.probe',{timeout:30000});
 await card.getByRole('button',{name:'Check account and models'}).click();const result=await (await probe).json();if(result.error)throw Error(result.error.data.code);
 await nav(page,'Models');await page.locator('table [data-testid]').first().click();await page.locator('#selection-json').waitFor();
 if(errors.length)throw Error(errors.join(';'));
 console.log(JSON.stringify({nativeBrowserPairing:true,nativeAccountProbe:result.result.status,liveModelSelector:true,providerPromptsSubmitted:0,browserErrors:errors.length}));
 await page.screenshot({path:resolve(root,'.verification/live-native-selector.png'),fullPage:true});
}finally{await browser?.close();child.kill();}
