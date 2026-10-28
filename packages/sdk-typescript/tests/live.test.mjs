import test from 'node:test';
import assert from 'node:assert/strict';
import { createLiveSnapshotStore } from '../dist/index.js';
const snapshot={protocolVersion:'1.0',revision:'1',cursor:'7',mode:'synthetic',observedAt:'2026-10-28T00:00:00Z',providers:[],installations:[],profiles:[],accounts:[],connections:[],routes:[],models:[],quotaPools:[],diagnostics:[]};
const tick=()=>new Promise(resolve=>setImmediate(resolve));
test('clean event EOF backs off and an old cleanup cannot cancel a new connection',async()=>{
 const signals=[];const queries=[];
 const client={snapshot:{get:async()=>snapshot},inventory:{async *subscribe(query,{signal}){signals.push(signal);queries.push(query);}}};
 const store=createLiveSnapshotStore(client);
 try {
  const first=store.connect();await tick();
  assert.equal(signals.length,1);assert.deepEqual(queries[0],{after:'7'});
  first();const second=store.connect();await tick();
  assert.equal(signals.length,2);first();
  assert.equal(signals[1].aborted,false);second();assert.equal(signals[1].aborted,true);
 } finally {store.dispose();}
});
test('disconnect cancels the pending coalesced catalog read',async()=>{
 let reads=0;
 const client={snapshot:{get:async()=>{reads++;return snapshot;}},inventory:{async *subscribe(){yield {changes:[{}],issuesChanged:false};}}};
 const store=createLiveSnapshotStore(client);
 try{const stop=store.connect();await tick();stop();await new Promise(resolve=>setTimeout(resolve,70));assert.equal(reads,1);}finally{store.dispose();}
});
