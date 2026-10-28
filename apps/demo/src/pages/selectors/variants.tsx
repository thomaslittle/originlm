import { lazy, Suspense } from 'react';
import { ArrowLeftIcon, ArrowUpIcon, CaretRightIcon, CheckCircleIcon, CommandIcon, MinusCircleIcon, ScrollIcon, StarIcon, WarningCircleIcon, XCircleIcon } from '@phosphor-icons/react';
import type { Model } from 'originlm';
import type { Candidate } from '@originlm/selector-core';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from '@/components/ui/input-group';
import { Item, ItemContent, ItemDescription, ItemTitle } from '@/components/ui/item';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { Skeleton } from '@/components/ui/skeleton';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { CheckRow } from '@/components/common/check-row';
import { ProviderGlyph } from '@/components/common/provider-glyph';
import { providerName } from '@/lib/provider-icons';
import { billingVariant, capNames, formatTokens, known } from '@/lib/format';
import { cn } from '@/lib/utils';
import { useActions, useAllCandidates, useDemo } from '@/state/context';
import { EmptyRoutes, ModelList } from './model-list';
import { CAPABILITY_OPTIONS, PRESETS } from './variants-meta';

// The picker (cmdk + popover) is only needed once this variant is opened.
const ModelPicker = lazy(() => import('@/components/model-picker').then((module) => ({ default: module.ModelPicker })));

interface VariantProps {
  candidates: Candidate[];
}

export function RailVariant({ candidates }: VariantProps) {
  const actions = useActions();
  const all = useAllCandidates();
  const providers = useDemo((state) => state.snapshot?.providers ?? []);
  const provider = useDemo((state) => state.provider);
  const query = useDemo((state) => state.query);
  const favorites = useDemo((state) => state.favorites);

  const perProvider = new Map<string, number>();
  for (const candidate of all) perProvider.set(candidate.route.providerId, (perProvider.get(candidate.route.providerId) ?? 0) + 1);
  const favoriteKeys = new Set(favorites);
  const groups: { id: string; name: string; count: number }[] = [
    { id: 'all', name: 'All connections', count: all.length },
    { id: 'favorites', name: 'Favorites', count: favorites.length },
    ...providers.map((item) => ({ id: item.id, name: item.name, count: perProvider.get(item.id) ?? 0 })),
  ];
  const visible =
    provider === 'favorites'
      ? candidates.filter((candidate) => favoriteKeys.has(candidate.key))
      : query.trim() || provider === 'all'
        ? candidates
        : candidates.filter((candidate) => candidate.route.providerId === provider);
  const heading = query ? 'Search all connections' : provider === 'favorites' ? 'Favorite routes' : (providers.find((item) => item.id === provider)?.name ?? 'All connections');

  return (
    <div className="grid min-h-[30rem] grid-cols-[3.5rem_minmax(0,1fr)] sm:grid-cols-[12rem_minmax(0,1fr)]">
      <div data-testid="provider-rail" className="flex flex-col gap-1 border-r bg-background/30 p-2">
        {groups.map((group) => (
          <Button
            key={group.id}
            variant="ghost"
            data-provider={group.id}
            aria-label={group.name}
            aria-pressed={provider === group.id}
            onClick={() => actions.patch({ provider: group.id })}
            className={cn(
              'h-auto w-full justify-start gap-2.5 px-2 py-2 text-left text-xs text-muted-foreground',
              provider === group.id && 'bg-primary/10 text-foreground ring-1 ring-primary/30 hover:bg-primary/10',
            )}
          >
            {group.id === 'favorites' ? (
              <span aria-hidden="true" className="grid size-6 shrink-0 place-items-center rounded-md bg-warning/15 text-warning">
                <StarIcon className="size-3.5" />
              </span>
            ) : group.id === 'all' ? (
              <span aria-hidden="true" className="grid size-6 shrink-0 place-items-center rounded-md bg-primary/15 text-brand-foreground">
                <CommandIcon className="size-3.5" />
              </span>
            ) : (
              <ProviderGlyph id={group.id} name={group.name} size="sm" />
            )}
            <span className="hidden min-w-0 flex-1 truncate sm:block">{group.name}</span>
            <span className="ml-auto hidden font-mono text-[0.65rem] tabular-nums text-muted-foreground sm:block">{group.count}</span>
          </Button>
        ))}
      </div>
      <div className="min-w-0">
        <div className="flex items-center justify-between gap-2.5 border-b px-4 py-3.5 text-sm">
          <strong>{heading}</strong>
          <Badge variant="outline">{String(visible.length)}</Badge>
        </div>
        <ModelList candidates={visible} />
      </div>
    </div>
  );
}

