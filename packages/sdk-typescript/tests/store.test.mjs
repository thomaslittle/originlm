import test from 'node:test';import assert from 'node:assert/strict';
import {createSnapshotStore,LocalAIError} from '../dist/index.js';
const snapshot=()=>({protocolVersion:'1.0',revision:'fixture',mode:'synthetic',observedAt:'2026-09-24T00:00:00Z',providers:[],installations:[],profiles:[],accounts:[],connections:[],routes:[],models:[],quotaPools:[],diagnostics:[]});
function fixture(read=async()=>snapshot()) {const calls={read:0,scan:0,probe:0,close:0};return{calls,client:{snapshot:{get:async()=>{calls.read++;return read();}},discovery:{refresh:async()=>{calls.scan++;}},connections:{probe:async()=>{calls.probe++;}},close:async()=>{calls.close++;}}};}

test('concurrent successful probes share the newest snapshot instead of cancelling each other',async()=>{
 const reads=[];const f=fixture(()=>new Promise(resolve=>reads.push(resolve)));const store=createSnapshotStore(f.client);
 const first=store.probe('first');
 await new Promise(resolve=>setImmediate(resolve));
 const second=store.probe('second');
 await new Promise(resolve=>setImmediate(resolve));
 assert.equal(reads.length,2);
 const results=Promise.all([first,second]);
 reads[0]({...snapshot(),revision:'old'});
 reads[1]({...snapshot(),revision:'new'});
 assert.deepEqual((await results).map(s=>s.revision),['new','new']);
 assert.equal(store.getSnapshot().snapshot.revision,'new');store.dispose();
});
test('construction, subscription and reads do not start discovery or any request',()=>{const f=fixture();const store=createSnapshotStore(f.client);const initial=store.getSnapshot();const off=store.subscribe(()=>{});assert.equal(store.getSnapshot(),initial);assert.equal(store.getServerSnapshot(),initial);off();assert.deepEqual(f.calls,{read:0,scan:0,probe:0,close:0});store.dispose();assert.equal(f.calls.close,0);});
test('concurrent snapshot loads share one public SDK request',async()=>{let finish;const f=fixture(()=>new Promise(r=>finish=r));const store=createSnapshotStore(f.client);const a=store.load(),b=store.load();assert.equal(f.calls.read,1);finish(snapshot());await Promise.all([a,b]);assert.equal(store.getSnapshot().loading,false);assert.equal(store.getSnapshot().snapshot.mode,'synthetic');store.dispose();});
test('only explicit scan and probe calls enrich data',async()=>{const f=fixture();const store=createSnapshotStore(f.client);await store.load();assert.equal(f.calls.scan,0);await store.scan();assert.equal(f.calls.scan,1);await store.probe('chosen');assert.equal(f.calls.probe,1);store.dispose();});
test('a late request cannot repopulate a disposed store',async()=>{let finish;const f=fixture(()=>new Promise(r=>finish=r));const store=createSnapshotStore(f.client);const request=store.load();store.dispose();finish(snapshot());await assert.rejects(request,e=>e.code==='CLOSED');assert.equal(store.getSnapshot().snapshot,null);});
test('authorization failures clear old account data and normalize the error',async()=>{let failing=false;const f=fixture(async()=>{if(failing)throw new LocalAIError('LOCAL_ACCESS_DENIED');return snapshot();});const store=createSnapshotStore(f.client);await store.load();failing=true;await assert.rejects(store.load());assert.equal(store.getSnapshot().snapshot,null);assert.equal(store.getSnapshot().error.code,'LOCAL_ACCESS_DENIED');store.dispose();});
test('the store owns immutable data instead of mutating host input',async()=>{const original=snapshot();const f=fixture(async()=>original);const store=createSnapshotStore(f.client);await store.load();assert.notEqual(store.getSnapshot().snapshot,original);assert.throws(()=>store.getSnapshot().snapshot.models.push({}));assert.equal(original.models.length,0);store.dispose();});

test('store uses paged catalogs when the runtime advertises models.page',async()=>{const base=snapshot();base.routes=[{id:'r',connectionId:'c',providerId:'p',displayName:'R',billing:{state:'unknown',reason:'x'},capabilities:[],catalogRevision:'1'}];let pages=0;const client={handshake:{methods:['models.page']},snapshot:{get:async p=>p?.fields?.length===1?{revision:'fixture'}:{...base,models:undefined}},models:{page:async()=>{pages++;return{revision:'fixture',models:[{id:'m',routeId:'r',nativeId:'n',name:'N',revision:'1',isDefault:true,capabilities:[],options:[],contextWindow:{state:'unknown',reason:'x'}}],nextCursor:null};}},discovery:{refresh:async()=>{}},connections:{probe:async()=>{}}};const store=createSnapshotStore(client);const value=await store.load();assert.equal(pages,1);assert.equal(value.models[0].id,'m');store.dispose();});
