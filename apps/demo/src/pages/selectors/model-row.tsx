import { memo } from 'react';
import { BracketsCurlyIcon, CheckIcon, ImageIcon, TextAaIcon, TextTIcon, WarningIcon, WifiSlashIcon, WrenchIcon, type Icon } from '@phosphor-icons/react';
import type { Candidate } from '@originlm/selector-core';
import { Badge } from '@/components/ui/badge';
import { ProviderGlyph } from '@/components/common/provider-glyph';
import { billingVariant, capNames, reasonLabel } from '@/lib/format';
import { cn } from '@/lib/utils';

const CAPABILITY_ICONS: Record<string, Icon> = {
  'generation.text': TextAaIcon,
  'input.text': TextTIcon,
  'input.image': ImageIcon,
  'structured.output': BracketsCurlyIcon,
  tools: WrenchIcon,
  offline: WifiSlashIcon,
};

/** Supported reads as a neutral chip, conditional as attention, anything else as muted. */
function supportClass(support: string): string {
  if (support === 'supported') return '';
  if (support === 'conditional') return 'border-warning/30 bg-warning/10 text-warning';
  return 'border-dashed bg-transparent text-muted-foreground line-through decoration-muted-foreground/50';
}

interface ModelRowProps {
  candidate: Candidate;
  selected: boolean;
  onSelect: (candidate: Candidate) => void;
}

/** One route. Identity (account, billing, native id) stays visible; eligibility is explained, never hidden. */
export const ModelRow = memo(function ModelRow({ candidate, selected, onSelect }: ModelRowProps) {
  const { model, route, connection } = candidate;
  return (
    <div
      data-model-row=""
      className={cn(
        'group/row relative rounded-xl border bg-card/40 transition-[border-color,background-color,box-shadow] duration-200',
        'hover:border-primary/30 hover:bg-card/80',
        selected && 'border-primary/50 bg-primary/8 shadow-[0_0_0_1px_color-mix(in_oklch,var(--primary)_35%,transparent)] hover:border-primary/50',
      )}
    >
      <button
        type="button"
        onClick={() => onSelect(candidate)}
        data-testid={model.id}
        data-eligible={String(candidate.eligible)}
        aria-pressed={selected}
        className="flex w-full min-w-0 items-start gap-3 rounded-xl p-3.5 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <ProviderGlyph id={route.providerId} name={route.displayName || route.providerId} className="mt-0.5" />
        <span className="min-w-0 flex-1">
          <span className="flex items-start justify-between gap-2">
            <strong className="text-sm font-semibold break-words">{model.name}</strong>
            {selected ? (
              <Badge variant="brand">
                <CheckIcon aria-hidden="true" />
                Selected
              </Badge>
            ) : null}
          </span>
          <span className="mt-1 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-xs text-muted-foreground">
            <span>{connection.displayName}</span>
            <Badge variant={billingVariant(candidate.billingLabel)}>{candidate.billingLabel}</Badge>
          </span>
          <span className="mt-2.5 flex flex-wrap gap-1.5">
            {model.capabilities.map((capability) => {
              const Icon = CAPABILITY_ICONS[capability.id];
              return (
                <Badge key={capability.id} variant={capability.support === 'supported' ? 'secondary' : 'outline'} className={supportClass(capability.support)} title={`${capNames[capability.id] ?? capability.id}: ${capability.support}`}>
                  {Icon ? <Icon aria-hidden="true" /> : null}
                  {capNames[capability.id] ?? capability.id}
                </Badge>
              );
            })}
          </span>
          {!candidate.eligible ? (
            <span className="mt-2.5 flex items-start gap-1.5 text-xs break-words text-warning">
              <WarningIcon className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
              {candidate.reasons.map(reasonLabel).join(' · ')}
            </span>
          ) : null}
        </span>
      </button>
    </div>
  );
});
