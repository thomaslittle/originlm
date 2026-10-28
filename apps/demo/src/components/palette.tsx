import { useDeferredValue, useMemo, useState } from 'react';
import { CheckCircleIcon, WarningCircleIcon } from '@phosphor-icons/react';
import { groupCandidates, resolveCandidates } from '@originlm/selector-core';
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { Kbd } from '@/components/ui/kbd';
import { ProviderGlyph } from '@/components/common/provider-glyph';
import { useActions, useDemo } from '@/state/context';

const MAX_RESULTS = 40;

/** Keyboard-first route search. Filtering is done by the selector core, not by cmdk. */
export default function PaletteDialog() {
  const actions = useActions();
  const catalog = useDemo((state) => state.catalog);
  const allowPaid = useDemo((state) => state.allowPaid);
  const providers = useDemo((state) => state.snapshot?.providers);
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState('');
  const deferredQuery = useDeferredValue(query);

  const groups = useMemo(() => {
    if (!catalog) return [];
    const results = resolveCandidates(catalog, { query: deferredQuery, allowPaidFallback: allowPaid }).slice(0, MAX_RESULTS);
    return groupCandidates(results);
  }, [catalog, deferredQuery, allowPaid]);

  // Filtering is external, so cmdk cannot re-highlight on its own: highlight the first
  // result unless the user has moved to another one that is still listed.
  const keys = groups.flatMap((group) => group.candidates.map((candidate) => candidate.key));
  const active = keys.includes(highlight) ? highlight : (keys[0] ?? '');

  return (
    <Dialog open onOpenChange={(open) => actions.patch({ paletteOpen: open })}>
      <DialogContent id="palette-dialog" showCloseButton={false} className="top-[18%] -translate-y-0 gap-0 overflow-hidden p-0 sm:max-w-xl" aria-label="Find a provider model">
        <DialogTitle className="sr-only">Find a route</DialogTitle>
        <DialogDescription className="sr-only">Search every provider, model, account, and native identifier.</DialogDescription>
        <Command shouldFilter={false} loop value={active} onValueChange={setHighlight} className="rounded-none bg-transparent p-0">
          <CommandInput aria-label="Search every model" placeholder="Search every provider, model, account…" value={query}
            onValueChange={(next) => {
              setQuery(next);
              setHighlight('');
            }}
          />
          <CommandList className="max-h-[26rem] p-2">
            <CommandEmpty>No matching routes.</CommandEmpty>
            {groups.map((group) => {
              const provider = providers?.find((item) => item.id === group.providerId);
              const name = provider?.name ?? group.providerId;
              return (
                <CommandGroup key={group.providerId} heading={name}>
                  {group.candidates.map((candidate) => (
                    <CommandItem
                      key={candidate.key}
                      value={candidate.key}
                      onSelect={() => {
                        actions.select(candidate);
                        actions.patch({ paletteOpen: false });
                      }}
                      className="items-center gap-3 py-2"
                    >
                      <ProviderGlyph id={group.providerId} name={name} />
                      <span className="grid min-w-0 flex-1 text-left">
                        <strong className="truncate text-sm font-medium">{candidate.model.name}</strong>
                        <small className="truncate text-xs text-muted-foreground">{`${candidate.connection.displayName} · ${candidate.billingLabel}`}</small>
                      </span>
                      {candidate.eligible ? (
                        <CheckCircleIcon className="size-4 text-success" aria-label="Eligible in snapshot" />
                      ) : (
                        <WarningCircleIcon className="size-4 text-warning" aria-label="Needs attention" />
                      )}
                    </CommandItem>
                  ))}
                </CommandGroup>
              );
            })}
          </CommandList>
          <div className="flex items-center gap-2 border-t px-4 py-2.5 text-xs text-muted-foreground">
            <Kbd>↑ ↓</Kbd>
            <span>Navigate</span>
            <Kbd>Enter</Kbd>
            <span>Inspect</span>
            <Kbd>Esc</Kbd>
            <span>Close</span>
          </div>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