export function CompactVariant() {
  return (
    <div className="grid gap-5 p-6">
      <div className="rounded-2xl border bg-surface-subtle p-5">
        <h3 className="text-sm font-semibold">A model picker that fits anywhere</h3>
        <p className="mt-1 mb-4 text-xs text-muted-foreground">
          Provider, account, and model stay together, even in one compact control. The popover keeps a provider rail, search, and favorites.
        </p>
        <InputGroup className="bg-background/50">
          <InputGroupTextarea aria-label="Prompt preview" rows={4} placeholder="Your application’s prompt goes here. This preview does not send it." />
          <InputGroupAddon align="block-end">
            <Suspense fallback={<Skeleton className="h-7 w-44" />}>
              <ModelPicker side="bottom" />
            </Suspense>
            <InputGroupButton type="button" size="icon-sm" variant="default" aria-label="Send (preview only)" disabled className="ml-auto rounded-full">
              <ArrowUpIcon />
            </InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </div>
      <p className="text-xs text-muted-foreground">The configuration panel uses the same native option descriptors as every other selector.</p>
    </div>
  );
}

export function PaletteVariant({ candidates }: VariantProps) {
  const actions = useActions();
  return (
    <div className="grid gap-4 p-6">
      <div className="relative overflow-hidden rounded-2xl border bg-linear-to-br from-primary/10 via-background/40 to-chart-2/10 p-6">
        <Badge variant="brand" className="mb-4">
          Keyboard first
        </Badge>
        <h3 className="text-base font-semibold">Find a route without leaving the keyboard.</h3>
        <p className="mt-1 mb-4 max-w-md text-sm text-muted-foreground">Search model names, provider names, accounts, and native identifiers.</p>
        <div className="flex flex-wrap items-center gap-3">
          <Button onClick={() => actions.patch({ paletteOpen: true })}>
            <CommandIcon data-icon="inline-start" />
            Open command palette
          </Button>
          <KbdGroup>
            <Kbd>Ctrl</Kbd>
            <span className="text-xs text-muted-foreground">/</span>
            <Kbd>⌘</Kbd>
            <span className="text-xs text-muted-foreground">+</span>
            <Kbd>K</Kbd>
          </KbdGroup>
        </div>
      </div>
      <ModelList candidates={candidates.slice(0, 4)} />
    </div>
  );
}

export function CapabilityVariant({ candidates }: VariantProps) {
  const actions = useActions();
  const required = useDemo((state) => state.required);
  return (
    <div>
      <div className="flex flex-wrap gap-2 border-b px-4 py-3.5">
        {CAPABILITY_OPTIONS.map((option) => (
          <CheckRow
            key={option.id}
            chip
            label={option.label}
            checked={required.includes(option.id)}
            onCheckedChange={(checked) => actions.patch({ required: checked ? [...required, option.id] : required.filter((id) => id !== option.id) })}
          />
        ))}
      </div>
      <ModelList candidates={candidates} />
    </div>
  );
}

/** Capabilities shown in the table, as icons. Anything the provider did not report is a plain dash. */
const COMPARE_CAPABILITIES: { id: string; title: string }[] = [
  { id: 'input.image', title: 'Images' },
  { id: 'tools', title: 'Tools' },
];

const SUPPORT_LABEL: Record<string, string> = { supported: 'Supported', conditional: 'Conditional', unsupported: 'Not supported' };

function SupportIcon({ capability, support, conditions }: { capability: string; support?: string; conditions?: readonly string[] }) {
  if (!support || !SUPPORT_LABEL[support]) {
    return (
      <span title={`${capability}: not reported by the provider`} className="text-muted-foreground">
        <span aria-hidden="true">{'\u2014'}</span>
        <span className="sr-only">{`${capability}: not reported`}</span>
      </span>
    );
  }
  const Glyph = support === 'supported' ? CheckCircleIcon : support === 'conditional' ? WarningCircleIcon : XCircleIcon;
  const tone = support === 'supported' ? 'text-success' : support === 'conditional' ? 'text-warning' : 'text-destructive';
  const detail = conditions?.length ? ` (requires ${conditions.join(', ')})` : '';
  return (
    <span title={`${capability}: ${SUPPORT_LABEL[support]}${detail}`} className={cn('inline-flex', tone)}>
      <Glyph weight="fill" className="size-4.5" aria-hidden="true" />
      <span className="sr-only">{`${capability}: ${SUPPORT_LABEL[support]}${detail}`}</span>
    </span>
  );
}

