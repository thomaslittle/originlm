import type { CSSProperties } from 'react';
import { providerIconUrl, providerName } from '@/lib/provider-icons';
import { cn } from '@/lib/utils';

const TILE = {
  sm: { box: 'size-6 rounded-md', mark: 'size-3.5', text: 'text-[0.6rem]' },
  md: { box: 'size-8 rounded-lg', mark: 'size-[1.125rem]', text: 'text-xs' },
  lg: { box: 'size-11 rounded-xl', mark: 'size-6', text: 'text-base' },
} as const;

/** The bare brand mark, tinted with currentColor. Falls back to the provider's initial. */
export function ProviderMark({ id, name, className }: { id: string; name?: string; className?: string }) {
  const url = providerIconUrl(id);
  if (!url) {
    return (
      <span aria-hidden="true" className={cn('inline-grid place-items-center font-semibold uppercase', className)}>
        {providerName(id, name).slice(0, 1)}
      </span>
    );
  }
  return <span aria-hidden="true" className={cn('provider-mark', className)} style={{ '--icon': `url("${url}")` } as CSSProperties} />;
}

/** A provider tile: the real brand mark on a neutral surface (a letter only when no mark exists). */
export function ProviderGlyph({ id, name, size = 'md', className }: { id: string; name?: string; size?: keyof typeof TILE; className?: string }) {
  const tile = TILE[size];
  return (
    <span aria-hidden="true" className={cn('inline-grid shrink-0 place-items-center bg-surface text-foreground ring-1 ring-border', tile.box, tile.text, className)}>
      <ProviderMark id={id} name={name} className={tile.mark} />
    </span>
  );
}
