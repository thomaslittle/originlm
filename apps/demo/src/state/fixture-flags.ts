import type { AccessRoute, Connection, EvidenceSource, Model, Observation4, Snapshot } from 'originlm';
import type { FixtureFlags } from './types';

/**
 * Scripted scenarios for automated checks. The browser never fabricates accounts
 * outside these flags, and flags are only honoured for the loopback fixture.
 */
export function applyFixtureFlags(snapshot: Snapshot, flags: FixtureFlags): Snapshot {
  let next: Snapshot = snapshot;
  if (flags.staleAuth) {
    next = {
      ...next,
      connections: next.connections.map((connection) =>
        connection.auth.state === 'known' && connection.auth.value === 'authenticated'
          ? { ...connection, auth: { ...connection.auth, freshness: 'stale' } }
          : connection,
      ),
    };
  }
  if (flags.extraProfiles) {
    const now = new Date().toISOString();
    const source: EvidenceSource = { kind: 'synthetic', adapterId: 'fixture', adapterVersion: '1', reference: 'fixture' };
    const localBilling: Observation4 = { state: 'known', value: 'local', source, observedAt: now, freshness: 'fresh', verification: 'observed' };
    const localConnection: Connection = {
      id: 'local-lm',
      providerId: 'lmstudio',
      displayName: 'LM Studio · Default profile',
      installationId: 'install-lm',
      profileId: 'profile-lm',
      accountId: null,
      auth: { state: 'unknown', reason: 'LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH' },
      plan: { state: 'unknown', reason: 'NOT_REPORTED' },
      status: 'needs_auth',
      diagnostics: [],
    };
    const signedOut: Connection = {
      id: 'signed-out',
      providerId: 'codex',
      displayName: 'Codex · Additional profile',
      installationId: 'install-codex',
      profileId: 'profile-codex',
      accountId: null,
      auth: { state: 'known', value: 'unauthenticated', source, observedAt: now, freshness: 'fresh', verification: 'observed' },
      plan: { state: 'unknown', reason: 'NOT_REPORTED' },
      status: 'needs_auth',
      diagnostics: [],
    };
    const localRoute: AccessRoute = {
      id: 'route-lm',
      connectionId: 'local-lm',
      providerId: 'lmstudio',
      displayName: 'LM Studio',
      billing: localBilling,
      capabilities: [],
      catalogRevision: 'fixture-lm',
    };
    const localModel: Model = {
      id: 'model-lm',
      routeId: 'route-lm',
      nativeId: 'ornith-1.5',
      name: 'Ornith 1.5',
      revision: 'fixture-lm',
      isDefault: false,
      capabilities: [],
      options: [],
      contextWindow: { state: 'unknown', reason: 'NOT_REPORTED' },
    };
    next = {
      ...next,
      providers: [...next.providers, { id: 'lmstudio', name: 'LM Studio', kind: 'local_runtime' }],
      connections: [...next.connections, localConnection, signedOut],
      routes: [...next.routes, localRoute],
      models: [...next.models, localModel],
    };
  }
  return next;
}