export function CompareVariant({ candidates }: VariantProps) {
  const actions = useActions();
  const providers = useDemo((state) => state.snapshot?.providers ?? []);
  const selectedModelId = useDemo((state) => state.selection?.modelId);
  const rows = candidates.slice(0, 50);
  const contextOf = (candidate: Candidate) => formatTokens(known((candidate.model as unknown as Model).contextWindow));
  const hasContext = rows.some((candidate) => contextOf(candidate));
  const capabilityColumns = COMPARE_CAPABILITIES.filter((column) => rows.some((candidate) => candidate.model.capabilities.some((item) => item.id === column.id)));
  return (
    <Table className="table-fixed">
      <TableHeader>
        <TableRow className="bg-surface-subtle hover:bg-surface-subtle">
          <TableHead className="h-11 w-[42%] px-3 text-xs font-semibold text-muted-foreground">Model / route</TableHead>
          <TableHead className="h-11 w-[18%] px-3 text-xs font-semibold text-muted-foreground">Billing</TableHead>
          {hasContext ? <TableHead className="h-11 w-[10%] px-3 text-xs font-semibold text-muted-foreground">Context</TableHead> : null}
          {capabilityColumns.map((column) => (
            <TableHead key={column.id} className="h-11 w-[8%] px-3 text-center text-xs font-semibold text-muted-foreground">
              {column.title}
            </TableHead>
          ))}
          <TableHead className="h-11 w-[14%] px-3 text-xs font-semibold text-muted-foreground">Status</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {candidates.length ? (
          rows.map((candidate) => {
            const provider = providers.find((item) => item.id === candidate.route.providerId);
            const context = contextOf(candidate);
            const providerLabel = providerName(candidate.route.providerId, provider?.name);
            // "Claude Code · Default profile", not "Claude Code · Claude Code · Default profile".
            const account = candidate.connection.displayName.toLowerCase().startsWith(providerLabel.toLowerCase()) ? candidate.connection.displayName : `${providerLabel} \u00b7 ${candidate.connection.displayName}`;
            const selected = selectedModelId === candidate.model.id;
            return (
              <TableRow
                key={candidate.key}
                data-state={selected ? 'selected' : undefined}
                onClick={() => actions.select(candidate)}
                className="cursor-pointer hover:bg-accent-subtle data-[state=selected]:bg-accent-surface"
              >
                <TableCell className="px-3 py-3">
                  <div className="flex min-w-0 items-center gap-2.5">
                    <ProviderGlyph id={candidate.route.providerId} name={provider?.name} />
                    <span className="grid min-w-0">
                      <button
                        type="button"
                        data-testid={candidate.model.id}
                        aria-pressed={selected}
                        onClick={(event) => {
                          event.stopPropagation();
                          actions.select(candidate);
                        }}
                        className="truncate text-left text-sm font-semibold outline-none hover:underline focus-visible:underline"
                      >
                        {candidate.model.name}
                      </button>
                      <small className="truncate text-xs text-muted-foreground">{account}</small>
                    </span>
                  </div>
                </TableCell>
                <TableCell className="px-3">
                  <Badge variant={billingVariant(candidate.billingLabel)} className="max-w-full truncate">
                    {candidate.billingLabel}
                  </Badge>
                </TableCell>
                {hasContext ? <TableCell className="px-3 font-mono text-xs tabular-nums">{context ?? <span className="text-muted-foreground">{'\u2014'}</span>}</TableCell> : null}
                {capabilityColumns.map((column) => {
                  const capability = candidate.model.capabilities.find((item) => item.id === column.id);
                  return (
                    <TableCell key={column.id} className="px-3 text-center">
                      <SupportIcon capability={column.title} support={capability?.support} conditions={capability?.conditions} />
                    </TableCell>
                  );
                })}
                <TableCell className="px-3">
                  <Badge variant={candidate.eligible ? 'success' : 'warning'}>{candidate.eligible ? 'Eligible' : 'Needs attention'}</Badge>
                </TableCell>
              </TableRow>
            );
          })
        ) : (
          <TableRow>
            <TableCell colSpan={6} className="p-4">
              <EmptyRoutes />
            </TableCell>
          </TableRow>
        )}
      </TableBody>
    </Table>
  );
}

const ALLOWANCE_NOTES: Record<string, string> = {
  'Included allowance': 'Existing allowance, not unlimited or free.',
  'Metered access': 'A separate billing decision is required.',
};

export function AllowanceVariant({ candidates }: VariantProps) {
  const labels = [...new Set(candidates.map((candidate) => candidate.billingLabel))];
  if (!labels.length) {
    return (
      <div className="p-6">
        <EmptyRoutes />
      </div>
    );
  }
  return (
    <div className="divide-y">
      {labels.map((label) => (
        <section key={label} className="px-3 py-5">
          <div className="flex items-center gap-2.5 px-2">
            <Badge variant={billingVariant(label)}>{label}</Badge>
            <h3 className="sr-only">{label}</h3>
            <p className="text-xs text-muted-foreground">{ALLOWANCE_NOTES[label] ?? 'Use the evidence reported by this connection.'}</p>
          </div>
          <ModelList candidates={candidates.filter((candidate) => candidate.billingLabel === label)} />
        </section>
      ))}
    </div>
  );
}

