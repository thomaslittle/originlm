import { useRouterState } from '@tanstack/react-router';
import type { Page } from '@/state/types';

export interface PageMeta {
  page: Page;
  /** Real URL path. Every path here is also served by the fixture and native asset servers. */
  path: '/overview' | '/connections' | '/models' | '/usage' | '/selector-lab' | '/playground' | '/diagnostics' | '/settings';
  description: string;
}

export const PAGES: readonly PageMeta[] = [
  { page: 'Overview', path: '/overview', description: 'Your AI, at a glance.' },
  { page: 'Connections', path: '/connections', description: 'Every account, profile, and access route stays distinct.' },
  { page: 'Models', path: '/models', description: 'A route-aware catalog. Native options, never guessed settings.' },
  { page: 'Usage', path: '/usage', description: 'The right units. The right windows. No fictional totals.' },
  { page: 'Selector lab', path: '/selector-lab', description: 'One selection engine. Eight ways to make it yours.' },
  { page: 'Playground', path: '/playground', description: 'Review session controls. A turn needs an explicit route and authorization.' },
  { page: 'Settings', path: '/settings', description: 'Local prices, with explicit sources and review dates.' },
  { page: 'Diagnostics', path: '/diagnostics', description: 'Understand the evidence behind every public response.' },
];

export const DEFAULT_PATH = '/overview';

export const pathFor = (page: Page): PageMeta['path'] => (PAGES.find((meta) => meta.page === page) ?? PAGES[4]).path;

export const metaForPath = (pathname: string): PageMeta | undefined => PAGES.find((meta) => meta.path === pathname);

/** The current page, derived from the URL: the router is the only source of truth. */
export function usePageMeta(): PageMeta {
  return useRouterState({ select: (state) => metaForPath(state.location.pathname) ?? PAGES[4] });
}
