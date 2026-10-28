import type { LocalAIClient } from './client.ts';
import type { Snapshot } from './generated.ts';
import { LocalAIError } from './errors.ts';
import { readCatalogSnapshot } from './catalog.ts';
export interface SnapshotState { readonly snapshot: Snapshot | null; readonly loading: boolean; readonly error: LocalAIError | null }
export interface SnapshotStore {
 getSnapshot(): SnapshotState; getServerSnapshot(): SnapshotState;
 subscribe(listener: () => void): () => void;
 load(): Promise<Snapshot>; scan(): Promise<Snapshot>; probe(connectionId: string): Promise<Snapshot>;
 dispose(): void;
}
function immutable<T>(value: T): T {
 if (value && typeof value === 'object') {
  for (const item of Object.values(value)) immutable(item);
  Object.freeze(value);
 }
 return value;
}
function copy(snapshot: Snapshot): Snapshot {
 if (!snapshot || !['live','synthetic'].includes(snapshot.mode) || !Array.isArray(snapshot.models) || !Array.isArray(snapshot.connections)) throw new LocalAIError('PROTOCOL_INVALID');
 return immutable(structuredClone(snapshot));
}
/** Subscribing or reading is inert. The host explicitly chooses load, scan, or probe. */
export function createSnapshotStore(client: LocalAIClient, initial?: Snapshot): SnapshotStore {
 let state: SnapshotState = Object.freeze({snapshot: initial ? copy(initial) : null, loading:false, error:null});
 const serverState = state;
 const listeners = new Set<() => void>(), actions = new Map<string, Promise<Snapshot>>();
 let closed=false, epoch=0, loadedEpoch=-1, loading:Promise<Snapshot>|undefined, controller:AbortController|undefined;
 const ensure=()=>{if(closed)throw new LocalAIError('CLOSED');};
 const publish=(next:SnapshotState)=>{
  state=Object.freeze(next);
  for(const listener of [...listeners]) { try { listener(); } catch { /* One host observer cannot suppress other observers. */ } }
 };
 function load():Promise<Snapshot> {
  try {ensure();}catch(error){return Promise.reject(error);}
  if(loading)return loading;
  const version=epoch, abort=new AbortController();controller=abort;
  publish({...state,loading:true,error:null});
  const work=readCatalogSnapshot(client,{signal:abort.signal}).then(value=>{
   if(closed)throw new LocalAIError('CLOSED');
   if(version!==epoch)throw new LocalAIError('CANCELLED');
   const snapshot=copy(value);loadedEpoch=version;publish({snapshot,loading:false,error:null});return snapshot;
  }).catch(error=>{
   const safe=error instanceof LocalAIError?error:new LocalAIError('PROVIDER_UNAVAILABLE');
   if(!closed&&version===epoch)publish({snapshot:null,loading:false,error:safe});
   throw safe;
  }).finally(()=>{if(loading===work)loading=undefined;});
  loading=work;return work;
 }
 function action(key:string,run:()=>Promise<unknown>):Promise<Snapshot>{
  try {ensure();}catch(error){return Promise.reject(error);}
  const existing=actions.get(key);if(existing)return existing;
  const work=(async()=>{
   await run();ensure();
   epoch++;controller?.abort();loading=undefined;
   // Another completed probe may supersede this read. Join its newer catalog
   // without replaying either metadata operation or hiding genuine read errors.
   for (;;) {
    if(loadedEpoch===epoch && state.snapshot)return state.snapshot;
    const version=epoch;
    try { return await load(); }
    catch(error) { if(closed || version===epoch)throw error; }
   }
  })().finally(()=>actions.delete(key));
  actions.set(key,work);return work;
 }
 return {
  getSnapshot:()=>state,getServerSnapshot:()=>serverState,
  subscribe(listener){ensure();listeners.add(listener);return()=>listeners.delete(listener);},
  load,scan:()=>action('scan',()=>client.discovery.refresh({level:'passive'})),
  probe(connectionId){if(typeof connectionId!=='string'||!connectionId||connectionId.length>256)return Promise.reject(new LocalAIError('INVALID_REQUEST'));return action('probe:'+connectionId,()=>client.connections.probe({connectionId}));},
  dispose(){if(closed)return;closed=true;epoch++;controller?.abort();publish({snapshot:null,loading:false,error:new LocalAIError('CLOSED')});listeners.clear();actions.clear();},
 };
}
