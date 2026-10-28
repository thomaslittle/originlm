import { ResetCountdown } from '@/components/common/reset-countdown';
import { CostTrend } from '@/components/common/cost-trend';
import { ShieldCheckIcon, GaugeIcon } from '@phosphor-icons/react';
import { Link } from '@tanstack/react-router';
import type { Connection } from 'originlm';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/common/empty-state';
import { ProviderGlyph } from '@/components/common/provider-glyph';
import { Section } from '@/components/common/section';
import { known } from '@/lib/format';
import { useDemo } from '@/state/context';

import { percent, readingOf } from '@/lib/usage';

const isVerified = (connection: Connection) => known(connection.auth) === 'authenticated' && connection.auth.state === 'known' && connection.auth.freshness === 'fresh';
const isStale = (connection: Connection) => known(connection.auth) === 'authenticated' && connection.auth.state === 'known' && connection.auth.freshness === 'stale';

function ProfileCard({ connection, stale }: { connection: Connection; stale?: boolean }) {
  const providers = useDemo((state) => state.snapshot?.providers);
  const provider = providers?.find((item) => item.id === connection.providerId)?.name ?? connection.providerId;
  return (
    <div className="flex items-start gap-3 rounded-xl border bg-background/40 p-4">
      <ProviderGlyph id={connection.providerId} name={provider} />
      <div className="min-w-0">
        <h3 className="text-sm font-semibold">{connection.displayName}</h3>
        <div className="my-2">
          <Badge variant={stale ? 'warning' : 'success'}>{stale ? 'Previously authenticated' : 'Authenticated'}</Badge>
        </div>
        {stale ? <p className="text-xs text-muted-foreground">{`${provider}: check is stale`}</p> : null}
        <p className="text-xs text-muted-foreground">{known(connection.plan)?.label ?? 'Plan not reported'}</p>
      </div>
    </div>
  );
}

