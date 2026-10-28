import test from 'node:test';
import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import { selectionKey,createCatalog,resolveCandidates,validateOption,reconcileSelection,groupCandidates,visibleQuotaPools } from '../src/index.ts';
const known=value=>({state:'known',value,freshness:'fresh'});
const unknown={state:'unknown',reason:'NOT_REPORTED'};
const cap=(id,support='supported')=>({id,support,origin:'model',conditions:[],evidence:known('fixture')});
function snapshot() {
 const connections=['personal','work','metered','offline'].map(id=>({id,providerId:id==='offline'?'local':'vendor',displayName:id,auth:known('authenticated')}));
 const routes=connections.map(c=>({id:`route:${c.id}`,connectionId:c.id,providerId:c.providerId,displayName:c.displayName,billing:known(c.id==='metered'?'api_metered':c.id==='offline'?'local':'subscription'),catalogRevision:'1',capabilities:[]}));
 const models=routes.map(r=>({id:`model:${r.id}`,routeId:r.id,nativeId:'same-model',name:'Same Model',revision:'1',capabilities:[cap('generation.text')],options:[]}));
 return {connections,routes,models,quotaPools:[]};
}
const select={id:'effort',label:'Effort',scope:'turn',revision:'1',control:{type:'select',choices:[{value:'cosmic',label:'Cosmic'},{value:'blocked',label:'Blocked',disabledReason:'No entitlement'}],defaultValue:'cosmic'}};

