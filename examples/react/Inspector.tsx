import { useState } from 'react';
import type { SnapshotStore } from 'originlm';
import { LocalAIProvider, useLocalAIStore, useLocalAISnapshot } from '@originlm/react';
import { ProviderModelSelector, selectorVariants, type SelectorVariant } from '@originlm/ui-react';
import type { Selection } from '@originlm/selector-core';
import '@originlm/ui-react/styles.css';
/** The application supplies and disposes its own store and authorized SDK client. */
export function Inspector({store}:{store:SnapshotStore}){
 return <LocalAIProvider store={store}><Panel/></LocalAIProvider>;
}
function Panel(){
 const store=useLocalAIStore(),{snapshot,loading,error}=useLocalAISnapshot();
 const [variant,setVariant]=useState<SelectorVariant>('rail'),[selection,setSelection]=useState<Selection|null>(null);
 const request=(operation:Promise<unknown>)=>void operation.catch(()=>{});
 return <main>
  <h1>OriginLM</h1><p>Your application owns the connection, permissions, and persistence.</p>
  <button onClick={()=>request(store.load())} disabled={loading}>Read current snapshot</button>
  <button onClick={()=>request(store.scan())} disabled={loading}>Discover local connections</button>
  {error&&<p role="alert">{error.code}</p>}
  <label>Selector layout <select value={variant} onChange={event=>setVariant(event.target.value as SelectorVariant)}>{selectorVariants.map(v=><option key={v}>{v}</option>)}</select></label>
  {snapshot?<ProviderModelSelector snapshot={snapshot} variant={variant} value={selection} onValueChange={setSelection}
   presets={[{id:'text',label:'Text response',requirements:{requiredCapabilities:['generation.text']}},{id:'tools',label:'Tool-enabled model',requirements:{requiredCapabilities:['tools.call']}}]}/>:<p>No snapshot loaded. Reading this page does not inspect your machine.</p>}
  <pre>{selection?JSON.stringify(selection,null,2):'No model selected'}</pre>
 </main>;
}
