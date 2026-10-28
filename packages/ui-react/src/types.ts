import type { ReactNode } from 'react';
import type { Snapshot } from 'originlm';
import type { Candidate, OptionView, Requirements, Selection } from '@originlm/selector-core';
export const selectorVariants=['rail','compact','command','capabilities','comparison','allowance','preset','mobile'] as const;
export type SelectorVariant=typeof selectorVariants[number];
export interface SelectorPreset {id:string;label:string;requirements:Requirements}
export interface ProviderModelSelectorProps {
 snapshot:Snapshot;
 variant?:SelectorVariant;
 value?:Selection|null;
 defaultValue?:Selection|null;
 onValueChange?:(selection:Selection|null)=>void;
 onSelectionInvalidated?:(reason:string)=>void;
 requirements?:Requirements;
 presets?:readonly SelectorPreset[];
 favorites?:readonly string[];
 onFavoritesChange?:(keys:readonly string[])=>void;
 allowUnavailableSelection?:boolean;
 density?:'comfortable'|'compact';
 className?:string;
 label?:string;
 renderProviderIcon?:(providerId:string)=>ReactNode;
 renderModel?:(candidate:Candidate)=>ReactNode;
 onSensitiveOption?:(descriptor:OptionView,value:unknown)=>boolean|Promise<boolean>;
}
export interface ModelOptionsProps {
 options:readonly OptionView[];
 selection:Selection;
 onChange:(selection:Selection)=>void;
 onSensitiveOption?:ProviderModelSelectorProps['onSensitiveOption'];
}
