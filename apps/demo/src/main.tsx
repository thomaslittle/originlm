import './styles.css';
import { createRoot } from 'react-dom/client';
import { LocalAIError, type LocalAIClient } from 'originlm';
import { App } from '@/components/app';
import { Pairing } from '@/components/pairing';
import { connectRuntime } from '@/lib/runtime';
import { applyStoredTheme } from '@/lib/theme';
import { createAppRouter, warmRoutes } from '@/router';
import { pathFor } from '@/lib/pages';
import { createDemoStore } from '@/state/store';
import type { DemoFactory, FixtureFlags } from '@/state/types';

type Root = ReturnType<typeof createRoot>;

function fixtureFlagsFromParams(params: URLSearchParams): FixtureFlags {
  const mode = params.get('mode');
  return {
    staleAuth: params.has('staleAuth'),
    extraProfiles: params.has('extraProfiles'),
    forceMode: mode === 'live' || mode === 'synthetic' ? mode : undefined,
    probeResults: params.has('probeResults'),
  };
}

/** The host's self-description, or null when there is no reachable, well-formed runtime. */
async function fetchJson(path: string): Promise<Record<string, unknown> | null> {
  try {
    const response = await fetch(path, { credentials: 'omit', cache: 'no-store', redirect: 'error' });
    return response.ok ? ((await response.json()) as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function mount(root: Root, factory: Omit<DemoFactory, 'navigate'>): void {
  const router = createAppRouter();
  const demo = createDemoStore({ ...factory, navigate: (page) => void router.navigate({ to: pathFor(page) }) });
  root.render(<App demo={demo} router={router} />);
  warmRoutes(router);
}

/** Real data: a client paired with the native helper. Nothing here is sample data. */
function mountLive(root: Root, client: LocalAIClient): void {
  mount(root, { connectFixture: async () => client, connectLive: connectRuntime, initialClient: client, initialMode: 'live' });
}

/**
 * Loopback synthetic fixture served by the test/fixture server. Every record comes from the
 * fixture service; scripted scenarios are opt-in through the hash fragment (`#demo-fixture&staleAuth=1`).
 */
async function mountFixtureServer(root: Root, params: URLSearchParams): Promise<void> {
  const bootstrap = await fetchJson('/fixture/bootstrap');
  if (!bootstrap) throw new Error('RUNTIME_UNAVAILABLE');
  const client = await connectRuntime(window.location.origin, String(bootstrap.sessionToken));
  mount(root, {
    onBulkProbe: params.has('bulkProbeHook') ? () => void (document.body.dataset.bulkProbe = 'started') : undefined,
    connectFixture: async () => client,
    connectLive: async () => {
      throw new LocalAIError('TRANSPORT_FAILED');
    },
    initialClient: client,
    initialMode: 'synthetic',
    fixtureFlags: params.has('demo-fixture') ? fixtureFlagsFromParams(params) : undefined,
  });
}

/**
 * The fallback: example data, used only when there is no paired native runtime. It is loaded on demand,
 * labeled "Synthetic showcase" everywhere, and offers a way back to pairing.
 */
async function mountExampleData(root: Root): Promise<void> {
  const { openExampleData } = await import('@/state/example-data');
  mount(root, {
    connectFixture: openExampleData,
    connectLive: connectRuntime,
    initialMode: 'synthetic',
    pair: () => window.location.reload(),
  });
}

async function boot(): Promise<void> {
  applyStoredTheme();
  const root = createRoot(document.querySelector('#app') ?? document.body);
  // Scripted flags live in the hash fragment so they never reach the server:
  // asset routes reject query strings on every path except /v1/events.
  const params = new URLSearchParams(window.location.hash.replace(/^#/, ''));
  const info = await fetchJson('/runtime/info');

  // Scripted checks and the fixture server: the host announces `synthetic`, or the hash forces it.
  if (params.has('demo-fixture') || info?.mode === 'synthetic') return mountFixtureServer(root, params);

  // Preferred: a native runtime that requires pairing. Real, detected accounts and models, after an explicit pair.
  if (info?.mode === 'live' && info.pairingRequired === true) {
    root.render(<Pairing onPaired={(client) => mountLive(root, client)} onExampleData={() => void mountExampleData(root)} />);
    return;
  }

  // No usable native runtime: fall back to clearly labeled example data instead of a dead end.
  return mountExampleData(root);
}

boot().catch(() => {
  const status = document.createElement('p');
  status.textContent = 'The demo could not start. Reload this page; if it persists, restart the OriginLM demo command.';
  status.setAttribute('role', 'alert');
  status.className = 'm-8 text-muted-foreground';
  document.querySelector('#app')?.replaceChildren(status);
});
