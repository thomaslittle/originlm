import {createLocalAI,createNativeMessagingTransport,type LocalAIClient,type Snapshot,type Selection,type Event as AIEvent} from '../../packages/sdk-typescript/src/index.ts';
declare const chrome:any;
const $=<T extends HTMLElement>(id:string)=>document.getElementById(id) as T;
let client:LocalAIClient|null=null,snapshot:Snapshot|null=null,session:string|null=null,turn:string|null=null,events:AbortController|null=null;
const button=(id:string)=>$<HTMLButtonElement>(id),status=(text:string)=>{$('status').textContent=text;};
async function action(fn:()=>Promise<void>){try{await fn();}catch(error){status((error as any)?.code??'Operation failed; no substitute account was selected');}}
function selection():Selection{const model=snapshot?.models.find(m=>m.id===$<HTMLSelectElement>('model').value);if(!model)throw Error('Choose an available model');return {routeId:model.routeId,modelId:model.id,revision:model.revision,options:{}};}
function render(){
 $('connections').replaceChildren();for(const connection of snapshot?.connections??[]){const card=document.createElement('div');card.className='connection';const heading=document.createElement('h3');heading.textContent=connection.displayName;const text=document.createElement('p');text.textContent=connection.auth.state==='known'?connection.auth.value:connection.auth.reason;const probe=document.createElement('button');probe.textContent='Inspect account and models';probe.onclick=()=>void action(async()=>{await client!.connections.probe({connectionId:connection.id});snapshot=await client!.snapshot.get();render();});card.append(heading,text,probe);$('connections').append(card);}
 const list=$<HTMLSelectElement>('model');list.replaceChildren();for(const m of snapshot?.models??[]){const opt=document.createElement('option');opt.value=m.id;opt.textContent=m.name+' — '+(snapshot?.routes.find(r=>r.id===m.routeId)?.displayName??m.routeId);list.append(opt);}list.onchange=()=>{$('details').textContent=JSON.stringify(snapshot?.models.find(m=>m.id===list.value),null,2);};list.dispatchEvent(new Event('change'));button('run').disabled=!client?.handshake.methods.includes('turns.start');
}
button('connect').onclick=()=>void action(async()=>{
 const port=chrome.runtime.connectNative('com.originlm.host');port.onDisconnect.addListener(()=>{void chrome.runtime.lastError;status('Native host disconnected');});
 client=await createLocalAI({transport:createNativeMessagingTransport({connect:()=>port})});snapshot=await client.snapshot.get();button('connect').disabled=true;button('scan').disabled=false;button('disconnect').disabled=false;render();status('Connected. Scan and metadata checks are explicit.');
});
button('scan').onclick=()=>void action(async()=>{await client!.discovery.refresh({level:'passive'});snapshot=await client!.snapshot.get();render();});
button('disconnect').onclick=()=>void action(async()=>{events?.abort();await client?.close();client=null;snapshot=null;render();button('connect').disabled=false;button('scan').disabled=true;status('Disconnected');});
button('run').onclick=()=>void action(async()=>{
 if(!$<HTMLInputElement>('consent').checked){status('Approve this request before running it');return;}
 const ai=client!;if(session)await ai.sessions.close({sessionId:session});
 const s=await ai.sessions.create({selection:selection(),workspaceId:'default',permissionMode:'read_only',allowPaid:false});session=s.id;events?.abort();events=new AbortController();$('output').textContent='';
 void(async()=>{try{for await(const e of ai.events.subscribe<AIEvent>({sessionId:s.id,after:'0'},{signal:events!.signal})){if(e.payload.type==='text_delta')$('output').textContent+=e.payload.text;if(e.payload.type==='approval_request')await ai.approvals.respond({approvalId:e.payload.approvalId,allow:false});if(e.payload.type==='final'){button('cancel').disabled=true;status('Turn '+e.payload.status);break;}}}catch(e){if(!events?.signal.aborted)status((e as any)?.code??'Event stream ended');}})();
 const t=await ai.turns.start({sessionId:s.id,prompt:$<HTMLTextAreaElement>('prompt').value,requestId:crypto.randomUUID()});turn=t.turnId;button('cancel').disabled=false;
});
button('cancel').onclick=()=>void action(async()=>{if(turn)await client!.turns.cancel({turnId:turn});});
