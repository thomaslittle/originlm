/** One synthetic producer -> authenticated-caller binding lifecycle; no live host. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {createSnapshotStore} from '../../packages/sdk-typescript/src/store.ts';
import {createCachedUsageProducer, createT3CachedUsageTool} from '../../packages/sdk-typescript/src/cached-usage-host.ts';

test('existing cache updates reach only the approved T3 caller without extra client work', {timeout:5000}, async()=>{
 const at='2026-10-24T00:00:00Z'; let now=Date.parse(at);
 const unknown={state:'unknown',reason:'PRIVATE-REASON',checkedAt:at};
 const reading={state:'known',freshness:'fresh',observedAt:at,verification:'reported',
  source:{kind:'synthetic',adapterId:'host-fixture',adapterVersion:'1.0.0',reference:'PRIVATE-REFERENCE'},
  value:{unit:'requests',used:'95',limit:'100',remaining:'5',usedFraction:0.95}};
 const snapshot={protocolVersion:'1.0',mode:'synthetic',revision:'fixture-1',observedAt:at,
  providers:[],installations:[],profiles:[],models:[],diagnostics:[],
  accounts:[{id:'account-one'},{id:'account-hidden'}],
  connections:[{id:'connection-one',accountId:'account-one',providerId:'codex',displayName:'PRIVATE-EMAIL'},
   {id:'connection-hidden',accountId:'account-hidden',providerId:'claude'}],
  routes:[{id:'route-one',connectionId:'connection-one'}],quotaPools:[
   {id:'pool-one',accountId:'account-one',connectionIds:['connection-one'],routeIds:['route-one'],sharing:'unknown',windows:[
    {id:'window-one',label:'PRIVATE-LABEL',reading,resetsAt:at},
    {id:'window-unknown',label:'PRIVATE-LABEL',reading:unknown},
    {id:'window-unsupported',label:'PRIVATE-LABEL',reading:{state:'unavailable',reason:'CAPABILITY_UNSUPPORTED'}}]},
   {id:'pool-hidden',accountId:'account-hidden',connectionIds:['connection-hidden'],routeIds:[],sharing:'unknown',windows:[]}]};
 let next=snapshot, requests=0, cacheReads=0, subscriptions=0, unsubscriptions=0, late;
 const forbidden=()=>{throw new Error('Provider/refresh work is forbidden');};
 const store=createSnapshotStore({snapshot:{get:async()=>{requests++;return next;}},
  discovery:{refresh:forbidden},connections:{probe:forbidden},close:forbidden},snapshot);
 const accountIds=['account-one'], connectionIds=['connection-one'], sessions=['approved-client'];
 const producer=createCachedUsageProducer({store:{
  getSnapshot(){cacheReads++;return store.getSnapshot();},
  subscribe(listener){subscriptions++;late=listener;const off=store.subscribe(listener);return()=>{unsubscriptions++;off();};},
 },scope:{accountIds,connectionIds},mappings:[{consumerAccountId:'operator-confirmed-billing-account',kitAccountId:'account-one'}],now:()=>now});
 let toolReads=0;
 const tool=createT3CachedUsageTool({source:{read(){toolReads++;return producer.read();}},
  environmentId:'approved-environment',clientSessionIds:sessions});
 const caller={environmentId:'approved-environment',thread:undefined,client:{sessionId:'approved-client'}};
 assert.equal(cacheReads,0);assert.equal(subscriptions,0);assert.equal(requests,0);
 assert.equal(tool.read(caller,{}).state,'unavailable');
 for(const other of [undefined, {...caller,environmentId:'other'}, {...caller,client:{sessionId:'other'}},
  {...caller,thread:{threadId:'agent-thread'}}, {...caller,client:undefined}]){
  assert.throws(()=>tool.read(other,{}),/CACHED_USAGE_ACCESS_DENIED/);
 }
 assert.equal(toolReads,1,'denied callers must not reach the producer');
 sessions.push('other');accountIds.push('account-hidden');connectionIds.push('connection-hidden');
 assert.throws(()=>tool.read({...caller,client:{sessionId:'other'}},{}),/CACHED_USAGE_ACCESS_DENIED/);
 producer.attach();producer.attach();
 assert.equal(subscriptions,1);assert.equal(cacheReads,1);assert.equal(requests,0);
 const first=tool.read(caller,{});
 assert.equal(first.quotaPools.length,1);assert.equal(first.quotaPools[0].windows[0].reading.value.remaining,'5');
 assert.equal(first.mappings[0].state,'explicit');
 assert.ok(first.quotaPools[0].windows.some(w=>w.reading.state==='unknown'));
 assert.ok(first.quotaPools[0].windows.some(w=>w.reading.reason==='CAPABILITY_UNSUPPORTED'));
 assert.ok(!JSON.stringify(first).includes('PRIVATE-'));assert.ok(!JSON.stringify(first).includes('account-hidden'));
 first.quotaPools[0].windows[0].reading.value.remaining='999';
 assert.equal(tool.read(caller,{}).quotaPools[0].windows[0].reading.value.remaining,'5');
 assert.equal(cacheReads,1,'tool reads must use the published projection only');
 assert.throws(()=>tool.read(caller,{refresh:true}),/INVALID_CACHED_USAGE_ARGUMENTS/);
 now+=120000;
 const aged=tool.read(caller,{}).quotaPools[0].windows[0];
 assert.equal(aged.reading.freshness,'stale');assert.equal(aged.reading.value.remaining,'5');assert.equal(aged.resetsAt,at);
 now=Date.parse(at);
 assert.equal(tool.read(caller,{}).quotaPools[0].windows[0].reading.freshness,'stale','a wall-clock rewind cannot promote a published stale reading');
 next=structuredClone(snapshot);next.revision='fixture-2';next.quotaPools[0].windows[0].reading.value.remaining='4';
 await store.load(); // Existing owner publishes one synthetic cache update; adapter does not load.
 assert.equal(requests,1);assert.equal(tool.read(caller,{}).revision,'fixture-2');
 const before=cacheReads;tool.read(caller,{});assert.equal(cacheReads,before);assert.equal(requests,1);
 producer.stop();assert.equal(unsubscriptions,1);late();assert.equal(producer.read().state,'unavailable');
 producer.attach();assert.equal(subscriptions,2);
 store.dispose();assert.equal(tool.read(caller,{}).state,'unavailable');
 assert.deepEqual(tool.read(caller,{}).quotaPools,[]);
 producer.dispose();producer.attach();late();assert.equal(subscriptions,2);assert.equal(producer.read().state,'unavailable');
 assert.equal(unsubscriptions,2);
 tool.revoke();assert.throws(()=>tool.read(caller,{}),/CACHED_USAGE_ACCESS_DENIED/);
 assert.equal(requests,1);assert.equal(tool.definition.name,'kit_usage_cached');
});
