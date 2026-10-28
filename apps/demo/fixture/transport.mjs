/** In-memory fixture transport for deterministic browser tests. No native access. */
import {LocalAIError} from '../../../packages/sdk-typescript/dist/index.js';
export function fixtureTransport(service){
 let closed=false;const cancellations=new Set();
 return {
  async request(method,params={},options={}){
   if(closed)throw new LocalAIError('CLOSED');if(options.signal?.aborted)throw new LocalAIError('CANCELLED');
   try{return service.handle(method,params);}catch(error){throw new LocalAIError(error.code??'INVALID_REQUEST');}
  },
  async *subscribe(query={},options={}){
   if(closed)throw new LocalAIError('CLOSED');const values=[];let pending=null,failure=null;
   const stop=code=>{failure=new LocalAIError(code);pending?.();pending=null;};
   const listener=event=>{if(!query.sessionId||query.sessionId===event.sessionId){if(values.length>=256)stop('EVENT_GAP');else values.push(event);pending?.();pending=null;}};
   const remove=service.subscribe(listener),cancel=()=>stop('CLOSED'),abort=()=>stop('CANCELLED');cancellations.add(cancel);
   options.signal?.addEventListener('abort',abort,{once:true});if(options.signal?.aborted)abort();
   try{
    for(const e of service.replay(query.after))listener(e);
    while(!closed){if(failure)throw failure;if(values.length){yield values.shift();continue;}await new Promise(resolve=>{pending=resolve;});}
    throw new LocalAIError('CLOSED');
   }finally{remove();cancellations.delete(cancel);options.signal?.removeEventListener('abort',abort);}
  },
  async close(){if(!closed){closed=true;for(const cancel of cancellations)cancel();service.close();}},
 };
}