export default function OverviewPage() {
  const snapshot = useDemo((state) => state.snapshot);
  const connections = snapshot?.connections ?? [];
  const modelCount = snapshot?.models.length ?? 0;
  const mode = useDemo((state) => state.mode);
  const verified = connections.filter(isVerified);
  const previous = connections.filter(isStale);
  const needsAuth = connections.filter((connection) => connection.status === 'needs_auth');
  const pools = snapshot?.quotaPools ?? [];
  const issues = snapshot?.issues?.filter((issue) => issue.state === 'open') ?? [];
  const windows = pools.flatMap((pool) => pool.windows.map((window) => ({ pool, window })));

  return (
    <div className="grid gap-8">
      <div className="flex flex-wrap items-end justify-between gap-4 border-b pb-5">
        <p className="text-sm text-muted-foreground">{mode === 'synthetic' ? 'Example data' : 'Provider-reported evidence'}</p>
        <div className="flex gap-3">
          <Button render={<Link to="/connections" />} nativeButton={false}>Manage connections</Button>
          <Button variant="outline" render={<Link to="/selector-lab" />} nativeButton={false}>Build with it</Button>
        </div>
      </div>

      <div data-capture="providers" className="flex flex-wrap gap-4 border-y py-4">
        {snapshot?.providers.map((provider) => <div key={provider.id} className="flex items-center gap-2">
          <ProviderGlyph id={provider.id} name={provider.name} /><span className="text-sm font-medium">{provider.name}</span>
        </div>)}
      </div>
      <dl className="grid grid-cols-2 gap-6 md:grid-cols-4">
        {[
          ['Profiles', connections.length], ['Authenticated now', verified.length],
          ['Route-scoped models', modelCount], ['Open issues', snapshot?.issues ? issues.length : 'Unknown'],
        ].map(([label, value]) => (
          <div key={label} className="border-l pl-4">
            <dt className="text-sm text-muted-foreground">{label}</dt>
            <dd className="mt-2 text-3xl font-semibold tabular-nums">{value}</dd>
          </div>
        ))}
      </dl>

      {needsAuth.length ? <p className="border-l-2 border-warning pl-4 text-sm"><Link to="/connections" className="underline underline-offset-4">Sign-in needs attention</Link>: {needsAuth.map((connection) => connection.displayName).join(', ')}.</p> : null}
      <div data-capture="usage-and-costs" className="grid items-start gap-8 xl:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
      <section aria-labelledby="allowances-title">
        <div className="mb-4 flex items-baseline justify-between gap-4">
          <h2 id="allowances-title" className="text-xl font-semibold">Usage and allowances</h2>
          <Link to="/usage" className="whitespace-nowrap text-sm underline underline-offset-4">Usage details</Link>
        </div>
        {windows.length ? (
          <ul className="grid gap-x-6 border-t sm:grid-cols-2">
            {windows.map(({ pool, window }) => {
              const reading = readingOf(window);
              const owners = connections.filter((connection) => pool.connectionIds.includes(connection.id));
              const provider = [...new Set(owners.map((connection) =>
                snapshot?.providers.find((item) => item.id === connection.providerId)?.name ?? connection.providerId
              ))].join(', ') || 'Provider not reported';
              return (
                <li key={`${pool.id}:${window.id}`} className="grid grid-cols-[4.5rem_minmax(0,1fr)] items-center gap-3 border-b py-5">
                  <div className="relative flex size-18 items-center justify-center text-primary">
                    <svg viewBox="0 0 80 80" className="absolute inset-0 size-full -rotate-90" aria-hidden="true">
                      <circle cx="40" cy="40" r="34" fill="none" stroke="var(--border)" strokeWidth="5" />
                      {reading?.fraction != null ? <circle cx="40" cy="40" r="34" fill="none" stroke="currentColor" strokeWidth="5" pathLength="100" strokeDasharray={`${reading.fraction * 100} 100`} strokeLinecap="round" /> : null}
                    </svg>
                    <span className="text-sm font-semibold tabular-nums">{reading?.fraction != null ? percent(reading.fraction) : '—'}</span>
                  </div>
                  <div className="min-w-0">
                    <h3 className="font-semibold">{provider}</h3>
                    <p className="text-sm text-muted-foreground">{window.label}</p>
                    <p className="mt-1 text-xs text-muted-foreground">{owners.map((owner) => owner.displayName).join(', ') || 'Connection not reported'}</p>
                  </div>
                  <div className="col-span-2 flex flex-wrap justify-between gap-2 text-xs text-muted-foreground">
                    <span>{reading ? reading.fraction === null ? `${reading.used} ${reading.unit}; total unknown` : `${percent(reading.fraction)} used; ${reading.freshness}` : 'Usage not reported'}</span>
                    <ResetCountdown at={window.resetsAt} />
                  </div>
                </li>
              );
            })}
          </ul>
        ) : (
          <EmptyState icon={GaugeIcon} title="No allowances reported yet"
            description="Connect a runtime and authorize metadata checks. Providers that do not report usage stay unknown." />
        )}
        <p className="mt-3 text-xs text-muted-foreground">Shared pools appear once. Allowances from different providers are never added together.</p>
      </section>
      <CostTrend />
      </div>

      {issues.length ? (
        <section aria-labelledby="issues-title">
          <h2 id="issues-title" className="mb-3 text-xl font-semibold">Needs attention</h2>
          <ul className="divide-y border-y">
            {issues.map((issue) => <li key={issue.id} className="py-3">
              <p className="font-medium">{issue.title}</p>
              <p className="text-sm text-muted-foreground">{issue.summary}</p>
            </li>)}
          </ul>
          <Link to="/diagnostics" className="mt-3 inline-block text-sm underline">Review diagnostics</Link>
        </section>
      ) : null}

      {verified.length ? (
        <Section title="Verified profiles" description="Only fresh authenticated profiles appear here.">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {verified.slice(0, 4).map((connection) => (
              <ProfileCard key={connection.id} connection={connection} />
            ))}
          </div>
        </Section>
      ) : previous.length ? null : (
        <Section title="Verified profiles" description="Only fresh authenticated profiles appear here.">
          <EmptyState icon={ShieldCheckIcon} title="No authenticated profiles yet" description="Detected clients remain unverified until an explicit account and model check succeeds." />
        </Section>
      )}

      {previous.length ? (
        <Section title="Previously authenticated" description="These profiles passed an earlier check, but that authentication evidence is stale.">
          <div data-testid="previously-authenticated-overview" className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {previous.slice(0, 4).map((connection) => (
              <ProfileCard key={connection.id} connection={connection} stale />
            ))}
          </div>
        </Section>
      ) : null}
    </div>
  );
}
