/** Node-only transport. Import does not spawn, scan, read credentials, or connect. */
import { isAbsolute } from 'node:path';
import {readGrant} from './grant.mjs';
import {processOptions, spawnHelper} from './process.mjs';
import { LocalAIError, remoteError, requireMethod } from '../dist/errors.js';
const failCode=code=>new LocalAIError(code);
const bounded=(v,min,max)=>{if(!Number.isSafeInteger(v)||v<min||v>max)throw failCode('INVALID_LIMIT');return v;};
class EventQueue {
 #values=[]; #waiter; #error; #max;
 constructor(max){this.#max=max;}
 push(value){
  if(this.#error)return;
  if(this.#waiter){const w=this.#waiter;this.#waiter=undefined;w.resolve(value);return;}
  if(this.#values.length>=this.#max){this.fail(failCode('EVENT_GAP'));return;}
  this.#values.push(value);
 }
 fail(error){this.#error=error;this.#values=[];this.#waiter?.reject(error);this.#waiter=undefined;}
 next(){
  if(this.#error)return Promise.reject(this.#error);
  if(this.#values.length)return Promise.resolve(this.#values.shift());
  return new Promise((resolve,reject)=>{this.#waiter={resolve,reject};});
 }
}
/** Explicitly launch one approved native helper, without shell interpolation. */
export async function openStdioTransport(options){
 if(typeof options.executable!=='string'||!isAbsolute(options.executable)||options.executable.includes('\0')||/\.(cmd|bat|ps1)$/i.test(options.executable))throw failCode('INVALID_EXECUTABLE');
 const args=options.args??[];
 if(!Array.isArray(args)||args.length>128||args.some(a=>typeof a!=='string'||a.includes('\0')||a.length>32768))throw failCode('INVALID_REQUEST');
 const max=bounded(options.maxFrameBytes??8388608,1024,16777216);
 const maxPending=bounded(options.maxPendingRequests??64,1,1024);
 const maxEvents=bounded(options.maxQueuedEvents??256,1,4096);
 const timeoutMs=bounded(options.timeoutMs??30000,1,3600000);
 const child=spawnHelper(options.executable,[...args],{...processOptions(options),shell:false,windowsHide:true,stdio:['pipe','pipe','pipe']});
 const pending=new Map(),subscriptions=new Map();let sequence=0,buffer=Buffer.alloc(0),ended=false,closing;
 let resolveExit;const exited=new Promise(resolve=>{resolveExit=resolve;});
 function fail(error){
  if(ended)return;ended=true;
  for(const request of pending.values())request.finish(error);
  for(const queue of subscriptions.values())queue.fail(error);
  subscriptions.clear();
  if(child.exitCode===null&&!child.killed)child.kill();
 }
 child.stderr.on('data',()=>{});
 child.stdin.on('error',()=>fail(failCode('TRANSPORT_CLOSED')));
 child.once('exit',()=>{resolveExit();fail(failCode('TRANSPORT_CLOSED'));});
 child.once('error',()=>{resolveExit();fail(failCode('RUNTIME_MISSING'));});
 function frame(line){
  let value;
  try{value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(line));}catch{throw failCode('PROTOCOL_INVALID');}
  if(!value||typeof value!=='object'||value.jsonrpc!=='2.0')throw failCode('PROTOCOL_INVALID');
  if(Object.hasOwn(value,'id')){
   if(!Number.isSafeInteger(value.id)||Object.hasOwn(value,'result')===Object.hasOwn(value,'error'))throw failCode('PROTOCOL_INVALID');
   const request=pending.get(value.id);if(!request)return;
   if(Object.hasOwn(value,'error')){
    if(!value.error||typeof value.error.code!=='number')throw failCode('PROTOCOL_INVALID');
    request.finish(remoteError(value.error.data));
   }else request.finish(null,value.result);
  }else if(value.method==='events.error'){
   const p=value.params;
   if(!p||typeof p.subscriptionId!=='string'||p.subscriptionId.length>256||typeof p.code!=='string'||!/^[_A-Z]{1,64}$/.test(p.code))throw failCode('PROTOCOL_INVALID');
   subscriptions.get(p.subscriptionId)?.fail(failCode(p.code));
  }else if(value.method==='events.event'){
   const p=value.params;
   if(!p||typeof p.subscriptionId!=='string'||p.subscriptionId.length>256||!Object.hasOwn(p,'event'))throw failCode('PROTOCOL_INVALID');
   subscriptions.get(p.subscriptionId)?.push(p.event);
  }else throw failCode('PROTOCOL_INVALID');
 }
 child.stdout.on('data',chunk=>{
  if(ended)return;
  try{
   buffer=Buffer.concat([buffer,chunk]);let position;
   while((position=buffer.indexOf(10))!==-1){
    if(position>max)throw failCode('FRAME_TOO_LARGE');
    const line=buffer.subarray(0,position);buffer=buffer.subarray(position+1);
    if(line.length)frame(line);
   }
   if(buffer.length>max)throw failCode('FRAME_TOO_LARGE');
  }catch(error){fail(error instanceof LocalAIError?error:failCode('PROTOCOL_INVALID'));}
 });
 await new Promise((resolve,reject)=>{child.once('spawn',resolve);child.once('error',()=>reject(failCode('RUNTIME_MISSING')));});
 function request(method,params={},call={},onResult){
  if(ended)return Promise.reject(failCode('CLOSED'));
  if(call.signal?.aborted)return Promise.reject(failCode('CANCELLED'));
  try{requireMethod(method);}catch(error){return Promise.reject(error);}
  if(pending.size>=maxPending)return Promise.reject(failCode('TOO_MANY_REQUESTS'));
  if(sequence>=Number.MAX_SAFE_INTEGER)return Promise.reject(failCode('REQUEST_ID_EXHAUSTED'));
  let body,duration;const id=++sequence;
  try{body=JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n';duration=bounded(call.timeoutMs??timeoutMs,1,3600000);}catch(error){return Promise.reject(error instanceof LocalAIError?error:failCode('INVALID_REQUEST'));}
  if(Buffer.byteLength(body)>max)return Promise.reject(failCode('FRAME_TOO_LARGE'));
  return new Promise((resolve,reject)=>{
   let timer;
   const abort=()=>finish(failCode('CANCELLED'));
   const finish=(error,value)=>{
    if(!pending.has(id))return;
    pending.delete(id);clearTimeout(timer);call.signal?.removeEventListener('abort',abort);
    if(error){reject(error);return;}
    try{onResult?.(value);resolve(value);}catch(e){reject(e);}
   };
   pending.set(id,{finish});
   timer=setTimeout(()=>finish(failCode('TIMEOUT')),duration);
   call.signal?.addEventListener('abort',abort,{once:true});
   child.stdin.write(body,error=>{if(error)finish(failCode('TRANSPORT_CLOSED'));});
  });
 }
 return {
  request,
  async *subscribe(query={},call={}){
   let id,queue;
   await request('events.subscribe',query,call,result=>{
    if(!result||typeof result.subscriptionId!=='string'||!result.subscriptionId||result.subscriptionId.length>256||subscriptions.has(result.subscriptionId))throw failCode('PROTOCOL_INVALID');
    id=result.subscriptionId;queue=new EventQueue(maxEvents);subscriptions.set(id,queue);
   });
   const abort=()=>queue.fail(failCode('CANCELLED'));
   if(call.signal?.aborted)abort();else call.signal?.addEventListener('abort',abort,{once:true});
   try{while(true)yield await queue.next();}
   finally{
    call.signal?.removeEventListener('abort',abort);subscriptions.delete(id);
    if(!ended)await request('events.unsubscribe',{subscriptionId:id},{timeoutMs:1000}).catch(()=>{});
   }
  },
  async close(){
   if(closing)return closing;
   closing=(async()=>{
    fail(failCode('CLOSED'));
    const timer=setTimeout(()=>{if(child.exitCode===null)child.kill('SIGKILL');},1000);
    await exited;clearTimeout(timer);
   })();return closing;
  },
 };
}

export async function openBrokerTransport(options){
 if(typeof options.address!=='string'||!options.address||options.address.length>200||options.address.includes('\0')||typeof options.token!=='string'||!/^[0-9a-f]{64}$/.test(options.token))throw failCode('INVALID_REQUEST');
 const transport=await openStdioTransport({...options,args:['connect','--ipc',options.address]});
 try{const result=await transport.request('broker.authenticate',{token:options.token});if(result?.authenticated!==true)throw failCode('LOCAL_ACCESS_DENIED');return transport;}
 catch(error){await transport.close();throw error;}
}

function sleep(ms){return new Promise(resolve=>setTimeout(resolve,ms));}
function validTokenFile(value,address){
 return value&&typeof value==='object'&&value.protocolVersion==='1.0'&&value.address===address&&typeof value.token==='string'&&/^[0-9a-f]{64}$/.test(value.token);
}
/**
 * Connect to an independent per-user broker, starting it only if no live
 * instance is reachable. The broker owns its lifetime; closing this transport
 * does not stop another application's work. Returns the same Transport shape as
 * openBrokerTransport and never starts a separate full scanner.
 */
export async function openSharedBrokerTransport(options){
 if(typeof options.executable!=='string'||!isAbsolute(options.executable)||options.executable.includes('\0')||/\.(cmd|bat|ps1)$/i.test(options.executable))throw failCode('INVALID_EXECUTABLE');
 if(typeof options.address!=='string'||!options.address||options.address.length>200||options.address.includes('\0'))throw failCode('INVALID_REQUEST');
 if(typeof options.tokenFile!=='string'||!isAbsolute(options.tokenFile)||options.tokenFile.includes('\0'))throw failCode('INVALID_REQUEST');
 const startArgs=options.startArgs??[];
 if(!Array.isArray(startArgs)||startArgs.length>64||startArgs.some(arg=>typeof arg!=='string'||arg.includes('\0')||arg.length>32768))throw failCode('INVALID_REQUEST');
 const processConfig=processOptions(options);
 const waitMs=bounded(options.waitMs??8000,1,60000);
 const attempt=async()=>{
  const parsed=readGrant(options.tokenFile,options.address);
  if(!validTokenFile(parsed,options.address))return null;
  try{return await openBrokerTransport({...options,address:options.address,token:parsed.token});}
  catch(error){
   if(error instanceof LocalAIError&&['RUNTIME_MISSING','TRANSPORT_CLOSED'].includes(error.code))return null;
   throw error;
  }
 };
 const existing=await attempt();
 if(existing)return existing;
 const child=spawnHelper(options.executable,['serve','--ipc',options.address,'--independent','--token-file',options.tokenFile,...startArgs],{
  ...processConfig,shell:false,windowsHide:true,stdio:['pipe','pipe','pipe'],
 });
 child.stderr.on('data',()=>{});
 child.stdin.on('error',()=>{});
 child.once('error',()=>{});
 child.unref();
 const deadline=Date.now()+waitMs;
 try {
 let opened=null;
 while(Date.now()<deadline){
  opened=await attempt();
  if(opened)break;
  await sleep(100);
 }
 if(opened){
  // The broker is independent: releasing our pipes must not terminate it.
  try{child.stdout.destroy();child.stderr.destroy();child.stdin.end();}catch{}
  return opened;
 }
 try{child.kill();}catch{}
 throw failCode('RUNTIME_MISSING');
 } catch(error) {
  try{child.kill();}catch{}
  throw error;
 }
}

export {resolveBundledBinary} from './bundle.mjs';
/** Explicitly verify and launch an extracted local release bundle. No download occurs. */
export async function openBundledTransport({bundleDirectory,...options}) {
 const {resolveBundledBinary}=await import('./bundle.mjs');
 const executable=await resolveBundledBinary(bundleDirectory);
 return openStdioTransport({...options,executable});
}
