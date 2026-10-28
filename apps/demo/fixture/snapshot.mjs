/** Entirely synthetic scenarios. These records are not claims about real providers. */
export function fixtureSnapshot(){
 const now=new Date().toISOString();
 const source={kind:'synthetic',adapterId:'synthetic',adapterVersion:'0.1.0-alpha.1',reference:'bundled demonstration scenarios'};
 const known=value=>({state:'known',value,source,observedAt:now,freshness:'fresh',verification:'reported'});
 const unknown=reason=>({state:'unknown',reason});
 const providers=[['codex','Codex'],['antigravity','Antigravity'],['grok','Grok'],['opencode','OpenCode'],['claude','Claude Code'],['local','Local runtime']].map(([id,name])=>({id,name,kind:'synthetic_client'}));
 const rows=[
  ['codex-personal','codex','Personal workspace','subscription','authenticated'],
  ['codex-work','codex','Work workspace','subscription','authenticated-stale'],
  ['antigravity-personal','antigravity','Google workspace','subscription','authenticated'],
  ['grok-personal','grok','Grok workspace','subscription','authenticated'],
  ['opencode-linked','opencode','Linked account','subscription','authenticated'],
  ['opencode-api','opencode','API-key route','api_metered','authenticated'],
  ['claude-expired','claude','Expired sign-in','unknown','expired'],
  ['local-machine','local','On this device','local','authenticated'],
 ];
 const installations=rows.map(([id,providerId])=>({id:'install-'+id,providerId,environmentId:'synthetic-host',locationHint:'Synthetic installation (no file was inspected)',version:known('fixture-1.0')}));
 const profiles=rows.map(([id])=>({id:'profile-'+id,installationId:'install-'+id,label:'Example profile'}));
 const accounts=rows.map(([id,providerId])=>({id:'account-'+id,providerId,displayLabel:known('Example account'),identitySource:known('synthetic/'+id)}));
 const connections=rows.map(([id,providerId,displayName,billing,auth])=>({
  id,providerId,displayName,installationId:'install-'+id,profileId:'profile-'+id,accountId:'account-'+id,
  auth:auth==='authenticated-stale'?{...known('authenticated'),freshness:'stale'}:known(auth),plan:billing==='unknown'?unknown('PLAN_NOT_REPORTED'):known({nativeId:'fixture-'+billing,label:billing==='subscription'?'Example included plan':billing==='local'?'Local compute':'Example metered access'}),
  status:auth==='expired'?'needs_auth':'ok',diagnostics:auth==='expired'?['SYNTHETIC_EXPIRED_LOGIN']:[],
 }));
 const routes=rows.map(([id,providerId,displayName,billing])=>({id:'route-'+id,connectionId:id,providerId,displayName,billing:billing==='unknown'?unknown('BILLING_NOT_REPORTED'):known(billing),capabilities:[],catalogRevision:'fixture-1'}));
 const options=(id)=>id.startsWith('codex')?[{id:'reasoningEffort',label:'Reasoning effort',scope:'turn',revision:'fixture-1',category:'thought_level',source,
   control:{type:'select',choices:['low','medium','high','cosmic'].map(value=>({value,label:value[0].toUpperCase()+value.slice(1)})),defaultValue:'medium'}}]
  :id.startsWith('antigravity')?[{id:'thinkingBudget',label:'Thinking budget',scope:'turn',revision:'fixture-1',category:'thought_level',source,control:{type:'number',min:0,max:16384,step:1024}}]
  :id.startsWith('grok')?[{id:'serviceTier',label:'Service tier',scope:'session',revision:'fixture-1',category:'service_tier',billingSensitive:true,source,control:{type:'select',choices:[{value:'standard',label:'Standard'},{value:'priority',label:'Priority'}],defaultValue:'standard'}}]
  :[];
 // Every capability the demo shows is reported explicitly (supported, conditional, or unsupported),
 // so the comparison table never has to print a missing value. Conditional means "supported once
 // the stated condition holds", which selection treats as unmet, never as supported.
 const cap=(id,support='supported',conditions=[])=>({id,support,origin:'model',conditions,evidence:known('Illustrative fixture only')});
 const contextWindows={codex:['128000','200000'],antigravity:['1000000','2000000'],grok:['256000','256000'],opencode:['128000','200000'],claude:['200000','1000000'],local:['32768','65536']};
 const imageInput={codex:[true,true],antigravity:[true,true],grok:[false,true],opencode:[true,true],claude:[true,true],local:[false,false]};
 const models=routes.flatMap((route,i)=>[0,1].map(n=>({
  id:`model-${route.connectionId}-${n}`,routeId:route.id,nativeId:n===0?'fixture/balanced':'fixture/precise',name:n===0?'Example Balanced':'Example Precise',revision:'fixture-1',isDefault:n===0,
  capabilities:[
   cap('generation.text'),cap('input.text'),
   cap('input.image',(imageInput[route.providerId]??[false,false])[n]?'supported':'unsupported'),
   n?cap('structured.output'):cap('structured.output','conditional',['json_mode']),
   i%3===0?cap('tools'):cap('tools','conditional',['tool_approval']),
   ...(route.providerId==='local'?[cap('offline')]:[]),
  ],
  options:n===0?options(route.connectionId):[],contextWindow:known((contextWindows[route.providerId]??['128000','128000'])[n]),
 })));
 const reading=(unit,used,limit)=>known({unit,used:String(used),limit:limit===null?null:String(limit),remaining:limit===null?null:String(Math.max(0,limit-used)),usedFraction:limit===null?null:used/limit});
 const quotaPools=[
  {id:'pool-shared-personal',accountId:'account-codex-personal',connectionIds:['codex-personal','opencode-linked'],routeIds:['route-codex-personal','route-opencode-linked'],sharing:'verified',windows:[{id:'example-session',label:'Example session window',reading:reading('credits',25,100),durationSeconds:18000,resetsAt:new Date(Date.now()+3600000).toISOString()}]},
  {id:'pool-work',accountId:'account-codex-work',connectionIds:['codex-work'],routeIds:['route-codex-work'],sharing:'unknown',windows:[{id:'example-week',label:'Example weekly window',reading:{...reading('credits',70,100),freshness:'stale'},durationSeconds:604800,resetsAt:null}]},
  {id:'pool-unknown-total',accountId:'account-antigravity-personal',connectionIds:['antigravity-personal'],routeIds:['route-antigravity-personal'],sharing:'unknown',windows:[{id:'example-count',label:'Count without a published total',reading:reading('requests',17,null),durationSeconds:86400,resetsAt:null}]},
 ];
 return {protocolVersion:'1.0',revision:'fixture-1',observedAt:now,mode:'synthetic',providers,installations,profiles,accounts,connections,routes,models,quotaPools,diagnostics:['All records are synthetic examples. No installed clients, accounts, or credentials have been inspected.']};
}
