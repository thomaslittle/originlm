import type { UsageWindow } from 'originlm';
import { known } from '@/lib/format';

/** Pressure colors: calm below 70%, attention to 90%, alarm beyond (the accent red is not a status color). */
export function toneFor(fraction: number): string {
  if (fraction >= 0.9) return 'var(--destructive)';
  if (fraction >= 0.7) return 'var(--warning)';
  return 'var(--chart-3)';
}

export const percent = (fraction: number) => `${Math.round(fraction * 100)}%`;

export interface Reading {
  window: UsageWindow;
  used: string;
  unit: string;
  limit: string | null;
  /** Used share of a provider-published total, or null when no total exists (never invented). */
  fraction: number | null;
  freshness: string;
}

export function readingOf(window: UsageWindow): Reading | null {
  const value = known(window.reading);
  if (!value) return null;
  const hasTotal = value.limit != null && typeof value.usedFraction === 'number' && Number.isFinite(value.usedFraction);
  return {
    window,
    used: value.used,
    unit: value.unit,
    limit: value.limit ?? null,
    fraction: hasTotal ? Math.max(0, Math.min(1, value.usedFraction as number)) : null,
    freshness: window.reading.state === 'known' ? window.reading.freshness : 'unknown',
  };
}
