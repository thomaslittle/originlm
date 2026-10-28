import type { CallOptions, Transport } from './types.ts';
import { LocalAIError, remoteError, requireMethod } from './errors.ts';
export interface NativePort {
 postMessage(value: unknown):void; disconnect():void;
 onMessage:{addListener(fn:(v:unknown)=>void):void;removeListener(fn:(v:unknown)=>void):void};
 onDisconnect:{addListener(fn:()=>void):void;removeListener(fn:()=>void):void};
}
const failure=(code:string)=>new LocalAIError(code);
class Queue {
 values:unknown[]=[]; error?:LocalAIError; waiter?:{resolve:(v:unknown)=>void;reject:(e:unknown)=>void};
 push(value:unknown){if(this.error)return;if(this.waiter){this.waiter.resolve(value);this.waiter=undefined;}else if(this.values.length>=256)this.fail(failure('EVENT_GAP'));else this.values.push(value);}
 fail(error:LocalAIError){this.error=error;this.values=[];this.waiter?.reject(error);this.waiter=undefined;}
 async next():Promise<unknown>{if(this.error)throw this.error;if(this.values.length)return this.values.shift();return new Promise((resolve,reject)=>{this.waiter={resolve,reject};});}
}
/** Only call in an extension page or service worker, after the user's Connect action. */
export function createNativeMessagingTransport(options:{connect:()=>NativePort}):Transport {
 const port=options.connect();let sequence=0,closed=false;
 const pending=new Map<number,{finish:(error?:LocalAIError,value?:unknown)=>void}>();
 const subscriptions=new Map<string,Queue>();
 function end(error:LocalAIError){if(closed)return;closed=true;for(const call of pending.values())call.finish(error);for(const queue of subscriptions.values())queue.fail(error);subscriptions.clear();port.onMessage.removeListener(message);port.onDisconnect.removeListener(disconnect);try{port.disconnect();}catch{/* Already closed by browser. */}}
 const disconnect=()=>end(failure('TRANSPORT_CLOSED'));
 function message(raw:unknown){
  try{
   if(!raw||typeof raw!=='object'||Array.isArray(raw))throw failure('PROTOCOL_INVALID');
   const v=raw as Record<string,any>;
   if(v.jsonrpc!=='2.0'||new TextEncoder().encode(JSON.stringify(raw)).length>1_048_576)throw failure('PROTOCOL_INVALID');
   if(Object.hasOwn(v,'id')){
    if(!Number.isSafeInteger(v.id)||Object.hasOwn(v,'result')===Object.hasOwn(v,'error'))throw failure('PROTOCOL_INVALID');
    const call=pending.get(v.id);if(!call)return;
    if(v.error){if(typeof v.error.code!=='number')throw failure('PROTOCOL_INVALID');call.finish(remoteError(v.error.data));}
    else call.finish(undefined,v.result);
   }else if(v.method==='events.event'||v.method==='events.error'){
    const p=v.params;if(!p||typeof p.subscriptionId!=='string')throw failure('PROTOCOL_INVALID');
    const queue=subscriptions.get(p.subscriptionId);if(!queue)return;
    if(v.method==='events.error')queue.fail(failure(typeof p.code==='string'?p.code:'EVENT_GAP'));
    else if(Object.hasOwn(p,'event'))queue.push(p.event);else throw failure('PROTOCOL_INVALID');
   }else throw failure('PROTOCOL_INVALID');
  }catch(error){end(error instanceof LocalAIError?error:failure('PROTOCOL_INVALID'));}
 }
 port.onMessage.addListener(message);port.onDisconnect.addListener(disconnect);
 function request<T=unknown>(method:string,params:unknown={},call:CallOptions={},hook?:(v:T)=>void):Promise<T>{
  if(closed)return Promise.reject(failure('CLOSED'));
  if(call.signal?.aborted)return Promise.reject(failure('CANCELLED'));
  try{requireMethod(method);}catch(error){return Promise.reject(error);}
  if(pending.size>=64)return Promise.reject(failure('TOO_MANY_REQUESTS'));
  if(sequence>=Number.MAX_SAFE_INTEGER)return Promise.reject(failure('REQUEST_ID_EXHAUSTED'));
  const duration=call.timeoutMs??30000;
  if(!Number.isSafeInteger(duration)||duration<1||duration>3600000)return Promise.reject(failure('INVALID_LIMIT'));
  const id=++sequence;let body:unknown;
  try{const text=JSON.stringify({jsonrpc:'2.0',id,method,params});if(new TextEncoder().encode(text).length>1_048_576)throw failure('FRAME_TOO_LARGE');body=JSON.parse(text);}catch(e){return Promise.reject(e instanceof LocalAIError?e:failure('INVALID_REQUEST'));}
  return new Promise((resolve,reject)=>{
   const abort=()=>finish(failure('CANCELLED'));let timer:ReturnType<typeof setTimeout>;
   const finish=(error?:LocalAIError,value?:unknown)=>{if(!pending.delete(id))return;clearTimeout(timer);call.signal?.removeEventListener('abort',abort);if(error){reject(error);return;}try{hook?.(value as T);resolve(value as T);}catch(e){reject(e);}};
   pending.set(id,{finish});timer=setTimeout(()=>finish(failure('TIMEOUT')),duration);call.signal?.addEventListener('abort',abort,{once:true});
   try{port.postMessage(body);}catch{finish(failure('TRANSPORT_CLOSED'));}
  });
 }
 return {
  request,
  async *subscribe<T=unknown>(query={},call:CallOptions={}){
   let id='';const queue=new Queue();
   await request<{subscriptionId:string}>('events.subscribe',query,call,result=>{if(typeof result?.subscriptionId!=='string'||!result.subscriptionId||subscriptions.has(result.subscriptionId))throw failure('PROTOCOL_INVALID');id=result.subscriptionId;subscriptions.set(id,queue);});
   const abort=()=>queue.fail(failure('CANCELLED'));if(call.signal?.aborted)abort();else call.signal?.addEventListener('abort',abort,{once:true});
   try{while(true)yield await queue.next() as T;}finally{call.signal?.removeEventListener('abort',abort);subscriptions.delete(id);if(!closed)await request('events.unsubscribe',{subscriptionId:id},{timeoutMs:1000}).catch(()=>{});}
  },
  async close(){end(failure('CLOSED'));},
 };
}
