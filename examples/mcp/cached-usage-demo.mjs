/** Synthetic stdio MCP endpoint. No broker, provider, credential or configuration access. */
import {createLiveSnapshotStore} from '../../packages/sdk-typescript/src/live.ts';
import {createCachedUsageMcp} from '../../packages/sdk-typescript/src/cached-usage-mcp.ts';
import {serveCachedUsageMcp} from '../../packages/sdk-typescript/node/cached-usage-mcp.mjs';
import {LocalAIError} from '../../packages/sdk-typescript/src/errors.ts';
const at='2020-01-01T00:00:00Z';
const unknown={state:'unknown',reason:'PRIVATE-REASON',checkedAt:at};
let requests=0;
const forbidden=()=>{requests++;throw new Error('Synthetic cache attempted provider/client work');};
const client={snapshot:{get:forbidden},discovery:{refresh:forbidden},connections:{probe:forbidden},close:forbidden};
const accounts=['codex','claude'].map(provider=>({id:'account-'+provider,providerId:provider,
 displayLabel:{...unknown},identitySource:{...unknown}}));
const connections=accounts.map(account=>({id:'connection-'+account.providerId,providerId:account.providerId,
 installationId:'installation-'+account.providerId,profileId:'profile-'+account.providerId,displayName:'PRIVATE-EMAIL',
 accountId:account.id,auth:{...unknown},plan:{...unknown},status:'detected',diagnostics:[]}));
const known={state:'known',freshness:'fresh',observedAt:at,expiresAt:at,verification:'reported',
 source:{kind:'synthetic',adapterId:'documentation-fixture',adapterVersion:'0.1.0',reference:'PRIVATE-REFERENCE'},
 value:{unit:'percent',used:'95',limit:'100',remaining:'5',usedFraction:0.95}};
const quotaPools=[
 {id:'pool-codex',accountId:'account-codex',connectionIds:['connection-codex'],routeIds:['route-codex'],sharing:'unknown',windows:[
  {id:'window-known',label:'PRIVATE-WINDOW',reading:known,resetsAt:at,durationSeconds:18000},
  {id:'window-unknown',label:'PRIVATE-WINDOW',reading:unknown,resetsAt:null}]},
 {id:'pool-claude',accountId:'account-claude',connectionIds:['connection-claude'],routeIds:['route-claude'],sharing:'unknown',windows:[
  {id:'window-unsupported',label:'PRIVATE-WINDOW',reading:{state:'unavailable',reason:'CAPABILITY_UNSUPPORTED',checkedAt:at}}]},
 {id:'pool-hidden',accountId:'account-outside-scope',connectionIds:['connection-codex'],routeIds:[],sharing:'unknown',windows:[]},
];
const snapshot={protocolVersion:'1.0',mode:'synthetic',revision:'fixture-1',cursor:'fixture:1',observedAt:at,
 providers:accounts.map(account=>({id:account.providerId,name:account.providerId,kind:'synthetic'})),
 accounts,connections,installations:[],profiles:[],models:[],diagnostics:[],quotaPools,
 routes:connections.map(connection=>({id:'route-'+connection.providerId,connectionId:connection.id,providerId:connection.providerId,
  displayName:'Synthetic route',billing:unknown,capabilities:[],catalogRevision:'fixture-1'}))};
const store=createLiveSnapshotStore(client,snapshot);
if(process.argv.includes('--unavailable'))store.dispose();
if(process.argv.includes('--revoked')){
 let denials=0;
 client.snapshot.get=async()=>{requests++;return snapshot;};
 client.models={page:async()=>{requests++;return {revision:snapshot.revision,models:[],nextCursor:null};}};
 client.inventory={subscribe:async function*(){denials++;throw new LocalAIError('LOCAL_ACCESS_DENIED');}};
 let timer;
 const retired=new Promise((resolve,reject)=>{
  timer=setTimeout(()=>reject(new Error('Synthetic revocation deadline')),1000);
  const off=store.subscribe(()=>{if(store.getSnapshot().error){off();resolve();}});
 });
 store.connect();await retired;clearTimeout(timer);
 if(denials!==1)throw new Error('Synthetic subscription denial was not exercised');
}
const setupRequests=requests;
let cacheReads=0;
const view={getSnapshot(){cacheReads++;return store.getSnapshot();}};
const handler=createCachedUsageMcp({store:view,scope:{connectionIds:process.argv.includes('--no-scope') ? [] : connections.map(connection=>connection.id),accountIds:accounts.map(account=>account.id)},
 mappings:[{consumerAccountId:'example-consumer-account',kitAccountId:'account-codex',privateToken:'PRIVATE-MAPPING'}]});
await serveCachedUsageMcp(handler);
if(requests!==setupRequests)throw new Error('MCP unexpectedly made client requests');
// The bounded demonstration has two valid calls. Initialization/list/invalid calls read nothing.
if(process.env.LAI_MCP_VERIFY_READS==='1'&&cacheReads!==2)throw new Error('Unexpected cache read count');
