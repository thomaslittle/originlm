import type { ReactNode } from 'react';
import { CheckCircleIcon, CursorClickIcon, StarIcon, WarningCircleIcon } from '@phosphor-icons/react';
import { Link } from '@tanstack/react-router';
import type { Candidate, Selection } from '@originlm/selector-core';
import type { Model } from 'originlm';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { FieldGroup } from '@/components/ui/field';
import { CheckRow } from '@/components/common/check-row';
import { CodeBlock } from '@/components/common/code-block';
import { EmptyState } from '@/components/common/empty-state';
import { ProviderGlyph } from '@/components/common/provider-glyph';
import { billingVariant, known, reasonLabel } from '@/lib/format';
import { cn } from '@/lib/utils';
import { useActions, useAllCandidates, useDemo, useSelectedCandidate } from '@/state/context';
import { OptionField } from './option-field';

function Fact({ term, children, mono }: { term: string; children: ReactNode; mono?: boolean }) {
  return (
    <>
      <dt className="text-muted-foreground">{term}</dt>
      <dd className={cn('text-right break-words', mono && 'font-mono text-[0.7rem] break-all')}>{children}</dd>
    </>
  );
}

function EmptyPanel() {
  const all = useAllCandidates();
  return (
    <aside id="configuration-panel" className="surface grid content-start gap-3 p-5 lg:sticky lg:top-20">
      <EmptyState
        icon={CursorClickIcon}
        title={all.length ? 'Choose a model' : 'No model route available'}
        description={all.length ? 'Inspect a route to see its native settings and execution identity.' : 'Check account and model metadata on an authorized connection to load a catalog.'}
      />
      <Button variant="outline" size="sm" render={<Link to="/connections" />} nativeButton={false}>
        Open Connections
      </Button>
    </aside>
  );
}

function PanelHeader({ selected }: { selected: Candidate }) {
  const actions = useActions();
  const isFavorite = useDemo((state) => state.favorites.includes(selected.key));
  const { model, route, connection } = selected;
  return (
    <div className="relative border-b bg-linear-to-br from-accent-subtle via-transparent to-transparent p-5">
      <div className="mb-3 flex items-center justify-between gap-2">
        <span className="text-[0.65rem] font-semibold text-muted-foreground">Selected route</span>
        <Button
          variant="ghost"
          size="sm"
          data-testid="favorite-current"
          aria-label={isFavorite ? 'Remove current route from favorites' : 'Save current route to favorites'}
          onClick={() => actions.toggleFavorite(selected.key)}
          className={cn('text-xs', isFavorite ? 'text-warning' : 'text-muted-foreground')}
        >
          <StarIcon data-icon="inline-start" weight={isFavorite ? 'fill' : 'regular'} />
          {isFavorite ? 'Saved' : 'Save'}
        </Button>
      </div>
      <div className="flex items-start gap-3">
        <ProviderGlyph id={route.providerId} name={route.displayName || route.providerId} size="lg" />
        <div className="min-w-0">
          <h2 className="text-lg leading-tight font-semibold tracking-tight break-words">{model.name}</h2>
          <p className="mt-0.5 text-sm text-muted-foreground">{connection.displayName}</p>
        </div>
      </div>
      <div className="mt-4 flex flex-wrap gap-1.5">
        <Badge variant={billingVariant(selected.billingLabel)}>{selected.billingLabel}</Badge>
        <Badge variant={selected.eligible ? 'success' : 'warning'}>
          {selected.eligible ? <CheckCircleIcon aria-hidden="true" /> : <WarningCircleIcon aria-hidden="true" />}
          {selected.eligible ? 'Eligible in snapshot' : 'Needs attention'}
        </Badge>
      </div>
    </div>
  );
}

function RouteFacts({ selected }: { selected: Candidate }) {
  const providers = useDemo((state) => state.snapshot?.providers);
  const synthetic = useDemo((state) => state.mode === 'synthetic');
  const { model, route } = selected;
  const contextWindow = known((model as unknown as Model).contextWindow);
  return (
    <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2.5 text-xs">
      <Fact term="Provider">{providers?.find((provider) => provider.id === route.providerId)?.name ?? route.providerId}</Fact>
      <Fact term="Native model" mono>
        {model.nativeId}
      </Fact>
      <Fact term="Context">{contextWindow ? `${Number(contextWindow).toLocaleString()} · reported` : 'Not reported'}</Fact>
      <Fact term="Source">{synthetic ? 'Synthetic example' : 'Connected runtime'}</Fact>
    </dl>
  );
}

function NativeOptions({ selected, selection }: { selected: Candidate; selection: Selection }) {
  const actions = useActions();
  const sensitiveConsent = useDemo((state) => state.sensitiveConsent);
  const { options } = selected.model;
  const hasSensitiveOptions = options.some((option) => option.billingSensitive || option.permissionSensitive);
  return (
    <section className="border-t pt-4">
      <h3 className="mb-3 text-sm font-semibold">Native options</h3>
      {options.length ? (
        <FieldGroup className="gap-4">
          {options.map((descriptor) => (
            <OptionField key={descriptor.id} descriptor={descriptor} value={selection.options[descriptor.id]} onSet={actions.setOption} />
          ))}
        </FieldGroup>
      ) : (
        <p className="text-xs text-muted-foreground">This model advertises no editable options. Nothing is assumed.</p>
      )}
      {hasSensitiveOptions ? (
        <CheckRow className="mt-4" label="Allow sensitive configuration changes" checked={sensitiveConsent} onCheckedChange={(checked) => actions.patch({ sensitiveConsent: checked })} />
      ) : null}
    </section>
  );
}

/** The exact route, native options, and payload for the current selection. Sticky beside long lists. */
export function ConfigurationPanel() {
  const selected = useSelectedCandidate();
  const selection = useDemo((state) => state.selection);
  if (!selected || !selection) return <EmptyPanel />;
  return (
    <aside id="configuration-panel" className="surface min-w-0 overflow-hidden lg:sticky lg:top-20">
      <PanelHeader selected={selected} />
      <div className="grid gap-5 p-5">
        <RouteFacts selected={selected} />
        <NativeOptions selected={selected} selection={selection} />
        {selected.reasons.length ? (
          <div className="rounded-xl border border-warning/25 bg-warning/10 p-3.5 text-xs text-warning">{selected.reasons.map(reasonLabel).join(' · ')}</div>
        ) : null}
        <details className="border-t pt-3.5">
          <summary className="cursor-pointer text-xs text-muted-foreground select-none hover:text-foreground">Exact selection payload</summary>
          <CodeBlock id="selection-json" className="mt-3">
            {JSON.stringify(selection, null, 2)}
          </CodeBlock>
        </details>
        <p className="text-[0.7rem] leading-relaxed text-muted-foreground">
          Selection is not authorization. The runtime must recheck access, settings, and permissions before execution.
        </p>
      </div>
    </aside>
  );
}
