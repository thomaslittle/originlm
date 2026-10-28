import { useNavigate } from '@tanstack/react-router';
import { ArrowsClockwiseIcon, HardDriveIcon, ScanIcon, ShieldCheckIcon, ShieldWarningIcon } from '@phosphor-icons/react';
import type { Connection, Snapshot } from 'originlm';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Spinner } from '@/components/ui/spinner';
import { CodeBlock } from '@/components/common/code-block';
import { EmptyState } from '@/components/common/empty-state';
import { ProviderGlyph } from '@/components/common/provider-glyph';
import { Section } from '@/components/common/section';
import { compactIdentity, known } from '@/lib/format';
import { cn } from '@/lib/utils';
import { useActions, useDemo } from '@/state/context';
import { isLocalRuntimeProvider } from '../../lib/catalog_visibility.mjs';
import { connectionAuthLabel } from '../../lib/profile_auth.mjs';

const authState = (connection: Connection) => (connection.auth.state === 'known' ? connection.auth : null);
const isAuthenticated = (connection: Connection) => authState(connection)?.value === 'authenticated' && authState(connection)?.freshness === 'fresh';
const wasAuthenticated = (connection: Connection) => authState(connection)?.value === 'authenticated' && authState(connection)?.freshness === 'stale';
const isSignedOut = (connection: Connection) => authState(connection)?.value === 'unauthenticated' && authState(connection)?.freshness === 'fresh';

function hasLocalCatalog(connection: Connection, snapshot: Snapshot): boolean {
  return (
    isLocalRuntimeProvider(connection) &&
    snapshot.routes.some(
      (route) =>
        route.connectionId === connection.id &&
        known(route.billing) === 'local' &&
        route.billing.state === 'known' &&
        route.billing.freshness === 'fresh' &&
        snapshot.models.some((model) => model.routeId === route.id),
    )
  );
}

function Identity({ term, value }: { term: string; value: string }) {
  return (
    <>
      <dt className="text-muted-foreground">{term}</dt>
      <dd className="truncate text-right font-mono text-[0.7rem]" title={value} aria-label={`${term} identifier ${value}`}>
        {compactIdentity(value)}
      </dd>
    </>
  );
}

function ConnectionFacts({ connection }: { connection: Connection }) {
  return (
    <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-xs">
      <dt className="text-muted-foreground">Plan</dt>
      <dd className="text-right">{known(connection.plan)?.label ?? 'Not reported'}</dd>
      <Identity term="Profile" value={connection.profileId} />
      <Identity term="Account" value={connection.accountId ?? 'Unverified'} />
      <Identity term="Installation" value={connection.installationId} />
    </dl>
  );
}

function ConnectionActions({ connection, local }: { connection: Connection; local: boolean }) {
  const actions = useActions();
  const navigate = useNavigate();
  const mode = useDemo((state) => state.mode);
  const probing = useDemo((state) => state.probing);
  const batch = useDemo((state) => state.metadataBatch);
  const checking = probing.includes(connection.id);
  return (
    <div className="mt-5 flex flex-wrap gap-2">
      {mode === 'live' ? (
        <Button variant="outline" size="sm" disabled={checking || Boolean(batch)} onClick={() => void actions.probeConnection(connection.id)}>
          {checking ? <Spinner data-icon="inline-start" /> : <ArrowsClockwiseIcon data-icon="inline-start" />}
          {checking ? 'Checking\u2026' : local ? 'Refresh local catalog' : 'Check account and models'}
        </Button>
      ) : null}
      <Button
        variant="outline"
        size="sm"
        onClick={() => {
          actions.patch({ variant: 'rail', provider: connection.providerId, query: connection.displayName });
          void navigate({ to: '/selector-lab' });
        }}
      >
        Inspect models
      </Button>
    </div>
  );
}

