import { useId, useRef, useState, type ReactNode } from 'react';
import { validateOption, type OptionView } from '@originlm/selector-core';
import type { ModelOptionsProps } from './types.ts';
export function ModelOptions({options,selection,onChange,onSensitiveOption}:ModelOptionsProps){
 const prefix=useId(),[notice,setNotice]=useState(''),[pending,setPending]=useState(false);
 const identity=JSON.stringify(selection),latest=useRef(identity);latest.current=identity;
 async function change(option:OptionView,value:unknown){
  setNotice('');
  if(value!==undefined&&validateOption(option,value)){setNotice('That value is not supported by this native option.');return;}
  const captured=identity;
  if(option.billingSensitive||option.permissionSensitive){
   if(!onSensitiveOption){setNotice('Host approval is required before changing this sensitive option.');return;}
   setPending(true);let allowed=false;
   try {allowed=await onSensitiveOption(option,value);}catch{setNotice('The host could not approve this change.');}
   finally{setPending(false);}
   if(!allowed){setNotice('Host approval was not granted.');return;}
   if(latest.current!==captured){setNotice('The selection changed while approval was pending. Review it again.');return;}
  }
  const next={...selection.options};if(value===undefined)delete next[option.id];else next[option.id]=value;
  onChange({...selection,options:next});
 }
 if(!options.length)return <p className="originlm-empty">No editable native options are reported for this model.</p>;
 return <section className="originlm-options" aria-label="Native model options">
  {options.map((option,index)=>{
   const id=prefix+'-option-'+index,value=selection.options[option.id],control=option.control;let input:ReactNode;
   if(control.type==='select')input=<select id={id} value={value===undefined?'':JSON.stringify(value)} disabled={pending}
    onChange={event=>void change(option,event.target.value===''?undefined:JSON.parse(event.target.value))}>
    <option value="">{control.defaultValue==null?'Use provider default':`Provider default: ${control.choices.find(c=>c.value===control.defaultValue)?.label??control.defaultValue}`}</option>
    {control.choices.map(choice=><option key={choice.value} value={JSON.stringify(choice.value)} disabled={Boolean(choice.disabledReason)}>{choice.label}{choice.disabledReason?' — '+choice.disabledReason:''}</option>)}
   </select>;
   else if(control.type==='boolean')input=<select id={id} disabled={pending} value={value===undefined?'':String(value)} onChange={event=>void change(option,event.target.value===''?undefined:event.target.value==='true')}><option value="">Use provider default</option><option value="true">Enabled</option><option value="false">Disabled</option></select>;
   else if(control.type==='number')input=<input id={id} disabled={pending} type="number" min={control.min} max={control.max} step={control.step} value={typeof value==='number'?value:''} placeholder="Provider default" onChange={event=>void change(option,event.target.value===''?undefined:event.target.valueAsNumber)}/>;
   else if(control.type==='text')input=<input id={id} disabled={pending} type="text" maxLength={control.maxLength} value={typeof value==='string'?value:''} placeholder="Provider default" onChange={event=>void change(option,event.target.value===''?undefined:event.target.value)}/>;
   else input=<p id={id} className="originlm-empty">Inspectable only: {control.nativeType}</p>;
   return <div className="originlm-field" key={option.id}><label htmlFor={id}>{option.label}</label>{input}<small>{option.scope} setting{option.billingSensitive?' · may affect billing':''}{option.permissionSensitive?' · changes permissions':''}</small></div>;
  })}
  {notice&&<p className="originlm-notice" role="status">{notice}</p>}
 </section>;
}
