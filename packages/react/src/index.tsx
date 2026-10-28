import { createContext, useContext, useMemo, useSyncExternalStore, type ReactNode } from 'react';
import type { SnapshotStore, SnapshotState } from 'originlm';
import { createCatalog, resolveCandidates, reconcileSelection, type SnapshotView, type Requirements, type Selection } from '@originlm/selector-core';
const Context = createContext<SnapshotStore | null>(null);
/** The host owns the connection and explicitly starts discovery. Rendering never does. */
export function LocalAIProvider({store,children}: {store:SnapshotStore;children?:ReactNode}): ReactNode {
 return <Context.Provider value={store}>{children}</Context.Provider>;
}
export function useLocalAIStore():SnapshotStore {
 const store=useContext(Context);
 if(!store)throw new Error('Local AI hooks require an explicit LocalAIProvider');
 return store;
}
export function useLocalAISnapshot():SnapshotState {
 const store=useLocalAIStore();
 return useSyncExternalStore(store.subscribe,store.getSnapshot,store.getServerSnapshot);
}
const EMPTY:SnapshotView={connections:[],routes:[],models:[]};
export function useModelCatalog(snapshot:SnapshotView|null){
 return useMemo(()=>createCatalog(snapshot??EMPTY),[snapshot]);
}
export function useModelCandidates(snapshot:SnapshotView|null,requirements:Requirements={}){
 const catalog=useModelCatalog(snapshot);
 return useMemo(()=>resolveCandidates(catalog,requirements),[catalog,requirements]);
}
/** Reconciliation is advisory. A model revision never silently changes the host's selection. */
export function useSelectionReview(snapshot:SnapshotView|null,selection:Selection|null){
 const catalog=useModelCatalog(snapshot);
 return useMemo(()=>selection?reconcileSelection(catalog,selection):null,[catalog,selection]);
}
export { createSnapshotStore } from 'originlm';
export type { SnapshotStore, SnapshotState } from 'originlm';
