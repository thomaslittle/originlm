// Test-only browser host; never used by the shipped live/HTTP entry point.
import '../src/styles.css';
import { createRoot } from 'react-dom/client';
import { createLocalAI, LocalAIError, type CallOptions, type LocalAIClient, type Transport } from 'originlm';
import { App } from '../src/components/app';
import { createTestRouter } from '../src/router';
import { pathFor } from '../src/lib/pages';
import { createDemoStore } from '../src/state/store';
import { fixtureSnapshot } from '../fixture/snapshot.mjs';
import { createFixtureService } from '../fixture/service.mjs';
import { fixtureTransport } from '../fixture/transport.mjs';

interface TestOptions {
  maliciousLabel?: boolean;
  largeCatalog?: boolean;
  turnError?: { code: string; message: string } | null;
  nativeResumption?: boolean;
}

type TestGlobals = typeof globalThis & {
  mountTestApp: (options?: TestOptions) => Promise<void>;
  testNativeCalls: string[];
};

(globalThis as TestGlobals).mountTestApp = async (options: TestOptions = {}) => {
  const snapshot = fixtureSnapshot();
  if (options.maliciousLabel) snapshot.models[0].name = '<img src=x onerror="globalThis.__unsafeExecuted=true">';
  if (options.largeCatalog) {
    const models = snapshot.models;
    snapshot.models = Array.from({ length: 5000 }, (_, index) => ({
      ...models[index % models.length],
      id: `huge-${index}`,
      name: `Catalog Model ${index}`,
      nativeId: `fixture/huge-${index}`,
    }));
  }
  const connectFixture = async () =>
    createLocalAI({ transport: fixtureTransport(createFixtureService({ snapshot, turnError: options.turnError ?? null })) });
  let initialClient: LocalAIClient | null = null;
  if (options.nativeResumption) {
    (globalThis as TestGlobals).testNativeCalls = [];
    const service = createFixtureService({ snapshot });
    const base = fixtureTransport(service);
    let previousSession: Record<string, unknown> | null = null;
    // Layer native-resumption behaviour over the in-memory fixture transport.
    const transport: Transport = {
      ...base,
      subscribe: base.subscribe?.bind(base),
      close: () => base.close(),
      async request<T>(method: string, params: unknown = {}, call?: CallOptions): Promise<T> {
        if (['sessions.create', 'sessions.close', 'turns.start'].includes(method)) (globalThis as TestGlobals).testNativeCalls.push(method);
        if (method === 'runtime.handshake') {
          const handshake = await base.request<{ methods: string[] }>(method, params, call);
          return { ...handshake, methods: [...handshake.methods, 'sessions.suspend', 'sessions.resume'] } as T;
        }
        if (method === 'sessions.create') {
          previousSession = { ...(await base.request<Record<string, unknown>>(method, params, call)), nativeResumption: true };
          return previousSession as T;
        }
        if (method === 'sessions.suspend') return { resumeToken: 'test-only-resume-token-sentinel' } as T;
        if (method === 'sessions.resume') return { ...previousSession, state: 'ready' } as T;
        return base.request<T>(method, params, call);
      },
    };
    initialClient = await createLocalAI({ transport });
  }
  // In-memory history: the offline test page has no real URL to push state onto.
  // This suite exercises all selector variants; the product default is Overview.
  const router = createTestRouter('/selector-lab');
  const demo = createDemoStore({
    navigate: (page) => void router.navigate({ to: pathFor(page) }),
    connectFixture,
    connectLive: async () => {
      throw new LocalAIError('TRANSPORT_FAILED');
    },
    initialClient,
    initialMode: 'synthetic',
  });
  createRoot(document.querySelector('#app') as HTMLElement).render(<App demo={demo} router={router} />);
};
