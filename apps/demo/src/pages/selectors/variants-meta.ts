import { ColumnsIcon, CommandIcon, DeviceMobileIcon, FadersIcon, SidebarSimpleIcon, SlidersHorizontalIcon, SparkleIcon, WalletIcon, type Icon } from '@phosphor-icons/react';
import type { VariantId } from '@/state/types';

export const VARIANTS: { id: VariantId; label: string; description: string; icon: Icon }[] = [
  { id: 'rail', label: 'Provider rail', description: 'Providers first, models inside', icon: SidebarSimpleIcon },
  { id: 'compact', label: 'Compact', description: 'A small composer control', icon: FadersIcon },
  { id: 'palette', label: 'Command palette', description: 'Keyboard-first global search', icon: CommandIcon },
  { id: 'capability', label: 'Capability first', description: 'Choose what the task needs', icon: SlidersHorizontalIcon },
  { id: 'compare', label: 'Compare', description: 'Inspect routes side by side', icon: ColumnsIcon },
  { id: 'allowance', label: 'Allowance aware', description: 'Keep billing sources visible', icon: WalletIcon },
  { id: 'preset', label: 'Task presets', description: 'Requirements, not model hype', icon: SparkleIcon },
  { id: 'mobile', label: 'Mobile drill-down', description: 'One decision at a time', icon: DeviceMobileIcon },
];

export const CAPABILITY_OPTIONS: { id: string; label: string }[] = [
  { id: 'generation.text', label: 'Text generation' },
  { id: 'input.text', label: 'Text input' },
  { id: 'input.image', label: 'Image input' },
  { id: 'structured.output', label: 'Structured output' },
  { id: 'tools', label: 'Tool support' },
  { id: 'offline', label: 'Offline operation' },
];

export const PRESETS: { name: string; required: string[] }[] = [
  { name: 'Text task', required: ['generation.text'] },
  { name: 'Structured result', required: ['generation.text', 'structured.output'] },
  { name: 'Offline task', required: ['generation.text', 'offline'] },
  { name: 'Tool-assisted task', required: ['generation.text', 'tools'] },
];
