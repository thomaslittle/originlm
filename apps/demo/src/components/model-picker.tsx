import { useDeferredValue, useMemo, useState } from 'react';
import { CaretDownIcon, CheckIcon, SquaresFourIcon, StarIcon, WarningCircleIcon } from '@phosphor-icons/react';
import type { Candidate } from '@originlm/selector-core';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Command, CommandEmpty, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { ProviderMark } from '@/components/common/provider-glyph';
import { providerName } from '@/lib/provider-icons';
import { cn } from '@/lib/utils';
import { useActions, useAllCandidates, useDemo, useSelectedCandidate } from '@/state/context';
import { billingVariant } from '@/lib/format';

const ALL = 'all';
const FAVORITES = 'favorites';
const MAX_ROWS = 60;

/** Case-insensitive match of every search word against the route's searchable text. */
function matches(candidate: Candidate, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return words.every((word) => candidate.search.includes(word));
}

interface RailButtonProps {
  label: string;
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function RailButton({ label, active, onClick, children }: RailButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={active}
      title={label}
      onClick={onClick}
      className={cn(
        'relative flex aspect-square w-full items-center justify-center rounded-md text-muted-foreground transition-colors outline-none hover:bg-surface-hover hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50',
        active && 'bg-surface-hover text-foreground',
      )}
    >
      {children}
      {active ? <span aria-hidden="true" className="absolute -end-1 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-s-full bg-primary" /> : null}
    </button>
  );
}

function PickerBody({ onPicked }: { onPicked: () => void }) {
  const actions = useActions();
  const all = useAllCandidates();
  const selected = useSelectedCandidate();
  const providers = useDemo((state) => state.snapshot?.providers ?? []);
  const favorites = useDemo((state) => state.favorites);
  const [query, setQuery] = useState('');
  const deferredQuery = useDeferredValue(query);
  const [rail, setRail] = useState<string>(selected?.route.providerId ?? ALL);
  const [highlight, setHighlight] = useState('');
  const searching = query.trim().length > 0;

  const providerIds = useMemo(() => new Set(all.map((candidate) => candidate.route.providerId)), [all]);
  const favoriteKeys = useMemo(() => new Set(favorites), [favorites]);
  const railProviders = providers.filter((provider) => providerIds.has(provider.id));
  const { visible, total } = useMemo(() => {
    const pool = deferredQuery.trim()
      ? all.filter((candidate) => matches(candidate, deferredQuery))
      : rail === ALL
        ? all
        : rail === FAVORITES
          ? all.filter((candidate) => favoriteKeys.has(candidate.key))
          : all.filter((candidate) => candidate.route.providerId === rail);
    return { visible: pool.slice(0, MAX_ROWS), total: pool.length };
  }, [all, deferredQuery, rail, favoriteKeys]);

  const keys = visible.map((candidate) => candidate.key);
  const active = keys.includes(highlight) ? highlight : (selected && keys.includes(selected.key) ? selected.key : (keys[0] ?? ''));

  function choose(candidate: Candidate) {
    actions.select(candidate);
    onPicked();
  }

  function selectRail(id: string) {
    setRail(id);
    setQuery('');
    setHighlight('');
  }

  return (
    <div className="flex h-96 max-h-(--available-height) w-full">
      <div role="group" aria-label="Providers" className="flex w-12 shrink-0 flex-col gap-1 overflow-y-auto border-e border-seam bg-surface-subtle p-1">
        <RailButton label="All providers" active={rail === ALL && !searching} onClick={() => selectRail(ALL)}>
          <SquaresFourIcon className="size-4" />
        </RailButton>
        <RailButton label="Favorites" active={rail === FAVORITES && !searching} onClick={() => selectRail(FAVORITES)}>
          <StarIcon weight="fill" className="size-4" />
        </RailButton>
        <span aria-hidden="true" className="mx-2 my-0.5 h-px bg-seam" />
        {railProviders.map((provider) => (
          <RailButton key={provider.id} label={providerName(provider.id, provider.name)} active={rail === provider.id && !searching} onClick={() => selectRail(provider.id)}>
            <ProviderMark id={provider.id} name={provider.name} className="size-5" />
          </RailButton>
        ))}
      </div>
      <Command shouldFilter={false} loop value={active} onValueChange={setHighlight} className="min-w-0 flex-1 rounded-none bg-transparent p-0">
        <CommandInput
          aria-label="Search models"
          placeholder="Search models…"
          value={query}
          onValueChange={(next) => {
            setQuery(next);
            setHighlight('');
          }}
        />
        <CommandList className="max-h-none flex-1 p-1.5">
          <CommandEmpty>{rail === FAVORITES && !searching ? 'Star a route to pin it here.' : 'No matching routes.'}</CommandEmpty>
          {visible.map((candidate) => {
            const isFavorite = favoriteKeys.has(candidate.key);
            const isSelected = selected?.key === candidate.key;
            return (
              <div key={candidate.key} className="group/pick relative flex items-start rounded-md">
                <CommandItem value={candidate.key} onSelect={() => choose(candidate)} className="min-w-0 flex-1 items-start gap-2.5 py-2 pe-8">
                  <ProviderMark id={candidate.route.providerId} className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                  <span className="grid min-w-0 flex-1 gap-0.5 text-left">
                    <span className="flex items-center gap-1.5">
                      <strong className="truncate text-sm font-medium">{candidate.model.name}</strong>
                      {isSelected ? <CheckIcon className="size-3.5 shrink-0 text-brand-foreground" aria-label="Selected" /> : null}
                    </span>
                    <small className="truncate text-xs text-muted-foreground">{candidate.connection.displayName}</small>
                    <span className="mt-0.5 flex flex-wrap items-center gap-1">
                      <Badge variant={billingVariant(candidate.billingLabel)}>{candidate.billingLabel}</Badge>
                      {!candidate.eligible ? (
                        <Badge variant="warning">
                          <WarningCircleIcon />
                          Needs attention
                        </Badge>
                      ) : null}
                    </span>
                  </span>
                </CommandItem>
                <button
                  type="button"
                  aria-label={isFavorite ? 'Remove from favorites' : 'Add to favorites'}
                  aria-pressed={isFavorite}
                  onClick={() => actions.toggleFavorite(candidate.key)}
                  className={cn(
                    'absolute end-2 top-2 rounded p-0.5 transition-opacity outline-none focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring/50',
                    isFavorite ? 'text-warning' : 'text-muted-foreground opacity-0 group-hover/pick:opacity-100',
                  )}
                >
                  <StarIcon weight={isFavorite ? 'fill' : 'regular'} className="size-3.5" />
                </button>
              </div>
            );
          })}
          {total > MAX_ROWS ? <p className="px-3 py-2 text-xs text-muted-foreground">{`Showing ${MAX_ROWS} of ${total.toLocaleString()} routes. Search to narrow the list.`}</p> : null}
        </CommandList>
      </Command>
    </div>
  );
}

