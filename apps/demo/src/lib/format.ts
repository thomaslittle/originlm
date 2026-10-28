/** Presentation helpers. Runtime-supplied labels are always rendered as text nodes. */

export const capNames: Record<string, string> = {
  'generation.text': 'Text generation',
  'input.text': 'Text input',
  'input.image': 'Image input',
  'structured.output': 'Structured output',
  tools: 'Tool support',
  offline: 'Offline operation',
};

const REASONS: Record<string, string> = {
  AUTH_UNVERIFIED: 'Authentication has not been checked',
  AUTH_REQUIRED: 'Authentication needs attention',
  BILLING_UNKNOWN: 'Billing classification is not reported',
  BILLING_CONSENT_REQUIRED: 'Metered access needs your consent',
  RATE_LIMITED: 'The provider reports no ordinary usage allowance right now',
};

export function reasonLabel(reason: string): string {
  const [code, id = ''] = reason.split(':', 2);
  const capability = capNames[id] ?? id;
  if (code === 'CAPABILITY_UNKNOWN') return `${capability} is not confirmed`;
  if (code === 'CAPABILITY_UNSUPPORTED') return `${capability} is not supported`;
  if (code === 'CAPABILITY_CONDITIONAL') return `${capability} has unmet requirements`;
  return REASONS[code] ?? reason;
}

export function compactIdentity(value: string): string {
  return value.length > 28 ? `${value.slice(0, 25)}…` : value;
}

type KnownValue<T> = Extract<T, { state: 'known'; value: unknown }>['value'];

/** The value of a "known" observation, otherwise undefined. */
export function known<T extends { state: string }>(observation: T | null | undefined): KnownValue<T> | undefined {
  return observation && observation.state === 'known' ? (observation as unknown as { value: KnownValue<T> }).value : undefined;
}

/** Deterministic hue so every provider keeps the same accent across the UI. */
export function providerHue(id: string): number {
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) % 360;
  return hash;
}

/** Token counts as compact tokens: 128000 becomes "128K", 1000000 becomes "1M". */
export function formatTokens(value: unknown): string | undefined {
  const count = Number(value);
  if (!Number.isFinite(count) || count <= 0) return undefined;
  if (count >= 1_000_000) return `${Number((count / 1_000_000).toFixed(1))}M`;
  if (count >= 1_000) return `${Number((count / 1_000).toFixed(count >= 100_000 ? 0 : 1))}K`;
  return String(count);
}

export type BillingVariant = 'success' | 'info' | 'warning' | 'outline';

/** Badge tone for a billing label: included allowance and local compute read as safe, metered as a decision. */
export function billingVariant(label: string): BillingVariant {
  if (label === 'Included allowance') return 'success';
  if (label === 'Local compute') return 'info';
  if (label === 'Metered access' || label === 'Prepaid credits') return 'warning';
  return 'outline';
}
