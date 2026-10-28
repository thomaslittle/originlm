import { FileMagnifyingGlassIcon, FunnelXIcon, StackIcon } from '@phosphor-icons/react';
import type { Candidate } from '@originlm/selector-core';
import { EmptyState } from '@/components/common/empty-state';
import { useActions, useAllCandidates, useDemo } from '@/state/context';
import { ModelRow } from './model-row';

const MAX_ROWS = 50;

export function EmptyRoutes() {
  const all = useAllCandidates();
  const hasRequirements = useDemo((state) => state.required.length > 0);
  if (!all.length) {
    return (
      <EmptyState
        icon={StackIcon}
        title="No routes match the current filters."
        description="Check account and model metadata on an authorized connection to load routes."
      />
    );
  }
  if (hasRequirements) {
    return <EmptyState icon={FunnelXIcon} title="No routes have confirmed all selected capabilities." description="Unknown support is not treated as supported." />;
  }
  return <EmptyState icon={FileMagnifyingGlassIcon} title="No routes match the current filters." description="Clear filters or broaden the search to see more routes." />;
}

/** Bounded: at most 50 rows render; narrowing the search reveals the rest. */
export function ModelList({ candidates, onSelect }: { candidates: readonly Candidate[]; onSelect?: (candidate: Candidate) => void }) {
  const actions = useActions();
  const selectedModelId = useDemo((state) => state.selection?.modelId);
  if (!candidates.length) return <EmptyRoutes />;
  const choose = onSelect ?? actions.select;
  return (
    <div className="grid gap-2 p-2.5">
      {candidates.slice(0, MAX_ROWS).map((candidate) => (
        <ModelRow key={candidate.key} candidate={candidate} selected={selectedModelId === candidate.model.id} onSelect={choose} />
      ))}
      {candidates.length > MAX_ROWS ? (
        <p className="px-3 py-2 text-xs text-muted-foreground">
          {`Showing ${MAX_ROWS} of ${candidates.length.toLocaleString()} matches. Narrow the search to inspect the rest.`}
        </p>
      ) : null}
    </div>
  );
}
