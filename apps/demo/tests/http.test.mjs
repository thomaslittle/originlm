import test from 'node:test';
import assert from 'node:assert/strict';
import {request as httpRequest} from 'node:http';
import {existsSync} from 'node:fs';
import {mkdir,writeFile,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createFixtureServer} from '../server.mjs';
import {createLocalAI,createHttpTransport} from '../../../packages/sdk-typescript/dist/index.js';
const DIST=fileURLToPath(new URL('../dist/',import.meta.url)),BUILT=existsSync(DIST+'index.html');
async function server(t){const s=await createFixtureServer();t.after(()=>s.close());return s;}
async function auth(s){return (await(await fetch(s.url+'/fixture/bootstrap')).json()).sessionToken;}
test('fixture server listens on loopback and requires explicit authenticated RPC',async t=>{
 const s=await server(t);assert.equal(new URL(s.url).hostname,'127.0.0.1');const r=await fetch(s.url+'/v1/rpc',{method:'POST',headers:{'content-type':'application/json'},body:'{}'});assert.equal(r.status,401);
});
test('actual HTTP SDK talks to fixture service, never fallback account data',async t=>{
 const s=await server(t),ai=await createLocalAI({transport:createHttpTransport({baseUrl:s.url,token:await auth(s)})});t.after(()=>ai.close());const snapshot=await ai.snapshot.get();assert.equal(snapshot.mode,'synthetic');assert.ok(snapshot.connections.length>=5);
});
test('host and cross-origin requests are rejected even with valid token',async t=>{
 const s=await server(t),token=await auth(s);for(const extra of [{Origin:'https://evil.example'},{Host:'evil.example'}]){
 const status=await new Promise((resolve,reject)=>{const req=httpRequest(s.url+'/v1/rpc',{method:'POST',headers:{'content-type':'application/json',authorization:'Bearer '+token,...extra}},res=>{res.resume();resolve(res.statusCode);});req.on('error',reject);req.end('{}');});assert.equal(status,403);
 }
});
test('unknown static paths do not expose arbitrary repository files',async t=>{
 const s=await server(t);for(const p of ['/SOURCE-AUDIT.md','/../.git/config','/modules/sdk/../../package.json','/data/auth.json'])assert.equal((await fetch(s.url+p)).status,404);
});
test('HTTP errors and diagnostics never echo authentication headers',async t=>{
 const s=await server(t),token=await auth(s);const r=await fetch(s.url+'/v1/rpc',{method:'POST',headers:{'content-type':'application/json',authorization:'Bearer '+token},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'bad.method',params:{secret:token}})});
 assert.equal((await r.text()).includes(token),false);
});
test('body limits and invalid JSON reject without fixture actions',async t=>{
 const s=await server(t),token=await auth(s);for(const [body,code] of [['x'.repeat(1048577),413],['invalid',400]]){
 const r=await fetch(s.url+'/v1/rpc',{method:'POST',headers:{'content-type':'application/json',authorization:'Bearer '+token},body});assert.equal(r.status,code);
 }
});
test('SSE authorization is required and the SDK receives actual turn events',async t=>{
 const s=await server(t);assert.equal((await fetch(s.url+'/v1/events')).status,401);
 const token=await auth(s),ai=await createLocalAI({transport:createHttpTransport({baseUrl:s.url,token})});t.after(()=>ai.close());const snap=await ai.snapshot.get();const model=snap.models[0],route=snap.routes.find(r=>r.id===model.routeId);
 await ai.grants.request({connectionIds:[route.connectionId],scopes:['execution']});const session=await ai.sessions.create({selection:{routeId:route.id,modelId:model.id,revision:model.revision,options:{}}});
 const abort=new AbortController();const events=[];const collect=(async()=>{for await(const e of ai.events.subscribe({sessionId:session.id,after:'0'},{signal:abort.signal})){events.push(e);if(e.payload.type==='final')break;}})();
 await ai.turns.start({sessionId:session.id,prompt:'synthetic'});await collect;assert.ok(events.some(e=>e.payload.type==='text_delta'));assert.equal(events.at(-1).payload.status,'completed');
});
test('exact client routes serve the SPA entry and nothing else falls back to it',{skip:!BUILT&&'apps/demo/dist is not built (run npm run build:demo)'},async t=>{
 const s=await server(t),index=await fetch(s.url+'/');assert.equal(index.status,200);const html=await index.text();
 for(const p of ['/overview','/connections','/models','/usage','/selector-lab','/playground','/diagnostics']){const r=await fetch(s.url+p);assert.equal(r.status,200,p);assert.match(r.headers.get('content-type'),/^text\/html/);assert.equal(await r.text(),html,p);}
 for(const p of ['/connections/','/nope','/Connections','/connections/extra','/overview.txt'])assert.equal((await fetch(s.url+p)).status,404,p);
});
test('missing static assets report a build hint',async t=>{
 const s=await server(t),r=await fetch(s.url+'/assets/definitely-missing-file.js');assert.equal(r.status,404);const body=await r.json();assert.equal(body.error,'BUILD_ASSET_MISSING');assert.match(body.hint,/build:demo/);
});
test('self-hosted woff2 fonts are served with the font mime type',{skip:!BUILT&&'apps/demo/dist is not built (run npm run build:demo)'},async t=>{
 const dir=DIST+'assets/',file=dir+'originlm-test-font.woff2';await mkdir(dir,{recursive:true});await writeFile(file,Buffer.from('wOF2test'));t.after(()=>rm(file,{force:true}));
 const s=await server(t),r=await fetch(s.url+'/assets/originlm-test-font.woff2');assert.equal(r.status,200);assert.equal(r.headers.get('content-type'),'font/woff2');assert.equal(Buffer.from(await r.arrayBuffer()).toString(),'wOF2test');
});

test('fixture server announces itself as the synthetic runtime so the plain URL boots the showcase',async()=>{
 const server=await createFixtureServer();
 try{
  const response=await fetch(server.url+'/runtime/info');
  assert.equal(response.status,200);
  assert.deepEqual(await response.json(),{mode:'synthetic',pairingRequired:false});
 }finally{await server.close();}
});
