import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { createCatalog, resolveCandidates, reconcileSelection, type Candidate, type Selection } from '@originlm/selector-core';
import type { ProviderModelSelectorProps } from './types.ts';
import { ModelList } from './model-list.tsx';
import { ModelOptions } from './options.tsx';
import { VariantControls, Comparison } from './variants.tsx';
const EMPTY_REQUIREMENTS: NonNullable<ProviderModelSelectorProps['requirements']>={};
export function ProviderModelSelector(props:ProviderModelSelectorProps){
 const {snapshot,variant='rail',requirements=EMPTY_REQUIREMENTS}=props;
 const id=useId(),trigger=useRef<HTMLButtonElement>(null);
 const [internal,setInternal]=useState<Selection|null>(props.defaultValue??null);
 const value=props.value===undefined?internal:props.value;
 const [query,setQuery]=useState(''),[connection,setConnection]=useState<string>(''),[open,setOpen]=useState(false);
 const [required,setRequired]=useState<string[]>([]),[preset,setPreset]=useState(''),[page,setPage]=useState(0),[mobileStep,setMobileStep]=useState(0);
 const [localFavorites,setFavorites]=useState<readonly string[]>([]),[favoritesOnly,setFavoritesOnly]=useState(false),[notice,setNotice]=useState('');
 const favorites=props.favorites??localFavorites;
 const catalog=useMemo(()=>createCatalog(snapshot),[snapshot]);
 const chosenPreset=props.presets?.find(p=>p.id===preset);
 const all=useMemo(()=>resolveCandidates(catalog,{...requirements,...chosenPreset?.requirements,query,
  requiredCapabilities:[...(requirements.requiredCapabilities??[]),...(chosenPreset?.requirements.requiredCapabilities??[]),...required],
  billingPreference:variant==='allowance'?'prefer-included':requirements.billingPreference}),[catalog,requirements,chosenPreset,query,required,variant]);
 const availableConnections=useMemo(()=>{const routes=new Set(snapshot.models.map(m=>m.routeId));const ids=new Set(snapshot.routes.filter(r=>routes.has(r.id)).map(r=>r.connectionId));return snapshot.connections.filter(c=>ids.has(c.id));},[snapshot]);
 const activeConnection=availableConnections.some(c=>c.id===connection)?connection:availableConnections[0]?.id??'';
 const filtered=all.filter(c=>(!favoritesOnly||favorites.includes(c.key))&&(!(variant==='rail'||variant==='mobile')||Boolean(query)||c.connection.id===activeConnection));
 const selected=all.find(c=>c.model.id===value?.modelId&&c.route.id===value?.routeId);
 const review=useMemo(()=>value?reconcileSelection(catalog,value):null,[catalog,value]);
 const invalidReason=review?.needsReview?review.reason??'Model options or catalog revision changed. Review the selection.':'';
 const notified=useRef('');
 useEffect(()=>{if(invalidReason&&notified.current!==invalidReason){notified.current=invalidReason;props.onSelectionInvalidated?.(invalidReason);}if(!invalidReason)notified.current='';},[invalidReason,props.onSelectionInvalidated]);
 const update=(selection:Selection|null)=>{if(props.value===undefined)setInternal(selection);props.onValueChange?.(selection);};
 function choose(candidate:Candidate){
  if(!candidate.eligible&&!props.allowUnavailableSelection){setNotice(candidate.reasons.join('; '));return;}
  update({routeId:candidate.route.id,modelId:candidate.model.id,revision:candidate.model.revision,options:{}});setNotice('');
  if(variant==='mobile')setMobileStep(2);
  if(variant==='compact'){setOpen(false);trigger.current?.focus();}
 }
 const setProvider=(key:string)=>{setConnection(key);setQuery('');setPage(0);if(variant==='mobile')setMobileStep(1);};
 const toggleFavorite=()=>{if(!selected)return;const next=favorites.includes(selected.key)?favorites.filter(k=>k!==selected.key):[...favorites,selected.key];if(props.favorites===undefined)setFavorites(next);props.onFavoritesChange?.(next);};
 const rail=<nav className="originlm-provider-rail" aria-label="Providers and accounts">{availableConnections.map(c=><button key={c.id} type="button" aria-pressed={c.id===activeConnection} onClick={()=>setProvider(c.id)} title={c.displayName}>
  <span className="originlm-provider-icon" aria-hidden="true">{props.renderProviderIcon?.(c.providerId)??c.providerId.slice(0,2).toUpperCase()}</span><span>{c.displayName}</span>
 </button>)}</nav>;
 const search=<label className="originlm-search"><span className="originlm-visually-hidden">Search providers and models</span><input type="search" value={query} placeholder="Search providers, models, native IDs…" onChange={event=>{setQuery(event.target.value);setPage(0);}}/></label>;
 const list=<ModelList items={filtered} selection={value} choose={choose} allowUnavailable={Boolean(props.allowUnavailableSelection)} renderModel={props.renderModel} compact={props.density==='compact'}/>;
 const settings=selected&&value?<aside className="originlm-details" aria-label="Selected model configuration"><div className="originlm-detail-title"><strong>{selected.model.name}</strong><button type="button" onClick={toggleFavorite} aria-pressed={favorites.includes(selected.key)}>{favorites.includes(selected.key)?'Saved':'Save'}</button></div>
  <p>{selected.connection.displayName} · {selected.route.providerId}</p><code>{selected.model.nativeId}</code>
  <p className="originlm-billing">{selected.billingLabel} · Selection is not execution permission.</p>
  <ModelOptions key={selected.model.id} options={selected.model.options} selection={value} onChange={update} onSensitiveOption={props.onSensitiveOption}/>
  <details><summary>Exact selection</summary><pre>{JSON.stringify(value,null,2)}</pre></details>
 </aside>:<aside className="originlm-details originlm-empty">Select a model to inspect its native options and access route.</aside>;
 const selectors=<><VariantControls variant={variant} snapshot={snapshot} candidates={all} required={required} setRequired={setRequired} preset={preset} setPreset={setPreset} presets={props.presets}/>
  <div className="originlm-selector-body">{variant==='rail'?rail:null}<div className="originlm-selector-models">{search}{variant==='comparison'?<Comparison items={filtered} page={page} setPage={setPage} choose={choose} value={value}/>:list}</div>{settings}</div></>;
 return <section className={'originlm-selector '+(props.className??'')} data-originlm-variant={variant} data-density={props.density??'comfortable'} aria-label={props.label??'Provider and model selector'}
  onKeyDown={event=>{if(event.key==='Escape'&&variant==='compact'){setOpen(false);trigger.current?.focus();}}}>
  <header className="originlm-selector-header"><div><span className="originlm-kicker">{snapshot.mode==='synthetic'?'Synthetic examples':'Live runtime data'}</span><h3>{props.label??'Select your AI'}</h3></div>
   <button type="button" aria-pressed={favoritesOnly} onClick={()=>setFavoritesOnly(!favoritesOnly)}>Saved {favorites.length>0?`(${favorites.length})`:''}</button></header>
  {snapshot.mode==='synthetic'&&<p className="originlm-mode-note">Synthetic data. No account is being detected or used.</p>}
  {variant==='compact'?<><button ref={trigger} type="button" className="originlm-compact-trigger" aria-expanded={open} aria-controls={id} onClick={()=>setOpen(!open)}>{selected?.model.name??'Select a provider and model'} <span aria-hidden="true">⌄</span></button>{open&&<div id={id} className="originlm-compact-panel">{selectors}</div>}</>:variant==='mobile'?<div className="originlm-mobile-drilldown">{mobileStep>0&&<button type="button" onClick={()=>setMobileStep(mobileStep-1)}>Back</button>}<h4>{['Choose a provider','Choose a model','Model options'][mobileStep]}</h4>{mobileStep===0?rail:mobileStep===1?<>{search}{list}</>:settings}</div>:selectors}
  {(notice||invalidReason)&&<p className="originlm-notice" role="status">{notice||invalidReason}</p>}
 </section>;
}