/** Progress and outcome of this profile's metadata check. */
function CheckStatus({ connectionId }: { connectionId: string }) {
  const result = useDemo((state) => state.probeResults[connectionId]);
  const checking = useDemo((state) => state.probing.includes(connectionId));
  const queued = useDemo((state) => state.metadataBatch !== null && !state.probing.includes(connectionId) && !state.probeResults[connectionId]);
  if (checking || queued) {
    return (
      <p role="status" aria-live="polite" className="mt-4 flex items-center gap-2 rounded-lg border px-3 py-2 text-xs text-muted-foreground">
        {checking ? <Spinner /> : null}
        {checking ? 'Checking account and models\u2026' : 'Waiting for the earlier check of this provider'}
      </p>
    );
  }
  if (!result) return null;
  return (
    <p role="status" aria-live="polite" className={cn('metadata-result mt-4 rounded-lg border px-3 py-2 text-xs', result.ok ? 'border-success/25 bg-success/10 text-success' : 'border-warning/25 bg-warning/10 text-warning')}>
      {result.message}
    </p>
  );
}

function ConnectionCard({ connection, snapshot }: { connection: Connection; snapshot: Snapshot }) {
  const provider = snapshot.providers.find((item) => item.id === connection.providerId)?.name ?? connection.providerId;
  const local = isLocalRuntimeProvider(connection);
  const catalogReady = hasLocalCatalog(connection, snapshot);
  const verified = isAuthenticated(connection);
  const freshness = authState(connection)?.freshness ?? 'unavailable';

  return (
    <Card data-connection-id={connection.id} className="profile-card gap-0 bg-card p-5 [--card-spacing:--spacing(0)]">
      <div className="flex items-start gap-3">
        <ProviderGlyph id={connection.providerId} name={provider} size="lg" />
        <div className="min-w-0 flex-1">
          <h2 className="text-base leading-tight font-semibold tracking-tight">{connection.displayName}</h2>
          <p className="mt-0.5 text-xs text-muted-foreground">{provider}</p>
        </div>
      </div>
      <div className="my-4 flex flex-wrap gap-1.5">
        <Badge variant={verified || (local && catalogReady) ? 'success' : local ? 'outline' : 'warning'}>{connectionAuthLabel(connection)}</Badge>
        <Badge variant={catalogReady ? 'success' : 'outline'}>{local ? (catalogReady ? 'Local catalog ready' : 'Catalog not reported') : freshness}</Badge>
      </div>
      <ConnectionFacts connection={connection} />
      <CheckStatus connectionId={connection.id} />
      <details className="mt-4">
        <summary className="cursor-pointer text-xs text-muted-foreground select-none hover:text-foreground">Evidence and freshness</summary>
        <CodeBlock className="mt-3">{JSON.stringify({ auth: connection.auth, plan: connection.plan }, null, 2)}</CodeBlock>
      </details>
      <ConnectionActions connection={connection} local={local} />
    </Card>
  );
}

function CardGrid({ list, snapshot, testId }: { list: Connection[]; snapshot: Snapshot; testId?: string }) {
  return (
    <div data-testid={testId} className="grid grid-cols-1 gap-5 xl:grid-cols-2">
      {list.map((connection) => (
        <ConnectionCard key={connection.id} connection={connection} snapshot={snapshot} />
      ))}
    </div>
  );
}

function ProfileMetadata() {
  const actions = useActions();
  const batch = useDemo((state) => state.metadataBatch);
  const probing = useDemo((state) => state.probing.length > 0);
  const checked = useDemo((state) => Object.keys(state.probeResults).length > 0);
  return (
    <Section title="Profile metadata" description="Metadata-only: check every detected account and local catalog in sequence. No model prompts are sent.">
      {batch ? (
        <div role="status" aria-live="polite" className="mb-4 grid gap-2">
          <p className="text-sm">{`${batch.completed} of ${batch.total} profiles checked`}</p>
          <div className="h-1.5 overflow-hidden rounded-full bg-muted">
            <div className="h-full rounded-full bg-primary transition-none duration-500" style={{ width: `${(batch.completed / batch.total) * 100}%` }} />
          </div>
        </div>
      ) : null}
      <Button data-testid="check-all-profiles" disabled={probing || Boolean(batch)} onClick={() => void actions.probeAllConnections()}>
        {batch ? <Spinner data-icon="inline-start" /> : <ScanIcon data-icon="inline-start" />}
        {batch ? `Checking profile ${Math.min(batch.completed + 1, batch.total)} of ${batch.total}…` : checked ? 'Recheck all profiles' : 'Check all detected profiles'}
      </Button>
    </Section>
  );
}

