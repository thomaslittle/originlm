/** One inert source check: synthetic SQLite and in-memory Requests; never opens a listener. */
import assert from 'node:assert/strict';
import {DatabaseSync} from 'node:sqlite';
import {mkdtemp,readFile,unlink,rmdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {createSqliteUsageCache} from './owner-cache.mjs';
import {createCachedUsageRemoteMcp} from '../../../packages/sdk-typescript/src/cached-usage-remote.ts';
import {createCachedUsageReader} from '../../../packages/sdk-typescript/src/cached-usage-mcp.ts';

const directory=await mkdtemp(join(tmpdir(),'kit-usage-inert-'));
const path=join(directory,'synthetic.db');
try {
 const at='2020-01-01T00:00:00Z';
 const snapshot={protocolVersion:'1.0',mode:'synthetic',revision:'remote-fixture-1',observedAt:at,
  providers:[],installations:[],profiles:[],models:[],diagnostics:[],accounts:[{id:'account-one'},{id:'account-private'}],
  connections:[{id:'connection-one',accountId:'account-one',providerId:'codex',displayName:'PRIVATE-ACCOUNT'}],
  routes:[],quotaPools:[{id:'pool-one',accountId:'account-one',connectionIds:['connection-one'],routeIds:[],sharing:'unknown',windows:[
   {id:'five-hour',label:'PRIVATE-LABEL',durationSeconds:18000,resetsAt:at,reading:{state:'known',freshness:'stale',observedAt:at,verification:'reported',
    source:{kind:'synthetic',adapterId:'remote-fixture',adapterVersion:'1.0.0',reference:'PRIVATE-REFERENCE'},
    value:{unit:'percent',used:'95',remaining:'5',limit:'100',usedFraction:0.95}}},
   {id:'weekly',label:'PRIVATE-LABEL',durationSeconds:604800,reading:{state:'unknown',reason:'PRIVATE-REASON'}},
  ]}]};
 const db=new DatabaseSync(path);db.exec('CREATE TABLE snapshot(singleton INTEGER PRIMARY KEY,payload TEXT)');
 db.prepare('INSERT INTO snapshot VALUES(1,?)').run(JSON.stringify(snapshot));db.close();
 const hash=async()=>createHash('sha256').update(await readFile(path)).digest('hex');const before=await hash();
 assert.throws(()=>createSqliteUsageCache({databasePath:path}),/OWNER_CACHE_ACCESS_NOT_APPROVED/);
 const cache=createSqliteUsageCache({databasePath:path,ownerApprovedCacheRead:true});let reads=0;
 const read=createCachedUsageReader({store:{getSnapshot(){reads++;return cache.getSnapshot();}},
  scope:{accountIds:['account-one'],connectionIds:['connection-one']},mappings:[]});
 const route=createCachedUsageRemoteMcp({source:{read},
  publicOrigin:'https://usage.example.invalid',path:'/mcp/kit-usage',principalId:'fixture-principal',
  authenticate:async request=>request.headers.get('x-fixture-proof')==='yes'
   ? {subject:'fixture-principal',scopes:['kit.usage.cached.read']}:undefined});
 assert.equal(reads,0);
 const request=(packet,proof=true,extra={})=>new Request('https://usage.example.invalid/mcp/kit-usage',{
  method:'POST',headers:{'content-type':'application/json','accept':'application/json, text/event-stream',
   ...(proof?{'x-fixture-proof':'yes'}:{}),...extra},body:JSON.stringify({jsonrpc:'2.0',id:1,...packet})});
 assert.equal((await route.handle(request({method:'tools/call',params:{name:'kit_usage_cached',arguments:{}}},false))).status,401);
 assert.equal(reads,0);
 const init=await route.handle(request({method:'initialize',params:{protocolVersion:'2025-06-18',capabilities:{},clientInfo:{name:'inert-check',version:'1'}}}));
 assert.equal((await init.json()).result.protocolVersion,'2025-06-18');
 const list=await route.handle(request({method:'tools/list'}));assert.equal((await list.json()).result.tools[0].name,'kit_usage_cached');assert.equal(reads,0);
 const answer=await route.handle(request({method:'tools/call',params:{name:'kit_usage_cached',arguments:{}}}));
 assert.equal(answer.headers.get('cache-control'),'no-store');const report=(await answer.json()).result.structuredContent;
 assert.equal(report.mode,'synthetic');assert.equal(report.quotaPools.length,1);assert.equal(report.mappings.length,0);
 assert.deepEqual(report.unmappedAccountIds,['account-one']);assert.equal(report.contextCounters,'not_reported');
 const window=report.quotaPools[0].windows[0];assert.equal(window.reading.value.used,'95');assert.equal(window.reading.value.remaining,'5');
 assert.equal(window.reading.freshness,'stale');assert.equal(window.resetsAt,at);assert.equal(report.quotaPools[0].windows[1].reading.state,'unknown');
 assert.ok(!JSON.stringify(report).includes('PRIVATE-'));assert.equal(reads,1);
 const rejected=await route.handle(request({method:'tools/call',params:{name:'kit_usage_cached',arguments:{refresh:true}}}));
 assert.equal((await rejected.json()).error.code,-32602);assert.equal(reads,1);
 assert.equal((await route.handle(request({method:'tools/list'},true,{origin:'https://other.example.invalid'}))).status,403);
 // Revoke while an authenticated request is still uploading its body.
 let bodyController,bodyRequested;
 const readingBody=new Promise(resolve=>{bodyRequested=resolve;});
 const body=new ReadableStream({start(controller){bodyController=controller;},pull(){bodyRequested();}},{highWaterMark:0});
 const pending=route.handle(new Request('https://usage.example.invalid/mcp/kit-usage',{
  method:'POST',headers:{'content-type':'application/json',accept:'application/json','x-fixture-proof':'yes'},body,duplex:'half'}));
 await readingBody;route.revoke();
 bodyController.enqueue(new TextEncoder().encode(JSON.stringify({jsonrpc:'2.0',id:2,method:'tools/call',params:{name:'kit_usage_cached',arguments:{}}})));
 bodyController.close();
 assert.equal((await pending).status,403,'revocation during upload must deny the pending request');
 assert.equal(reads,1,'revoked upload must not read the cache');
 route.revoke();assert.equal((await route.handle(request({method:'tools/list'}))).status,403);assert.equal(reads,1);
 assert.equal(await hash(),before,'read-only cache must not change database bytes');
 console.log(JSON.stringify({result:'PASS',mode:'synthetic',cacheReads:reads,providerRequests:0,listeners:0,liveCredentialsOrGrants:0,
  checked:['inert construction','auth-before-cache','initialize/list inert','scoped stale/unknown usage','explicitly unmapped account','no reset inference','no refresh','origin check','revocation','revocation during upload','database bytes unchanged']}));
} finally {await unlink(path);await rmdir(directory);}