export function PresetVariant({ candidates }: VariantProps) {
  const actions = useActions();
  return (
    <div>
      <div className="grid grid-cols-1 gap-2.5 p-4 sm:grid-cols-2">
        {PRESETS.map((preset) => (
          <Item
            key={preset.name}
            variant="outline"
            render={<button type="button" onClick={() => actions.patch({ required: preset.required, onlyEligible: true })} />}
            className="cursor-pointer text-left hover:border-primary/40 hover:bg-accent-subtle"
          >
            <ItemContent>
              <ItemTitle className="text-sm font-semibold">{preset.name}</ItemTitle>
              <ItemDescription>{preset.required.map((id) => capNames[id] ?? id).join(' + ')}</ItemDescription>
            </ItemContent>
          </Item>
        ))}
      </div>
      <p className="px-4 pb-3 text-xs text-muted-foreground">
        Presets filter requirements. They do not automatically select an account, start a turn, or claim a model is best.
      </p>
      <ModelList candidates={candidates} />
    </div>
  );
}

const STEPS = 4;

export function MobileVariant({ candidates }: VariantProps) {
  const actions = useActions();
  const providers = useDemo((state) => state.snapshot?.providers ?? []);
  const connections = useDemo((state) => state.snapshot?.connections ?? []);
  const step = useDemo((state) => state.mobileStep);
  const mobileProvider = useDemo((state) => state.mobileProvider);
  const mobileConnection = useDemo((state) => state.mobileConnection);

  let body;
  if (step === 0) {
    body = providers.map((provider) => (
      <DrillRow key={provider.id} glyphId={provider.id} label={provider.name} onClick={() => actions.patch({ mobileProvider: provider.id, mobileStep: 1 })} />
    ));
  } else if (step === 1) {
    body = connections
      .filter((connection) => connection.providerId === mobileProvider)
      .map((connection) => (
        <DrillRow key={connection.id} glyphId={connection.providerId} label={connection.displayName} onClick={() => actions.patch({ mobileConnection: connection.id, mobileStep: 2 })} />
      ));
  } else if (step === 2) {
    body = (
      <ModelList
        candidates={candidates.filter((candidate) => candidate.connection.id === mobileConnection)}
        onSelect={(candidate) => {
          actions.select(candidate);
          actions.patch({ mobileStep: 3 });
        }}
      />
    );
  } else {
    body = (
      <div className="grid gap-3 p-4 text-xs">
        <Badge variant="brand" className="w-fit">
          Route selected
        </Badge>
        <h3 className="text-sm font-semibold">Now choose its native options.</h3>
        <p className="text-muted-foreground">The configuration panel below retains the exact account, model identifier, and supported settings.</p>
        <Button variant="outline" size="sm" className="w-fit" onClick={() => document.querySelector('#configuration-panel')?.scrollIntoView({ behavior: 'smooth' })}>
          <ScrollIcon data-icon="inline-start" />
          Review configuration
        </Button>
      </div>
    );
  }

  return (
    <div className="flex justify-center p-4 sm:p-8">
      <div className="w-full max-w-[22rem] overflow-hidden rounded-[2rem] border-4 border-border bg-background shadow-2xl">
        <div className="mx-auto mt-2 h-1.5 w-16 rounded-full bg-border" aria-hidden="true" />
        <div className="flex items-center justify-between gap-2.5 border-b px-4 py-3 text-xs">
          {step ? (
            <Button variant="outline" size="sm" onClick={() => actions.patch({ mobileStep: Math.max(0, step - 1) })}>
              <ArrowLeftIcon data-icon="inline-start" />
              Back
            </Button>
          ) : (
            <span className="font-medium">Choose a provider</span>
          )}
          <span className="flex items-center gap-1" aria-hidden="true">
            {Array.from({ length: STEPS }, (_, index) => (
              <span key={index} className={cn('h-1.5 rounded-full transition-colors', index === step ? 'w-5 bg-primary' : 'w-1.5 bg-border')} />
            ))}
          </span>
          <Badge variant="outline">{`${step + 1} / ${STEPS}`}</Badge>
        </div>
        <div className="grid min-h-[17rem] content-start gap-1.5 p-2">{body}</div>
      </div>
    </div>
  );
}

function DrillRow({ glyphId, label, onClick }: { glyphId: string; label: string; onClick: () => void }) {
  return (
    <Button variant="outline" onClick={onClick} className="h-auto w-full justify-start gap-3 px-3 py-3 text-left">
      <ProviderGlyph id={glyphId} name={label} />
      <span className="flex-1 truncate">{label}</span>
      <CaretRightIcon className="text-muted-foreground" aria-hidden="true" />
    </Button>
  );
}