export default function ConnectionsPage() {
  const actions = useActions();
  const snapshot = useDemo((state) => state.snapshot);
  const mode = useDemo((state) => state.mode);
  const probeResults = useDemo((state) => state.probeResults);
  const detectedExpanded = useDemo((state) => state.detectedExpanded);
  if (!snapshot) return null;

  const connections = snapshot.connections;
  const verified = connections.filter(isAuthenticated);
  const previous = connections.filter(wasAuthenticated);
  const local = connections.filter((connection) => isLocalRuntimeProvider(connection));
  const signedOut = connections.filter(isSignedOut);
  const detected = connections.filter((connection) => !isAuthenticated(connection) && !wasAuthenticated(connection) && !isLocalRuntimeProvider(connection) && !isSignedOut(connection));
  const uncheckedDetected = detected.filter((connection) => !probeResults[connection.id]);

  return (
    <div className="grid gap-5">
      {mode === 'live' ? <ProfileMetadata /> : null}
      {verified.length ? (
        <Section title="Authenticated profiles" description="Only fresh provider-confirmed authentication appears as a connected profile.">
          <CardGrid list={verified} snapshot={snapshot} testId="verified-profiles" />
        </Section>
      ) : previous.length ? null : (
        <Section title="Authenticated profiles" description="Only fresh provider-confirmed authentication appears as a connected profile.">
          <EmptyState icon={ShieldCheckIcon} title="No authenticated profiles yet" description="Run the metadata-only check below. No model prompts are sent." />
        </Section>
      )}
      {previous.length ? (
        <Section title="Previously authenticated" description="These profiles passed an earlier check, but that evidence is stale. Check account and models again to confirm their current status.">
          <CardGrid list={previous} snapshot={snapshot} testId="previously-authenticated-profiles" />
        </Section>
      ) : null}
      {local.length ? (
        <Section title="Local runtimes" description="Local model servers do not have account authentication. Their routes appear when the server reports a real model catalog.">
          <CardGrid list={local} snapshot={snapshot} testId="local-runtimes" />
        </Section>
      ) : null}
      {signedOut.length ? (
        <Section title="Not authenticated" description="These explicit checks did not confirm a signed-in account. Their models are excluded; check again after signing in.">
          <CardGrid list={signedOut} snapshot={snapshot} testId="unauthenticated-profiles" />
        </Section>
      ) : null}
      <details
        data-testid="detected-profiles"
        className="rounded-2xl border bg-card p-6"
        open={detectedExpanded}
        onToggle={(event) => actions.patch({ detectedExpanded: event.currentTarget.open })}
      >
        <summary className="flex cursor-pointer items-center gap-2.5 text-sm font-semibold select-none">
          {uncheckedDetected.length ? <ShieldWarningIcon className="size-4 text-warning" aria-hidden="true" /> : <HardDriveIcon className="size-4 text-muted-foreground" aria-hidden="true" />}
          {uncheckedDetected.length ? `Other detected clients · ${uncheckedDetected.length} need a metadata check` : `Other detected clients · ${detected.length} checked, not confirmed`}
        </summary>
        <p className="my-3 text-xs text-muted-foreground">
          {uncheckedDetected.length
            ? 'Installation alone does not prove an account is signed in. Check profiles to request supported metadata; unsupported sources remain unverified.'
            : 'These profiles have a metadata result but are not confirmed as authenticated. Each result explains whether authentication was absent or the provider could not be checked.'}
        </p>
        {detected.length ? <CardGrid list={detected} snapshot={snapshot} /> : <p className="text-xs text-muted-foreground">Every remaining profile has a current result above.</p>}
      </details>
    </div>
  );
}
