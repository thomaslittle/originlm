import type { Candidate, CapabilityView, Catalog, PoolView, Requirements } from './types.ts';
import { selectionKey } from './catalog.ts';
function capabilityReason(id: string, capabilities: readonly CapabilityView[], satisfied: ReadonlySet<string>): string | null {
  const matches = capabilities.filter(c => c.id === id);
  if (!matches.length) return `CAPABILITY_UNKNOWN:${id}`;
  if (matches.some(c => c.support === 'unsupported')) return `CAPABILITY_UNSUPPORTED:${id}`;
  if (matches.some(c => c.support === 'unknown' || c.evidence.state !== 'known' || c.evidence.freshness !== 'fresh')) return `CAPABILITY_UNKNOWN:${id}`;
  if (matches.some(c => c.support === 'conditional' || c.conditions.some(condition => !satisfied.has(condition)))) return `CAPABILITY_CONDITIONAL:${id}`;
  return null;
}
export function resolveCandidates(catalog: Catalog, requirements: Requirements = {}): Candidate[] {
  const terms = (requirements.query ?? '').slice(0, 2048).normalize('NFKC').toLowerCase().trim().split(/\s+/).filter(Boolean);
  const satisfied = new Set(requirements.satisfiedConditions ?? []);
  const candidates: Candidate[] = [];
  for (const entry of catalog.models.values()) {
    if (!terms.every(term => entry.search.includes(term))) continue;
    const reasons: string[] = [];
    const auth = entry.connection.auth;
    const billing = entry.route.billing;
    const billingValue = billing.state === 'known' && billing.freshness === 'fresh' ? billing.value : 'unknown';
    const accountlessLocal = ['ollama', 'lmstudio'].includes(entry.connection.providerId)
      && entry.route.providerId === entry.connection.providerId && billingValue === 'local'
      && auth.state === 'unknown' && auth.reason === 'LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH';
    if (!accountlessLocal) {
      if (auth.state !== 'known' || auth.freshness !== 'fresh' || auth.value === 'unchecked') reasons.push('AUTH_UNVERIFIED');
      else if (auth.value !== 'authenticated') reasons.push('AUTH_REQUIRED');
    }
    if (billingValue === 'subscription' && entry.route.capabilities.some(c => c.id === 'usage.included_allowed' && c.support === 'unsupported')) reasons.push('RATE_LIMITED');
    if (billingValue === 'unknown') reasons.push('BILLING_UNKNOWN');
    else if (!['subscription', 'local'].includes(billingValue) && !requirements.allowPaidFallback) reasons.push('BILLING_CONSENT_REQUIRED');
    for (const id of requirements.requiredCapabilities ?? []) {
      const reason = capabilityReason(id, [...entry.model.capabilities, ...entry.route.capabilities], satisfied);
      if (reason) reasons.push(reason);
    }
    const eligible = reasons.length === 0;
    if (requirements.eligibleOnly && !eligible) continue;
    const billingLabel = billingValue === 'subscription' ? 'Included allowance' : billingValue === 'local' ? 'Local compute'
      : billingValue === 'unknown' ? 'Billing unknown' : billingValue === 'prepaid_credits' ? 'Prepaid credits' : 'Metered access';
    candidates.push({ ...entry, key: selectionKey(entry.route.id, entry.model.id), eligible, reasons, billingLabel });
  }
  // Keep broad typeahead matches, but prefer the exact phrase the user entered.
  // Stable sorting retains account/route identity and original order for ties.
  if (terms.length) {
    const phrase = terms.join(' ');
    const relevance = (c: Candidate) => c.search.includes(phrase) ? 0 : 1;
    candidates.sort((a, b) => relevance(a) - relevance(b));
  }
  if (requirements.billingPreference === 'prefer-included') {
    const rank = (c: Candidate) => (c.eligible ? 0 : 2) + (['Included allowance', 'Local compute'].includes(c.billingLabel) ? 0 : 1);
    candidates.sort((a, b) => rank(a) - rank(b));
  }
  return candidates;
}
export function groupCandidates(candidates: readonly Candidate[]): { providerId: string; candidates: Candidate[] }[] {
  const groups = new Map<string, Candidate[]>();
  for (const candidate of candidates) {
    let group = groups.get(candidate.route.providerId);
    if (!group) { group = []; groups.set(candidate.route.providerId, group); }
    group.push(candidate);
  }
  return [...groups].map(([providerId, candidates]) => ({ providerId, candidates }));
}
/** Return unique provider-reported pools; never add unlike percentages or infer sharing. */
export function visibleQuotaPools<T extends PoolView>(pools: readonly T[], routeIds: readonly string[]): T[] {
  const visible = new Set(routeIds);
  const unique = new Map<string, T>();
  for (const pool of pools) if (pool.routeIds.some(id => visible.has(id)) && !unique.has(pool.id)) unique.set(pool.id, pool);
  return [...unique.values()];
}
