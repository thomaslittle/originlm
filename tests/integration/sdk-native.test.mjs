import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createLocalAI} from '../../packages/sdk-typescript/dist/index.js';
import {openStdioTransport} from '../../packages/sdk-typescript/node/index.mjs';
const executable=process.env.LAI_NATIVE_BINARY;
if(!executable)throw Error('LAI_NATIVE_BINARY must point to the compiled helper; run scripts/test_native_bridge.py');
async function connect(home,allow=true){
 return createLocalAI({transport:await openStdioTransport({executable,args:['serve','--stdio',...(allow?['--allow-passive-discovery']:[]),'--home',home]}),clientName:'node-integration'});
}
test('Node SDK to native Rust service: real scan, unchanged credentials, no fabricated metadata',async()=>{
 const home=await mkdtemp(join(tmpdir(),'originlm-node-'));
 await mkdir(join(home,'.codex'));await writeFile(join(home,'.codex','auth.json'),'PRIVATE-SENTINEL');
 const ai=await connect(home);
 try{
  // This host explicitly authorizes autonomous passive inventory. The first
  // cached read may precede or follow that scan; it must never imply metadata
  // permission or reveal the fixture's credential content in either ordering.
  const initial=await ai.snapshot.get();
  assert.equal(initial.mode,'live');
  assert.ok(initial.connections.every(c=>c.auth.state==='unknown'));
  assert.deepEqual(initial.models,[]);assert.deepEqual(initial.accounts,[]);
  assert.ok(!JSON.stringify(initial).includes('PRIVATE-SENTINEL'));
  await ai.discovery.refresh({level:'passive'});
  const snapshot=await ai.snapshot.get();
  assert.equal(snapshot.mode,'live');assert.ok(snapshot.connections.some(c=>c.providerId==='codex'));
  assert.ok(snapshot.connections.every(c=>c.auth.state==='unknown'));
  assert.deepEqual(snapshot.models,[]);assert.deepEqual(snapshot.accounts,[]);
  assert.ok(!JSON.stringify(snapshot).includes('PRIVATE-SENTINEL'));
  assert.equal(await readFile(join(home,'.codex','auth.json'),'utf8'),'PRIVATE-SENTINEL');
  const replies=await Promise.all(Array.from({length:24},()=>ai.snapshot.get({fields:['mode','connections']})));
  assert.ok(replies.every(r=>r.mode==='live'&&r.connections.length===snapshot.connections.length));
  await ai.grants.revoke(); assert.deepEqual((await ai.snapshot.get()).connections,[]);
  await assert.rejects(ai.discovery.refresh(),e=>e.code==='LOCAL_ACCESS_DENIED');
 }finally{await ai.close();await rm(home,{recursive:true,force:true});}
});
test('Node helper defaults to denied and cannot self-authorize over RPC',async()=>{
 const home=await mkdtemp(join(tmpdir(),'originlm-denied-'));const ai=await connect(home,false);
 try{
  assert.equal((await ai.grants.request({scopes:['discovery']})).status,'requires_host_approval');
  await assert.rejects(ai.discovery.refresh(),e=>e.code==='LOCAL_ACCESS_DENIED');
  await assert.rejects(ai.sessions.create({}),e=>e.code==='CAPABILITY_UNSUPPORTED');
 }finally{await ai.close();await rm(home,{recursive:true,force:true});}
});
test('Node empty-home scan stays empty in live mode',async()=>{
 const home=await mkdtemp(join(tmpdir(),'originlm-empty-'));const ai=await connect(home);
 try{await ai.discovery.refresh();assert.deepEqual((await ai.snapshot.get()).connections,[]);}
 finally{await ai.close();await rm(home,{recursive:true,force:true});}
});
test('Node typed catalog pages are served by the actual native helper',async()=>{
 const home=await mkdtemp(join(tmpdir(),'originlm-paged-'));const ai=await connect(home);
 try{
  assert.ok(ai.handshake.methods.includes('models.page'));
  const before=await ai.snapshot.get({fields:['revision']});
  const page=await ai.models.page({limit:10});
  assert.deepEqual(page.models,[]);assert.equal(page.nextCursor,null);
  const after=await ai.snapshot.get({fields:['revision']});
  // The empty fixture has one possible bootstrap publication. Keep revision
  // validation while permitting that publication between independent reads.
  assert.ok([before.revision,after.revision].includes(page.revision));
  await assert.rejects(ai.models.page({limit:0}),e=>e.code==='INVALID_REQUEST');
 }finally{await ai.close();await rm(home,{recursive:true,force:true});}
});
