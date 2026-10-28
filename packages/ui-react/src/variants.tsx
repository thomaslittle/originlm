import type { Snapshot } from 'originlm';
import type { Candidate, Selection } from '@originlm/selector-core';
import type { SelectorPreset, SelectorVariant } from './types.ts';
interface Controls {
 variant:SelectorVariant;snapshot:Snapshot;candidates:readonly Candidate[];
 required:readonly string[];setRequired:(value:string[])=>void;preset:string;setPreset:(value:string)=>void;
 presets?:readonly SelectorPreset[];
}
export function VariantControls({variant,snapshot,candidates,required,setRequired,preset,setPreset,presets}:Controls){
 if(variant==='capabilities'){
  const capabilities=[...new Set(snapshot.models.flatMap(m=>m.capabilities.map(c=>c.id)))].sort();
  return <fieldset className="originlm-capabilities"><legend>Required capabilities</legend>{capabilities.map(id=><label key={id}><input type="checkbox" checked={required.includes(id)} onChange={event=>setRequired(event.target.checked?[...required,id]:required.filter(k=>k!==id))}/>{id}</label>)}<small>Unverified capabilities do not satisfy requirements.</small></fieldset>;
 }
 if(variant==='preset')return <fieldset className="originlm-presets"><legend>Choose a task configuration</legend><button type="button" aria-pressed={!preset} onClick={()=>setPreset('')}>All models</button>{presets?.map(p=><button key={p.id} type="button" aria-pressed={preset===p.id} onClick={()=>setPreset(p.id)}>{p.label}</button>)}{!presets?.length&&<p>Supply your application's requirement presets. No quality ranking or preferred model is invented.</p>}</fieldset>;
 if(variant==='allowance'){
  const labels=[...new Set(candidates.map(c=>c.billingLabel))];
  return <div className="originlm-allowances" aria-label="Billing routes">{labels.map(label=><span key={label}>{label}</span>)}<small>Allowances use provider-specific units. Shared pools are not added together.</small></div>;
 }
 if(variant==='command')return <p className="originlm-keyboard-help">Search all routes. Use ↑ / ↓, Home / End, and Enter to select.</p>;
 return null;
}
interface ComparisonProps {items:readonly Candidate[];page:number;setPage:(value:number)=>void;choose:(value:Candidate)=>void;value:Selection|null}
export function Comparison({items,page,setPage,choose,value}:ComparisonProps){
 const size=30,current=Math.min(page,Math.max(0,Math.ceil(items.length/size)-1));
 return <div className="originlm-comparison"><table><caption>Compare reported access routes</caption><thead><tr><th>Model</th><th>Provider / account</th><th>Billing</th><th>Access</th></tr></thead><tbody>
  {items.slice(current*size,(current+1)*size).map(c=><tr key={c.key}><th scope="row"><button type="button" data-model-id={c.model.id} aria-pressed={value?.modelId===c.model.id} onClick={()=>choose(c)}>{c.model.name}</button></th><td>{c.route.providerId}<small>{c.connection.displayName}</small></td><td>{c.billingLabel}</td><td>{c.eligible?'Eligible in snapshot':c.reasons.join('; ')}</td></tr>)}
 </tbody></table><div className="originlm-pagination"><button type="button" disabled={current===0} onClick={()=>setPage(current-1)}>Previous</button><span>Page {current+1} of {Math.max(1,Math.ceil(items.length/size))}</span><button type="button" disabled={(current+1)*size>=items.length} onClick={()=>setPage(current+1)}>Next</button></div></div>;
}
