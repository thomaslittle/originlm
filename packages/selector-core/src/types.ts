/** Read-only structural projections of the Rust wire contract, not provider adapters. */
export type Observation<T> =
  | { state: 'known'; value: T; freshness: 'fresh' | 'stale'; [key: string]: unknown }
  | { state: 'unknown' | 'unavailable'; reason: string; [key: string]: unknown };
export interface CapabilityView {
  id: string;
  support: 'supported' | 'unsupported' | 'conditional' | 'unknown';
  origin: string;
  conditions: readonly string[];
  evidence: Observation<string>;
}
export type Control =
  | { type: 'select'; choices: readonly { value: string; label: string; disabledReason?: string | null }[]; defaultValue?: string | null }
  | { type: 'boolean' }
  | { type: 'number'; min: number; max: number; step: number }
  | { type: 'text'; maxLength: number }
  | { type: 'unknown'; nativeType: string };
export interface OptionView {
  id: string; label: string; scope: string; revision: string; control: Control;
  currentValue?: unknown; billingSensitive?: boolean; permissionSensitive?: boolean;
}
export interface ConnectionView {
  id: string; providerId: string; displayName: string;
  auth: Observation<'authenticated' | 'unauthenticated' | 'expired' | 'unchecked'>;
}
export interface RouteView {
  id: string; connectionId: string; providerId: string; displayName: string;
  billing: Observation<'subscription' | 'api_metered' | 'prepaid_credits' | 'local' | 'mixed' | 'unknown'>;
  catalogRevision: string; capabilities: readonly CapabilityView[];
}
export interface ModelView {
  id: string; routeId: string; nativeId: string; name: string; revision: string;
  capabilities: readonly CapabilityView[]; options: readonly OptionView[];
}
export interface SnapshotView {
  connections: readonly ConnectionView[]; routes: readonly RouteView[]; models: readonly ModelView[];
}
export interface IndexedModel { model: ModelView; route: RouteView; connection: ConnectionView; search: string }
export interface Catalog {
  models: ReadonlyMap<string, IndexedModel>;
  diagnostics: readonly { modelId: string; reason: string }[];
}
export interface Requirements {
  query?: string;
  requiredCapabilities?: readonly string[];
  satisfiedConditions?: readonly string[];
  allowPaidFallback?: boolean;
  billingPreference?: 'prefer-included' | 'preserve-order';
  eligibleOnly?: boolean;
}
export interface Candidate extends IndexedModel {
  key: string;
  /** UI selection eligibility only. Native execution still requires server authorization. */
  eligible: boolean;
  reasons: readonly string[];
  billingLabel: string;
}
export interface Selection {
  routeId: string; modelId: string; revision: string;
  options: Readonly<Record<string, unknown>>;
}
export interface Reconciliation {
  selection: Selection | null; needsReview: boolean; removedOptions: readonly string[]; reason?: string;
}
export interface PoolView { id: string; routeIds: readonly string[]; sharing: string; windows: readonly unknown[] }
