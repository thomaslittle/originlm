import { MagnifyingGlassIcon, XIcon } from '@phosphor-icons/react';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group';
import { CheckRow } from '@/components/common/check-row';
import { useActions, useCandidates, useDemo } from '@/state/context';
import type { VariantId } from '@/state/types';
import { ConfigurationPanel } from './configuration-panel';
import { AllowanceVariant, CapabilityVariant, CompactVariant, CompareVariant, MobileVariant, PaletteVariant, PresetVariant, RailVariant } from './variants';
import { VARIANTS } from './variants-meta';

function VariantTabs() {
  const actions = useActions();
  const variant = useDemo((state) => state.variant);
  return (
    <ToggleGroup
      aria-label="Selector variants"
      value={[variant]}
      // A single selection can only be replaced, never cleared: ignore the empty toggle-off.
      onValueChange={(next) => next[0] && actions.patch({ variant: next[0] as VariantId })}
      className="mb-5 grid w-full grid-cols-2 gap-2 sm:grid-cols-4 2xl:grid-cols-8"
    >
      {VARIANTS.map(({ id, label, description, icon: Icon }, index) => (
        <ToggleGroupItem
          key={id}
          value={id}
          data-variant={id}
          title={description}
          className="group/tab h-auto justify-start gap-2.5 rounded-xl border bg-card/50 px-3 py-2.5 text-left text-xs font-medium text-muted-foreground hover:border-primary/30 hover:bg-card hover:text-foreground aria-pressed:border-primary/50 aria-pressed:bg-accent-surface aria-pressed:text-foreground"
        >
          <span className="grid size-7 shrink-0 place-items-center rounded-lg bg-muted text-muted-foreground transition-colors group-aria-pressed/tab:bg-primary group-aria-pressed/tab:text-primary-foreground">
            <Icon className="size-3.5" aria-hidden="true" />
          </span>
          <span className="grid min-w-0 leading-tight">
            <small className="font-mono text-[0.65rem] tabular-nums text-muted-foreground">{String(index + 1).padStart(2, '0')}</small>
            <span className="truncate">{label}</span>
          </span>
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}

function Toolbar() {
  const actions = useActions();
  const query = useDemo((state) => state.query);
  const onlyEligible = useDemo((state) => state.onlyEligible);
  const allowPaid = useDemo((state) => state.allowPaid);
  return (
    <div className="mb-4 flex flex-wrap items-center gap-x-5 gap-y-3">
      <InputGroup className="h-10 min-w-60 flex-1 bg-card/50 md:max-w-xl">
        <InputGroupAddon>
          <MagnifyingGlassIcon aria-hidden="true" />
        </InputGroupAddon>
        <InputGroupInput
          id="catalog-query"
          type="search"
          aria-label="Search models"
          placeholder="Search models, providers, accounts…"
          value={query}
          onChange={(event) => actions.patch({ query: event.target.value })}
        />
        {query ? (
          <InputGroupAddon align="inline-end">
            <InputGroupButton size="icon-xs" aria-label="Clear search" onClick={() => actions.patch({ query: '' })}>
              <XIcon />
            </InputGroupButton>
          </InputGroupAddon>
        ) : null}
      </InputGroup>
      <CheckRow label="Only eligible routes" checked={onlyEligible} onCheckedChange={(checked) => actions.patch({ onlyEligible: checked })} />
      <CheckRow label="Include metered routes" checked={allowPaid} onCheckedChange={(checked) => actions.patch({ allowPaid: checked })} />
    </div>
  );
}

function SelectorView({ catalog }: { catalog: boolean }) {
  const storedVariant = useDemo((state) => state.variant);
  const candidates = useCandidates();
  // The Models page is the catalog-first view: always the side-by-side comparison.
  const variant = catalog ? 'compare' : storedVariant;
  let content;
  switch (variant) {
    case 'rail':
      content = <RailVariant candidates={candidates} />;
      break;
    case 'compact':
      content = <CompactVariant />;
      break;
    case 'palette':
      content = <PaletteVariant candidates={candidates} />;
      break;
    case 'capability':
      content = <CapabilityVariant candidates={candidates} />;
      break;
    case 'compare':
      content = <CompareVariant candidates={candidates} />;
      break;
    case 'allowance':
      content = <AllowanceVariant candidates={candidates} />;
      break;
    case 'preset':
      content = <PresetVariant candidates={candidates} />;
      break;
    default:
      content = <MobileVariant candidates={candidates} />;
  }
  return (
    <div id="variant-view" data-current-variant={variant} className="surface min-w-0 overflow-hidden">
      {content}
    </div>
  );
}

/** Selector lab (eight variants) or, with `catalog`, the Models page (comparison table). */
export function SelectorsPage({ catalog = false }: { catalog?: boolean }) {
  const hasSelection = useDemo((state) => state.selection !== null);
  // The catalog table gets the full width until a route is picked; the lab keeps its panel for guidance.
  const showPanel = !catalog || hasSelection;
  return (
    <div>
      {catalog ? null : <VariantTabs />}
      <Toolbar />
      <div className={showPanel ? 'grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_21rem]' : 'grid gap-5'}>
        <SelectorView catalog={catalog} />
        {showPanel ? <ConfigurationPanel /> : null}
      </div>
    </div>
  );
}
