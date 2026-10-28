import { useId, useRef, useState, type ReactNode, type KeyboardEvent } from 'react';
import type { Candidate, Selection } from '@originlm/selector-core';
interface Props {
 items: readonly Candidate[]; selection: Selection | null; choose: (candidate: Candidate) => void;
 allowUnavailable: boolean; renderModel?: (candidate: Candidate) => ReactNode; compact?: boolean;
}
/** Fixed-height virtual rows retain the active option, including while scrolling. */
export function ModelList({items,selection,choose,allowUnavailable,renderModel,compact}:Props) {
 const id=useId(),ref=useRef<HTMLDivElement>(null);
 const [start,setStart]=useState(0),[activeKey,setActiveKey]=useState<string|null>(null);
 const row=compact?44:56,visible=6,overscan=4;
 const found=items.findIndex(c=>c.key===activeKey),active=found<0?0:found;
 const beginning=Math.max(0,Math.min(start,Math.max(0,items.length-visible))-overscan);
 const indices=new Set(Array.from({length:Math.min(visible+overscan*2,items.length-beginning)},(_,i)=>beginning+i));
 if(items.length)indices.add(active);
 const optionId=(index:number)=>id+'-model-'+index;
 function keyboard(event:KeyboardEvent<HTMLDivElement>){
  if(!items.length)return;
  if(event.key==='Enter'||event.key===' '){event.preventDefault();choose(items[active]);return;}
  let next=active;
  switch(event.key){case 'ArrowDown':next++;break;case 'ArrowUp':next--;break;case 'Home':next=0;break;case 'End':next=items.length-1;break;case 'PageDown':next+=visible;break;case 'PageUp':next-=visible;break;default:return;}
  event.preventDefault();next=Math.max(0,Math.min(next,items.length-1));setActiveKey(items[next].key);
  const first=Math.max(0,next-Math.floor(visible/2));setStart(first);
  if(ref.current)ref.current.scrollTop=first*row;
 }
 if(!items.length)return <p className="originlm-empty" role="status">No models match these filters. Unknown capabilities never count as supported.</p>;
 return <div ref={ref} className="originlm-model-list" role="listbox" aria-label="Models" tabIndex={0}
  aria-activedescendant={optionId(active)} onKeyDown={keyboard} style={{height:visible*row}}
  onScroll={event=>setStart(Math.floor(event.currentTarget.scrollTop/row))}>
  <div role="presentation" style={{height:items.length*row,position:'relative'}}>
   {[...indices].sort((a,b)=>a-b).map(index=>{
    const item=items[index],selected=selection?.modelId===item.model.id&&selection.routeId===item.route.id;
    const disabled=!item.eligible&&!allowUnavailable;
    return <div key={item.key} id={optionId(index)} role="option" aria-selected={selected} aria-disabled={disabled}
     aria-posinset={index+1} aria-setsize={items.length} data-model-id={item.model.id}
     className={'originlm-model-row'+(selected?' originlm-selected':'')+(index===active?' originlm-active':'')}
     style={{position:'absolute',top:index*row,height:row,left:0,right:0}}
     title={item.reasons.length?item.reasons.join('; '):item.model.nativeId}
     onClick={()=>{setActiveKey(item.key);choose(item);}}>
     {renderModel?renderModel(item):<><span className="originlm-row-copy"><strong>{item.model.name}</strong><small>{item.connection.displayName} · {item.route.providerId}</small></span><span className="originlm-row-state">{selected?'Selected':item.eligible?item.billingLabel:'Inspect access'}</span></>}
    </div>;
   })}
  </div>
 </div>;
}
