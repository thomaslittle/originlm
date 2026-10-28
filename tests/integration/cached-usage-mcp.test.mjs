/** One bounded stdio MCP journey; synthetic cache, no broker/provider connection. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {writeFile} from 'node:fs/promises';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

async function journey(empty = false, noScope = false, revoked = false) {
 const entry = fileURLToPath(new URL('../../examples/mcp/cached-usage-demo.mjs', import.meta.url));
 const child = spawn(process.execPath, ['--experimental-strip-types', entry, ...(empty ? ['--unavailable'] : []), ...(noScope ? ['--no-scope'] : []), ...(revoked ? ['--revoked'] : [])], {
  stdio:['pipe','pipe','pipe'], windowsHide:true,env:{...process.env,LAI_MCP_VERIFY_READS:'1'},
 });
 let stdout='', stderr='';
 const timer=setTimeout(()=>child.kill(),10_000);
 child.stdout.on('data',data=>{stdout+=data;if(stdout.length>524288)child.kill();});
 child.stderr.on('data',data=>{stderr+=data;if(stderr.length>16384)child.kill();});
 const requests=[
  {id:1,method:'tools/call',params:{name:'kit_usage_cached',arguments:{}}},
  {id:2,method:'initialize',params:{protocolVersion:'2025-06-18',capabilities:{},clientInfo:{name:'synthetic-host',version:'1'}}},
  {method:'notifications/initialized'},
  {id:3,method:'tools/list',params:{}},
  {id:4,method:'tools/call',params:{name:'kit_usage_cached',arguments:{}}},
  {id:5,method:'tools/call',params:{name:'kit_usage_cached',arguments:{}}},
  {id:6,method:'tools/call',params:{name:'kit_usage_cached',arguments:{refresh:true}}},
  {id:7,method:'connections.probe',params:{}},
 ];
 const closed=new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',code=>resolve(code));});
 child.stdin.end(requests.map(request=>JSON.stringify({jsonrpc:'2.0',...request})).join('\n')+'\n');
 const code=await closed;clearTimeout(timer);
 assert.equal(code,0,stderr);
 const packets=stdout.trim().split('\n').map(line=>JSON.parse(line));
 assert.equal(packets.length,7);
 const byId=new Map(packets.map(packet=>[packet.id,packet]));
 assert.equal(byId.get(1).error.code,-32000);
 assert.equal(byId.get(2).result.protocolVersion,'2025-06-18');
 const tools=byId.get(3).result.tools;
 assert.equal(tools.length,1);assert.equal(tools[0].name,'kit_usage_cached');
 assert.equal(tools[0].annotations.readOnlyHint,true);
 assert.equal(byId.get(6).error.code,-32602);
 assert.equal(byId.get(7).error.code,-32601);
 const report=byId.get(4).result.structuredContent;
 assert.deepEqual(byId.get(5).result.structuredContent,report);
 for(const secret of ['PRIVATE-EMAIL','PRIVATE-WINDOW','PRIVATE-REFERENCE','PRIVATE-REASON','PRIVATE-MAPPING','account-outside-scope'])
  assert.ok(!stdout.includes(secret),secret);
 if(empty||revoked){assert.equal(report.state,'unavailable');assert.deepEqual(report.quotaPools,[]);assert.deepEqual(report.mappings,[]);return;}
 if(noScope){assert.deepEqual(report.quotaPools,[]);assert.deepEqual(report.connections,[]);
  assert.equal(report.mappings[0].state,'unknown');assert.ok(!Object.hasOwn(report.mappings[0],'kitAccountId'));return;}
 assert.equal(report.mode,'synthetic');assert.equal(report.state,'cached');
 assert.equal(report.quotaPools.length,2);
 const windows=report.quotaPools.flatMap(pool=>pool.windows);
 const known=windows.find(window=>window.reading.state==='known');
 assert.equal(known.reading.value.used,'95');assert.equal(known.reading.value.remaining,'5');
 assert.equal(known.reading.freshness,'stale');
 assert.equal(known.resetsAt,'2020-01-01T00:00:00Z','expired reset invented a recovery');
 assert.ok(windows.some(window=>window.reading.state==='unknown'));
 assert.ok(windows.some(window=>window.reading.state==='unavailable'&&window.reading.reason==='CAPABILITY_UNSUPPORTED'));
 assert.equal(report.mappings[0].state,'explicit');
 assert.deepEqual(report.unmappedAccountIds,['account-claude']);
 assert.equal(report.contextCounters,'not_reported');
 if(process.env.LAI_MCP_EVIDENCE)await writeFile(process.env.LAI_MCP_EVIDENCE,JSON.stringify(report,null,2)+'\n');
}

test('cached usage MCP endpoint: scoped/redacted windows, stale resets, unknowns and no provider work', {timeout:25000}, async()=>{
 await journey();await journey(true);await journey(false,true);await journey(false,false,true);
});
