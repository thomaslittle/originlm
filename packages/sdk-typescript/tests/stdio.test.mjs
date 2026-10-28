import test from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { openStdioTransport } from '../node/index.mjs';
import { createLocalAI } from '../dist/index.js';
const peer=fileURLToPath(new URL('./fixtures/peer.mjs',import.meta.url));
// Cold Node startup competes with the other workspace suites. Deadline behavior
// is tested separately with an explicit short per-request timeout.
const options={executable:process.execPath,args:[peer],timeoutMs:5000};
const open=extra=>openStdioTransport({...options,...extra});
test('explicit local process round trips through the same public client',async()=>{
 const t=await open();try{const ai=await createLocalAI({transport:t});assert.deepEqual(await ai.snapshot.get({fields:['models']}),{fields:['models']});}finally{await t.close();}
});
test('concurrent requests are correlated instead of relying on completion order',async()=>{
 const t=await open();try{const results=await Promise.all([t.request('test.delay',{value:'first',delay:30}),t.request('test.delay',{value:'second',delay:1})]);assert.deepEqual(results,['first','second']);}finally{await t.close();}
});
test('prompts and arbitrary strings are data, not executable shell arguments',async()=>{
 const t=await open();try{const text='$(touch unsafe); " & whoami';assert.deepEqual(await t.request('test.echo',{text}),{text});}finally{await t.close();}
});
test('relative executable or working directory is rejected',async()=>{
 await assert.rejects(open({executable:'node'}),/INVALID_EXECUTABLE/);await assert.rejects(open({cwd:'relative'}),/INVALID_WORKSPACE/);
});
test('inherited NODE_OPTIONS is not injected into the helper',async()=>{
 const previous=process.env.NODE_OPTIONS;process.env.NODE_OPTIONS='--synthetic-invalid-option';let t;
 try{t=await open({env:{EXPLICIT_VALUE:'allowed'}});assert.deepEqual(await t.request('test.env'),{nodeOptions:null,custom:'allowed'});}finally{if(previous===undefined)delete process.env.NODE_OPTIONS;else process.env.NODE_OPTIONS=previous;await t?.close();}
});
test('request timeout does not retry and transport can still answer another request',async()=>{
 const t=await open();try{await t.request('test.echo',{ready:true});await assert.rejects(t.request('test.hang',{}, {timeoutMs:30}),/TIMEOUT/);assert.deepEqual(await t.request('test.echo',{ok:true}),{ok:true});}finally{await t.close();}
});
test('request AbortSignal rejects without replay or fallback',async()=>{
 const t=await open();try{const c=new AbortController();const pending=t.request('test.hang',{}, {signal:c.signal});c.abort();await assert.rejects(pending,/CANCELLED/);}finally{await t.close();}
});
test('malformed peer output fails the transport closed',async()=>{
 const t=await open();try{await assert.rejects(t.request('test.bad'),/PROTOCOL_INVALID/);await assert.rejects(t.request('test.echo'),/CLOSED/);}finally{await t.close();}
});
test('oversized unterminated stdout is bounded',async()=>{
 const t=await open({maxFrameBytes:1024});try{await assert.rejects(t.request('test.large'),/FRAME_TOO_LARGE/);}finally{await t.close();}
});
test('peer exit rejects in-flight operations',async()=>{
 const t=await open();try{await assert.rejects(t.request('test.exit'),/TRANSPORT_CLOSED/);}finally{await t.close();}
});
test('events in same stdout chunk as subscribe response are not lost',async()=>{
 const t=await open();try{const events=[];for await(const e of t.subscribe()){events.push(e);if(events.length===3)break;}assert.deepEqual(events.map(e=>e.sequence),['1','2','3']);}finally{await t.close();}
});
test('event consumer overflow fails explicitly instead of dropping output',async()=>{
 const t=await open({maxQueuedEvents:2});try{const iterator=t.subscribe({after:'manual'})[Symbol.asyncIterator]();await iterator.next();await t.request('test.flood');await assert.rejects(iterator.next(),/EVENT_GAP/);}finally{await t.close();}
});
test('close is idempotent and does not leave pending requests',async()=>{
 const t=await open();const request=t.request('test.hang');const check=assert.rejects(request,/CLOSED/);await t.close();await check;await t.close();await assert.rejects(t.request('test.echo'),/CLOSED/);
});
test('stderr is drained but not returned or logged',async()=>{
 const t=await open();try{assert.deepEqual(await t.request('test.stderr'),{ok:true});assert.equal(JSON.stringify(t).includes('SECRET'),false);}finally{await t.close();}
});

test('server event gap fails only its subscription and preserves request transport',async()=>{
 const t=await open();try{const iterator=t.subscribe({after:'manual'})[Symbol.asyncIterator]();await iterator.next();assert.deepEqual(await t.request('test.gap'),{ok:true});await assert.rejects(iterator.next(),/EVENT_GAP/);assert.deepEqual(await t.request('test.echo',{alive:true}),{alive:true});}finally{await t.close();}
});
test('stdio carries nested auth method identifiers without relaxing malformed names',async()=>{
 const t=await open();try{assert.deepEqual(await t.request('auth.chatgpt.status',{synthetic:true}),{synthetic:true});for(const method of ['auth..status','.auth.status','auth.status.'])await assert.rejects(t.request(method),/INVALID_METHOD/);}finally{await t.close();}
});

test('standard OS data directories survive the minimal child environment',async()=>{
 const keys=['XDG_STATE_HOME','APPDATA','LOCALAPPDATA','LOCAL_AI_KIT_STATE_DIR','ORIGINLM_STATE_DIR'];
 const previous=Object.fromEntries(keys.map(key=>[key,process.env[key]]));
 const expected=Object.fromEntries(keys.map(key=>[key,'synthetic-state-directory']));
 Object.assign(process.env,expected);let transport;
 try{transport=await open();assert.deepEqual(await transport.request('test.directories'),expected);}
 finally{await transport?.close();for(const key of keys){if(previous[key]===undefined)delete process.env[key];else process.env[key]=previous[key];}}
});

test('invalid native executable bytes produce a safe typed startup error',async()=>{
 await assert.rejects(open({executable:process.execPath+'\0SHOULD_NOT_ESCAPE'}),error=>error.name==='LocalAIError'&&error.code==='INVALID_EXECUTABLE'&&!error.message.includes('SHOULD_NOT_ESCAPE'));
});
