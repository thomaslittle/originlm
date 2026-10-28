import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createLocalAI} from '../dist/index.js';
import {readCatalogSnapshot} from '../dist/catalog.js';
const header={protocolVersion:'1.0',revision:'r',observedAt:'2026-09-24T00:00:00Z',cursor:'4',health:{},issues:[],mode:'live',providers:[],installations:[],profiles:[],accounts:[],connections:[],routes:[{id:'route'}],quotaPools:[],diagnostics:[]};
async function client(mode='normal'){
 let calls=[];
 const ai=await createLocalAI({transport:{async request(method,p){calls.push({method,p});
  if(method==='runtime.handshake')return{protocolVersion:'1.0',methods:['snapshot.get',...(mode==='legacy'?[]:['models.page'])]};
  if(method==='snapshot.get')return (mode==='legacy'||!p?.fields)?{...header,models:[]}:(p.fields.length===1?{revision:'r'}:header);
  if(method==='models.page')return{revision:mode==='changed'?'new':'r',models:[{id:p.cursor==='next'?'m2':'m1',routeId:'route'}],nextCursor:mode==='loop'?'next':p.cursor?null:'next'};
 },async close(){}}});return{ai,calls};
}
test('construction never loads a catalog and paginated reads preserve all models',async()=>{
 const{ai,calls}=await client();assert.equal(calls.length,1);
 const snapshot=await readCatalogSnapshot(ai);assert.deepEqual(snapshot.models.map(m=>m.id),['m1','m2']);
 assert.equal(snapshot.cursor,'4');
 for(const field of ['cursor','health','issues']) assert.ok(calls.find(c=>c.method==='snapshot.get').p.fields.includes(field));
 assert.ok(calls.filter(c=>c.method==='snapshot.get').every(c=>!c.p.fields.includes('models')));
});
test('changed catalog is rejected rather than combining different account snapshots',async()=>{
 const{ai}=await client('changed');await assert.rejects(readCatalogSnapshot(ai),e=>e.code==='SELECTION_STALE');
});
test('repeated cursors or model IDs cannot cause endless reads',async()=>{
 const{ai,calls}=await client('loop');await assert.rejects(readCatalogSnapshot(ai),e=>e.code==='PROTOCOL_INVALID');assert.ok(calls.length<8);
});
test('legacy runtimes retain the existing explicit snapshot path',async()=>{
 const{ai}=await client('legacy');assert.deepEqual((await readCatalogSnapshot(ai)).models,[]);
});
test('host model limits are enforced without returning truncated successful data',async()=>{
 const{ai}=await client();await assert.rejects(readCatalogSnapshot(ai,{maxModels:1}),e=>e.code==='FRAME_TOO_LARGE');
});
