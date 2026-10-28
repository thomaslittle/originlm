import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {once} from 'node:events';
import {mkdtemp,rm} from 'node:fs/promises';
import {join,resolve} from 'node:path';import {tmpdir} from 'node:os';
import {createLocalAI} from '../../packages/sdk-typescript/dist/index.js';
import * as nodeSDK from '../../packages/sdk-typescript/node/index.mjs';
const binary=process.env.LAI_NATIVE_BINARY??resolve('target/debug/'+(process.platform==='win32'?'originlm.exe':'originlm'));
async function setup(){const home=await mkdtemp(join(tmpdir(),'originlm-broker-sdk-'));const address=process.platform==='win32'?'originlm-'+crypto.randomUUID():join(home,'broker.sock');const p=spawn(binary,['serve','--ipc',address,'--home',home,'--allow-passive-discovery'],{stdio:['pipe','pipe','pipe'],windowsHide:true});p.stderr.on('data',()=>{});const lines=createInterface({input:p.stdout});const iterator=lines[Symbol.asyncIterator]();async function key(){const v=await Promise.race([iterator.next(),new Promise((_,reject)=>setTimeout(()=>reject(Error('BROKER_TIMEOUT')),3000).unref())]);return JSON.parse(v.value).token;}const first=await key();return{address,first,async next(){p.stdin.write('{"action":"authorize"}\n');return key();},async close(){const exit=once(p,'exit');p.stdin.end();await exit;lines.close();await rm(home,{recursive:true,force:true});}};}
test('Node SDK shared broker uses independent credentials and one host survives client close',async()=>{const b=await setup();let a,c;try{assert.equal(typeof nodeSDK.openBrokerTransport,'function');a=await createLocalAI({transport:await nodeSDK.openBrokerTransport({executable:binary,address:b.address,token:b.first})});c=await createLocalAI({transport:await nodeSDK.openBrokerTransport({executable:binary,address:b.address,token:await b.next()})});await a.discovery.refresh();await a.close();a=null;assert.equal((await c.runtime.health()).status,'ok');assert.equal((await c.snapshot.get()).mode,'live');await assert.rejects(()=>nodeSDK.openBrokerTransport({executable:binary,address:b.address,token:b.first}),e=>e.code==='LOCAL_ACCESS_DENIED');}finally{await a?.close();await c?.close();await b.close();}});
