import test from 'node:test';
import assert from 'node:assert/strict';
import {createFixtureService} from '../fixture/service.mjs';
const setup=t=>{const s=createFixtureService();t.after(()=>s.close());return s;};
const selected=s=>{const snap=s.handle('snapshot.get');const model=snap.models[0];return {routeId:model.routeId,modelId:model.id,revision:model.revision,options:{}};};
function session(s){const selection=selected(s);const c=s.handle('snapshot.get').routes.find(r=>r.id===selection.routeId).connectionId;s.handle('grants.request',{connectionIds:[c],scopes:['execution']});return s.handle('sessions.create',{selection});}
test('fixture snapshot always declares synthetic mode and evidence',t=>{
 const s=setup(t),snap=s.handle('snapshot.get');assert.equal(snap.mode,'synthetic');assert.ok(snap.connections.length>=5);assert.ok(snap.models.every(m=>m.nativeId.startsWith('fixture/')));
 assert.equal(snap.connections[0].auth.source.kind,'synthetic');assert.equal(s.inspect().sessions,0);
});
test('fixture handshake advertises only implemented methods',t=>{
 const s=setup(t),h=s.handle('runtime.handshake');assert.equal(h.protocolVersion,'1.0');assert.ok(h.methods.includes('sessions.create'));assert.equal(h.methods.includes('connections.authenticate'),false);
 assert.throws(()=>s.handle('connections.authenticate'),/CAPABILITY_UNSUPPORTED/);
});
test('fixture snapshot projection does not run discovery',t=>{
 const s=setup(t);assert.deepEqual(Object.keys(s.handle('snapshot.get',{fields:['mode','revision']})).sort(),['mode','revision']);assert.equal(s.inspect().refreshes,0);
});
test('duplicate native model names retain separate routes',t=>{
 const s=setup(t),snap=s.handle('snapshot.get');const matches=snap.models.filter(m=>m.nativeId==='fixture/balanced');assert.ok(matches.length>=2);assert.equal(new Set(matches.map(m=>m.id)).size,matches.length);
});
test('shared pools and missing denominators remain explicit',t=>{
 const pools=setup(t).handle('usage.read');assert.ok(pools.some(p=>p.routeIds.length>1&&p.sharing==='verified'));
 const unknown=pools.flatMap(p=>p.windows).find(w=>w.reading.value.limit===null);assert.equal(unknown.reading.value.usedFraction,null);
});
test('session creation requires explicit fixture grant',t=>{
 const s=setup(t);assert.throws(()=>s.handle('sessions.create',{selection:selected(s)}),/LOCAL_ACCESS_DENIED/);
});
test('stale selections fail before a simulated turn',t=>{
 const s=setup(t);const selection=selected(s);selection.revision='old';assert.throws(()=>s.handle('selection.validate',{selection}),/SELECTION_STALE/);
});
test('unsupported option is not coerced to a default',t=>{
 const s=setup(t),selection=selected(s);selection.options={reasoningEffort:'not-reported'};assert.throws(()=>s.handle('selection.validate',{selection}),/OPTION_UNSUPPORTED/);
});
test('simulated turns require explicit billing acceptance on API route',t=>{
 const s=setup(t),snap=s.handle('snapshot.get');const route=snap.routes.find(r=>r.billing.value==='api_metered');const model=snap.models.find(m=>m.routeId===route.id);const selection={routeId:route.id,modelId:model.id,revision:model.revision,options:{}};
 assert.throws(()=>s.handle('selection.validate',{selection}),/BILLING_CONSENT_REQUIRED/);
});
test('stream, cancellation and final events use public event shape',async t=>{
 const s=setup(t),sess=session(s),events=[];let cancel;
 const got=new Promise(resolve=>{cancel=s.subscribe(e=>{events.push(e);if(e.payload.type==='text_delta')resolve();});});
 const turn=s.handle('turns.start',{sessionId:sess.id,prompt:'hello'});await got;s.handle('turns.cancel',{turnId:turn.turnId});cancel();
 assert.ok(events.every(e=>e.schemaVersion==='1.0'&&e.adapterId==='synthetic'));assert.equal(s.handle('sessions.get',{sessionId:sess.id}).state,'cancelled');assert.ok(events.some(e=>e.payload.type==='final'&&e.payload.status==='cancelled'));
});
test('tool approval pauses, denial does not perform an action',t=>{
 const s=setup(t),sess=session(s),events=[];s.subscribe(e=>events.push(e));s.handle('turns.start',{sessionId:sess.id,prompt:'x',simulateApproval:true});
 assert.equal(s.handle('sessions.get',{sessionId:sess.id}).state,'awaiting_approval');const approval=events.find(e=>e.payload.type==='approval_request');
 s.handle('approvals.respond',{approvalId:approval.payload.approvalId,allow:false});assert.equal(s.handle('sessions.get',{sessionId:sess.id}).state,'completed');assert.ok(events.some(e=>e.payload.type==='final'&&e.payload.status==='denied'));
});
test('revoking a grant blocks subsequent turns',t=>{
 const s=setup(t),sess=session(s),grants=s.handle('grants.list');s.handle('grants.revoke',{grantId:grants[0].id});assert.throws(()=>s.handle('turns.start',{sessionId:sess.id,prompt:'x'}),/LOCAL_ACCESS_DENIED/);
});
test('redacted diagnostics cannot expose a native path or provider secret',t=>{
 const d=setup(t).handle('diagnostics.report');assert.equal(d.mode,'synthetic');assert.equal(d.nativeAccess,false);assert.equal(d.inferenceCalls,0);
});

test('closed sessions cannot be reconfigured and turn history is bounded',()=>{
 const service=createFixtureService();
 const snapshot=service.handle('snapshot.get');
 const model=snapshot.models[0];
 const selection={routeId:model.routeId,modelId:model.id,revision:model.revision,options:{}};
 service.handle('grants.request',{connectionIds:[snapshot.routes[0].connectionId],scopes:['execution']});
 const session=service.handle('sessions.create',{selection});
 service.handle('sessions.close',{sessionId:session.id});
 assert.throws(()=>service.handle('sessions.updateOptions',{sessionId:session.id,options:{}}),/CONFLICT/);
 const active=service.handle('sessions.create',{selection});
 for(let i=0;i<128;i++){
  const turn=service.handle('turns.start',{sessionId:active.id,prompt:'synthetic'});
  service.handle('turns.cancel',{turnId:turn.turnId});
 }
 assert.throws(()=>service.handle('turns.start',{sessionId:active.id,prompt:'synthetic'}),/RESOURCE_LIMIT/);
 service.close();
});

test('event cursors beyond exact integer range are rejected',()=>{
 const service=createFixtureService();
 assert.throws(()=>service.replay('9999999999999999'),/INVALID_REQUEST/);
 service.close();
});