test('accountless local runtimes require matching providers and fresh local billing',()=>{
 for(const provider of ['ollama','lmstudio']){
  const s=snapshot();s.connections[0].providerId=provider;s.routes[0].providerId=provider;
  s.connections[0].auth={state:'unknown',reason:'LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH'};s.routes[0].billing=known('local');
  assert.equal(resolveCandidates(createCatalog(s))[0].eligible,true);
  s.routes[0].billing.freshness='stale';
  assert.equal(resolveCandidates(createCatalog(s))[0].eligible,false);
 }
 for(const provider of ['vendor','ollama']){
  const s=snapshot();s.connections[0].providerId=provider;
  s.connections[0].auth={state:'unknown',reason:'LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH'};s.routes[0].billing=known('local');
  assert.equal(resolveCandidates(createCatalog(s))[0].eligible,false);
 }
});
test('route identity and delimiter collisions stay distinct',()=>{
 assert.notEqual(selectionKey('a:b','c'),selectionKey('a','b:c'));
 assert.notEqual(selectionKey('personal','same'),selectionKey('work','same'));
});
test('four routes to same native model remain four candidates',()=>{
 const c=resolveCandidates(createCatalog(snapshot()),{}); assert.equal(c.length,4); assert.equal(new Set(c.map(x=>x.key)).size,4);
});
test('authenticated subscription can be eligible without promising free use',()=>{
 const c=resolveCandidates(createCatalog(snapshot()),{}).find(x=>x.route.connectionId==='personal');
 assert.equal(c.eligible,true); assert.equal(c.billingLabel,'Included allowance');
});
test('API billing requires consent; local and included routes do not inherit it',()=>{
 const c=resolveCandidates(createCatalog(snapshot()),{});
 assert.ok(c.find(x=>x.route.connectionId==='metered').reasons.includes('BILLING_CONSENT_REQUIRED'));
 assert.equal(c.find(x=>x.route.connectionId==='offline').eligible,true);
 const allowed=resolveCandidates(createCatalog(snapshot()),{allowPaidFallback:true});
 assert.equal(allowed.find(x=>x.route.connectionId==='metered').eligible,true);
});
test('unknown billing is not treated as included allowance',()=>{
 const s=snapshot();s.routes[0].billing=unknown;
 assert.ok(resolveCandidates(createCatalog(s),{}).find(x=>x.route.connectionId==='personal').reasons.includes('BILLING_UNKNOWN'));
});
test('expired authentication dominates stale catalogs',()=>{
 const s=snapshot();s.connections[0].auth=known('expired');
 const c=resolveCandidates(createCatalog(s),{}).find(x=>x.route.connectionId==='personal');
 assert.equal(c.eligible,false);assert.ok(c.reasons.includes('AUTH_REQUIRED'));
});
test('stale authentication does not authorize execution',()=>{
 const s=snapshot();s.connections[0].auth={...known('authenticated'),freshness:'stale'};
 assert.ok(resolveCandidates(createCatalog(s),{})[0].reasons.includes('AUTH_UNVERIFIED'));
});
test('unknown required capability does not satisfy filter',()=>{
 const s=snapshot();s.models[0].capabilities=[cap('generation.text','unknown')];
 const c=resolveCandidates(createCatalog(s),{requiredCapabilities:['generation.text']});
 assert.ok(c.find(x=>x.model.id===s.models[0].id).reasons.includes('CAPABILITY_UNKNOWN:generation.text'));
});
test('conditional capability is not unconditionally eligible',()=>{
 const s=snapshot();s.models[0].capabilities=[cap('tools','conditional')];
 const c=resolveCandidates(createCatalog(s),{requiredCapabilities:['tools']});
 assert.ok(c.find(x=>x.model.id===s.models[0].id).reasons.includes('CAPABILITY_CONDITIONAL:tools'));
});
test('unknown capability evidence cannot become confirmed support',()=>{
 const s=snapshot();s.models[0].capabilities[0].evidence=unknown;
 assert.equal(resolveCandidates(createCatalog(s),{requiredCapabilities:['generation.text']}).find(x=>x.model.id===s.models[0].id).eligible,false);
});
test('search indexes native ID, provider and connection',()=>{
 assert.equal(resolveCandidates(createCatalog(snapshot()),{query:'work same-model'}).length,1);
 assert.equal(resolveCandidates(createCatalog(snapshot()),{query:'not-a-model'}).length,0);
});
test('provider grouping preserves all access routes',()=>{
 const groups=groupCandidates(resolveCandidates(createCatalog(snapshot()),{}));
 assert.equal(groups.find(x=>x.providerId==='vendor').candidates.length,3);
});
test('missing route or connection is excluded and diagnosed by the index',()=>{
 const s=snapshot();s.routes.splice(0,1);s.connections=s.connections.filter(c=>c.id!=='work');
 const catalog=createCatalog(s);assert.equal(catalog.models.size,2);assert.equal(catalog.diagnostics.length,2);
});
test('duplicate identifiers fail rather than silently replace billing identities',()=>{
 const s=snapshot();s.models.push({...s.models[0]}); assert.throws(()=>createCatalog(s),/duplicate/i);
});
test('native future effort is allowed and guessed effort is not',()=>{
 assert.equal(validateOption(select,'cosmic'),null);assert.match(validateOption(select,'high'),/OPTION_UNSUPPORTED/);
 assert.match(validateOption(select,'blocked'),/OPTION_UNSUPPORTED/);
});
test('numeric options enforce step, range, finite numbers and type',()=>{
 const d={...select,control:{type:'number',min:1,max:10,step:2}};
 assert.equal(validateOption(d,3),null);
 for(const value of [2,0,11,NaN,Infinity,'3',true]) assert.notEqual(validateOption(d,value),null);
});
test('boolean, text and unknown option types are handled without guessing',()=>{
 assert.equal(validateOption({...select,control:{type:'boolean'}},false),null);
 assert.notEqual(validateOption({...select,control:{type:'boolean'}},'false'),null);
 assert.equal(validateOption({...select,control:{type:'text',maxLength:2}},'🌎a'),null);
 assert.notEqual(validateOption({...select,control:{type:'text',maxLength:2}},'abc'),null);
 assert.notEqual(validateOption({...select,control:{type:'unknown',nativeType:'future'}},'x'),null);
});
test('changing model clears unsupported saved options visibly, without selecting defaults',()=>{
 const s=snapshot();const selected={routeId:s.routes[0].id,modelId:s.models[0].id,revision:'0',options:{effort:'high'}};
 const result=reconcileSelection(createCatalog(s),selected);
 assert.deepEqual(result.selection.options,{});assert.deepEqual(result.removedOptions,['effort']);assert.equal(result.needsReview,true);
});
test('model disappearance returns no selection rather than routing elsewhere',()=>{
 const s=snapshot();const result=reconcileSelection(createCatalog(s),{routeId:'gone',modelId:'gone',revision:'1',options:{}});
 assert.equal(result.selection,null); assert.equal(result.reason,'MODEL_UNAVAILABLE');
});
test('valid selections retain exact route, native options and revision',()=>{
 const s=snapshot();s.models[0].options=[select]; const selection={routeId:s.routes[0].id,modelId:s.models[0].id,revision:'1',options:{effort:'cosmic'}};
 const result=reconcileSelection(createCatalog(s),selection);assert.deepEqual(result.selection,selection);assert.equal(result.needsReview,false);
});
test('route/model mismatch cannot pick a different account',()=>{
 const s=snapshot();const result=reconcileSelection(createCatalog(s),{routeId:s.routes[1].id,modelId:s.models[0].id,revision:'1',options:{}});
 assert.equal(result.selection,null);assert.equal(result.reason,'ROUTE_MISMATCH');
});
test('selection does not mutate frozen catalogs or saved options',()=>{
 const s=snapshot();s.models.forEach(m=>Object.freeze(m));Object.freeze(s.models);const selected=Object.freeze({routeId:s.routes[0].id,modelId:s.models[0].id,revision:'1',options:Object.freeze({})});
 reconcileSelection(createCatalog(s),selected);assert.equal(s.models.length,4);
});
test('shared pool appears once without combining unknown allowances',()=>{
 const pools=[{id:'pool',routeIds:['a','b'],sharing:'verified',windows:[]},{id:'unknown1',routeIds:['a'],sharing:'unknown',windows:[]},{id:'unknown2',routeIds:['b'],sharing:'unknown',windows:[]}];
 assert.equal(visibleQuotaPools(pools,['a','b']).length,3);assert.equal(visibleQuotaPools(pools,['b']).length,2);
});
test('5000 models are indexed and queried without per-row catalog scans',()=>{
 const s=snapshot();s.models=Array.from({length:5000},(_,i)=>({...s.models[i%4],id:`m${i}`,nativeId:`native-${i}`,name:`Model ${i}`}));
 const start=performance.now();const catalog=createCatalog(s);assert.equal(resolveCandidates(catalog,{query:'native-4999'}).length,1);
 const elapsed=performance.now()-start;console.log(`5000-model index + search: ${elapsed.toFixed(2)}ms`);assert.ok(elapsed<1000);
});

