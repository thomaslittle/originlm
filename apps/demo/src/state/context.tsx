import { createContext, use, useDeferredValue, useMemo, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { resolveCandidates, type Candidate, type Catalog } from '@originlm/selector-core';
import type { DemoActions, DemoStore } from './store';
import type { DemoState } from './types';

const DemoContext = createContext<DemoStore | null>(null);

export function DemoProvider({ demo, children }: { demo: DemoStore; children: ReactNode }) {
  return <DemoContext value={demo}>{children}</DemoContext>;
}

function useDemoStore(): DemoStore {
  const demo = use(DemoContext);
  if (!demo) throw new Error('DemoProvider is missing');
  return demo;
}

/** Subscribe to one slice. Return primitives or stable references from the selector. */
export function useDemo<T>(selector: (state: DemoState) => T): T {
  return useStore(useDemoStore().store, selector);
}

/** Actions are created once per store, so this never re-renders its caller. */
export function useActions(): DemoActions {
  return useDemoStore().actions;
}

const NONE: Candidate[] = [];

// One resolved list per (catalog, billing consent), shared by every consumer in a render.
const resolvedCache = new WeakMap<Catalog, Map<boolean, Candidate[]>>();

function resolveAll(catalog: Catalog, allowPaid: boolean): Candidate[] {
  let byConsent = resolvedCache.get(catalog);
  if (!byConsent) resolvedCache.set(catalog, (byConsent = new Map()));
  let list = byConsent.get(allowPaid);
  if (!list) byConsent.set(allowPaid, (list = resolveCandidates(catalog, { allowPaidFallback: allowPaid })));
  return list;
}

/** Every route in the catalog, unfiltered. */
export function useAllCandidates(): Candidate[] {
  const catalog = useDemo((state) => state.catalog);
  const allowPaid = useDemo((state) => state.allowPaid);
  return catalog ? resolveAll(catalog, allowPaid) : NONE;
}

/** Routes matching the current search and requirement filters. */
export function useCandidates(): Candidate[] {
  const catalog = useDemo((state) => state.catalog);
  const allowPaid = useDemo((state) => state.allowPaid);
  const query = useDemo((state) => state.query);
  const required = useDemo((state) => state.required);
  const onlyEligible = useDemo((state) => state.onlyEligible);
  // Typing stays responsive on very large catalogs: the input updates immediately
  // and the list catches up at lower priority.
  const deferredQuery = useDeferredValue(query);
  return useMemo(
    () =>
      catalog
        ? resolveCandidates(catalog, { query: deferredQuery, requiredCapabilities: required, eligibleOnly: onlyEligible, allowPaidFallback: allowPaid })
        : NONE,
    [catalog, deferredQuery, required, onlyEligible, allowPaid],
  );
}

/** The candidate matching the current selection, when it is in the catalog. */
export function useSelectedCandidate(): Candidate | undefined {
  const all = useAllCandidates();
  const selection = useDemo((state) => state.selection);
  return useMemo(
    () => all.find((candidate) => candidate.route.id === selection?.routeId && candidate.model.id === selection?.modelId),
    [all, selection],
  );
}
