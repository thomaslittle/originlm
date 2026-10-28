/** Test/demo harness only, not a substitute for the Rust service or provider adapters. */
import {fixtureCosts} from './costs.mjs';
import {fixtureSnapshot} from './snapshot.mjs';
import {createCatalog,resolveCandidates,reconcileSelection} from '../../../packages/selector-core/dist/index.js';
export function createFixtureService({snapshot:provided,turnError=null}={}){
 const snapshot=provided?structuredClone(provided):fixtureSnapshot();
 if(snapshot.mode!=='synthetic')throw Error('SYNTHETIC_MODE_REQUIRED');
 const costs=fixtureCosts(snapshot);
 const catalog=createCatalog(snapshot),sessions=new Map(),turns=new Map(),grants=new Map(),approvals=new Map(),listeners=new Set(),history=[];
 let counter=0,sequence=0,refreshes=0,closed=false;
 const error=code=>{const e=new Error(code);e.code=code;throw e;};
 const id=prefix=>`${prefix}-${++counter}`;
 const clone=value=>structuredClone(value);
 function emit(session,payload){
  const event={sessionId:session.id,turnId:session.turnId??null,sequence:String(++sequence),timestamp:new Date().toISOString(),adapterId:'synthetic',schemaVersion:'1.0',payload};
  history.push(event);if(history.length>256)history.shift();for(const listener of listeners)listener(clone(event));
 }
 function getSession(sessionId){const session=sessions.get(sessionId);if(!session)error('NOT_FOUND');return session;}
 function validate(selection,allowPaid=false){
  if(!selection||typeof selection!=='object'||!selection.options||typeof selection.options!=='object'||Array.isArray(selection.options))error('INVALID_REQUEST');
  const result=reconcileSelection(catalog,selection);
  if(!result.selection)error(result.reason==='ROUTE_MISMATCH'?'SESSION_ROUTE_MISMATCH':'NOT_FOUND');
  if(result.removedOptions.length)error('OPTION_UNSUPPORTED');
  if(result.needsReview)error('SELECTION_STALE');
  const candidate=resolveCandidates(catalog,{allowPaidFallback:allowPaid,requiredCapabilities:['generation.text']}).find(c=>c.model.id===selection.modelId);
  if(!candidate?.eligible)error(candidate?.reasons[0]?.split(':')[0]??'CAPABILITY_UNKNOWN');
  return candidate;
 }
 function authorized(connectionId,allowPaid=false){return [...grants.values()].some(g=>g.connectionIds.includes(connectionId)&&g.scopes.includes('execution')&&(!allowPaid||g.allowPaid));}
 function complete(turn,status){
  clearTimeout(turn.timer);turn.done=true;
  const session=getSession(turn.sessionId);session.state=status==='cancelled'?'cancelled':'completed';
  emit(session,{type:'state',state:session.state});emit(session,{type:'final',status});
 }
 function stream(turn){
  if(closed||turn.done)return;
  const session=getSession(turn.sessionId);session.state='running';
  const chunks=['This is a synthetic response. ','It traveled through the public SDK and local event stream. ','No AI provider was called and no allowance was consumed.'];
  let index=0;
  const next=()=>{if(closed||turn.done)return;if(index<chunks.length){emit(session,{type:'text_delta',text:chunks[index++]});turn.timer=setTimeout(next,80);}else complete(turn,'completed');};
  turn.timer=setTimeout(next,0);
 }
 const handlers={
  'runtime.handshake':()=>({protocolVersion:'1.0',runtimeVersion:'0.1.0-alpha.1-fixture',methods:Object.keys(handlers)}),
  'runtime.health':()=>({status:'ok',mode:'synthetic'}),
  'snapshot.get':p=>{if(p.fields){if(!Array.isArray(p.fields)||p.fields.some(f=>!Object.hasOwn(snapshot,f)))error('INVALID_REQUEST');return Object.fromEntries(p.fields.map(f=>[f,snapshot[f]]));}return snapshot;},
  'connections.list':()=>snapshot.connections,'models.list':p=>snapshot.models.filter(m=>!p.routeId||m.routeId===p.routeId),
  'costs.read':()=>costs.read(), 'costs.setPrices':p=>costs.set(p.prices),
  'usage.read':()=>snapshot.quotaPools,'capabilities.read':p=>snapshot.models.find(m=>m.id===p.modelId)?.capabilities??[],
  'adapters.list':()=>snapshot.providers.map(p=>({id:p.id,classification:'synthetic',liveQualified:false})),
  'discovery.refresh':p=>{if(p.level&&p.level!=='passive')error('CAPABILITY_UNSUPPORTED');snapshot.revision=`fixture-${++refreshes+1}`;return {scanId:id('fixture-scan'),mode:'synthetic',revision:snapshot.revision};},
  'selection.resolve':p=>resolveCandidates(catalog,p).map(c=>({selection:{routeId:c.route.id,modelId:c.model.id,revision:c.model.revision,options:{}},eligible:c.eligible,reasons:c.reasons})),
  'selection.validate':p=>{validate(p.selection,p.allowPaid===true);return {valid:true,mode:'synthetic'};},
  'grants.request':p=>{
   if(!Array.isArray(p.connectionIds)||!Array.isArray(p.scopes)||p.connectionIds.some(id=>!snapshot.connections.some(c=>c.id===id))||p.scopes.some(s=>!['execution','metadata'].includes(s)))error('INVALID_REQUEST');
   if(grants.size>=32)error('RESOURCE_LIMIT');const grant={id:id('grant'),connectionIds:p.connectionIds,scopes:p.scopes,allowPaid:p.allowPaid===true,mode:'synthetic'};grants.set(grant.id,grant);return grant;
  },
  'grants.list':()=>[...grants.values()],
  'grants.revoke':p=>{const grant=grants.get(p.grantId);if(!grant)error('NOT_FOUND');grants.delete(grant.id);for(const turn of turns.values()){const s=getSession(turn.sessionId);if(!turn.done&&grant.connectionIds.includes(s.connectionId))complete(turn,'cancelled');}return {revoked:true};},
  'sessions.create':p=>{
   const candidate=validate(p.selection,p.allowPaid===true);if(!authorized(candidate.connection.id,p.allowPaid===true))error('LOCAL_ACCESS_DENIED');if(sessions.size>=32)error('RESOURCE_LIMIT');
   const session={id:id('session'),connectionId:candidate.connection.id,selection:clone(p.selection),state:'ready',nativeResumption:false,createdAt:new Date().toISOString(),allowPaid:p.allowPaid===true};sessions.set(session.id,session);return session;
  },
  'sessions.get':p=>getSession(p.sessionId),
  'sessions.close':p=>{const s=getSession(p.sessionId);const turn=turns.get(s.turnId);if(turn&&!turn.done)complete(turn,'cancelled');s.state='closed';return {closed:true};},
  'sessions.updateOptions':p=>{const s=getSession(p.sessionId);if(['running','awaiting_approval','closed'].includes(s.state))error('CONFLICT');const selection={...s.selection,options:p.options};validate(selection,s.allowPaid);s.selection=selection;return s;},
  'turns.start':p=>{
   const session=getSession(p.sessionId);if(!authorized(session.connectionId,session.allowPaid))error('LOCAL_ACCESS_DENIED');if(['running','awaiting_approval','closed'].includes(session.state))error('CONFLICT');
   if(turns.size>=128)error('RESOURCE_LIMIT');if(typeof p.prompt!=='string'||p.prompt.length>8192)error('INVALID_REQUEST');validate(session.selection,session.allowPaid);
   const turn={id:id('turn'),sessionId:session.id,done:false,timer:null};turns.set(turn.id,turn);session.turnId=turn.id;
   if(p.simulateApproval){session.state='awaiting_approval';const approvalId=id('approval');approvals.set(approvalId,turn);emit(session,{type:'approval_request',approvalId,action:'Read a synthetic resource',detail:'Simulation only: no filesystem access occurs.'});}
   else if(turnError){turn.done=true;session.state='error';emit(session,{type:'error',code:turnError.code??'PROVIDER_UNAVAILABLE',message:turnError.message??'fixture provider error'});emit(session,{type:'state',state:'error'});emit(session,{type:'final',status:'error'});}
   else stream(turn);return {turnId:turn.id,mode:'synthetic'};
  },
  'turns.cancel':p=>{const turn=turns.get(p.turnId);if(!turn)error('NOT_FOUND');if(!turn.done)complete(turn,'cancelled');return {cancelled:true};},
  'approvals.respond':p=>{const turn=approvals.get(p.approvalId);if(!turn||turn.done)error('NOT_FOUND');if(typeof p.allow!=='boolean')error('INVALID_REQUEST');approvals.delete(p.approvalId);emit(getSession(turn.sessionId),{type:'approval_resolved',approvalId:p.approvalId,allowed:p.allow});if(p.allow)stream(turn);else complete(turn,'denied');return {allowed:p.allow};},
  'diagnostics.report':()=>({mode:'synthetic',nativeAccess:false,inferenceCalls:0,connections:snapshot.connections.length,models:snapshot.models.length,refreshes,activeSessions:sessions.size,limitations:['Native runtime and provider adapters are not wired into this fixture harness.','Live execution is disabled in the preview UI.']}),
  'events.subscribe':()=>({transport:'sse'}),'events.unsubscribe':()=>({unsubscribed:true}),
 };
 return {
  handle(method,params={}){if(closed)error('CLOSED');if(!Object.hasOwn(handlers,method))error('CAPABILITY_UNSUPPORTED');if(!params||typeof params!=='object'||Array.isArray(params))error('INVALID_REQUEST');return clone(handlers[method](params));},
  subscribe(listener){listeners.add(listener);return()=>listeners.delete(listener);},
  replay(after){if(after===undefined)return [];if(!/^\d{1,16}$/.test(after))error('INVALID_REQUEST');const n=Number(after);if(!Number.isSafeInteger(n))error('INVALID_REQUEST');if(history.length&&n<Number(history[0].sequence)-1)error('EVENT_GAP');return history.filter(e=>Number(e.sequence)>n).map(clone);},
  inspect(){return {sessions:sessions.size,refreshes};},
  close(){closed=true;for(const turn of turns.values())clearTimeout(turn.timer);listeners.clear();},
 };
}