test('explicit backend denial of included usage is not overridden by paid fallback or elapsed reset time',()=>{
 const s=snapshot();s.routes[0].capabilities.push({...cap('usage.included_allowed','unsupported'),origin:'runtime'});
 for(const paid of [false,true]){const choice=resolveCandidates(createCatalog(s),{allowPaidFallback:paid}).find(c=>c.connection.id==='personal');assert.equal(choice.eligible,false);assert.ok(choice.reasons.includes('RATE_LIMITED'));}
 s.routes[0].capabilities[0].evidence.freshness='stale';assert.equal(resolveCandidates(createCatalog(s)).find(c=>c.connection.id==='personal').eligible,false);
 s.routes[0].capabilities[0].support='supported';assert.equal(resolveCandidates(createCatalog(s)).find(c=>c.connection.id==='personal').eligible,true);
});

test('exact account phrases outrank incidental substrings without collapsing routes',()=>{
 const s=snapshot();
 s.connections[0].displayName='Personal workspace';
 s.connections[1].displayName='Work workspace';
 const c=resolveCandidates(createCatalog(s),{query:'Work workspace'});
 assert.equal(c[0].connection.id,'work');
 assert.ok(c.some(x=>x.connection.id==='personal'));
 assert.notEqual(c[0].key,c.find(x=>x.connection.id==='personal').key);
});
test('search ranking remains stable for equal matches and empty searches',()=>{
 const s=snapshot();
 const c=createCatalog(s);
 assert.deepEqual(resolveCandidates(c).map(x=>x.connection.id),s.connections.map(x=>x.id));
 assert.deepEqual(resolveCandidates(c,{query:'same model'}).map(x=>x.connection.id),s.connections.map(x=>x.id));
});