interface ModelPickerProps {
  side?: 'top' | 'bottom';
  align?: 'start' | 'center' | 'end';
  className?: string;
  disabled?: boolean;
}

/** T3-style picker: a provider rail with real marks, search, favorites, and route identity on every row. */
export function ModelPicker({ side = 'bottom', align = 'start', className, disabled }: ModelPickerProps) {
  const [open, setOpen] = useState(false);
  const selected = useSelectedCandidate();
  const all = useAllCandidates();
  return (
    <Popover open={open && !disabled} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={disabled || !all.length}
            aria-label={selected ? `Model: ${selected.model.name}, ${selected.connection.displayName}` : 'Choose a model and account'}
            title={selected ? `${selected.model.name} · ${selected.connection.displayName}` : undefined}
            className={cn('min-w-0 max-w-64 shrink justify-between gap-1.5 text-foreground/75 hover:text-foreground', className)}
          />
        }
      >
        {selected ? <ProviderMark id={selected.route.providerId} className="size-4 shrink-0 text-foreground" /> : null}
        <span className="min-w-0 truncate">{selected ? selected.model.name : 'Choose a model'}</span>
        {selected ? <span className="hidden min-w-0 truncate text-xs text-foreground/60 sm:inline">{selected.connection.displayName}</span> : null}
        <CaretDownIcon aria-hidden="true" className="size-3 shrink-0 opacity-60" />
      </PopoverTrigger>
      <PopoverContent side={side} align={align} sideOffset={6} className="w-[min(30rem,calc(100vw-2rem))] gap-0 overflow-hidden p-0">
        <PickerBody onPicked={() => setOpen(false)} />
      </PopoverContent>
    </Popover>
  );
}
