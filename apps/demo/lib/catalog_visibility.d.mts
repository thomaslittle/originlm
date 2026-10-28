import type { Snapshot } from 'originlm';

export function isLocalRuntimeProvider(connection: { providerId?: string } | null | undefined): boolean;
export function filterLiveCatalog(snapshot: Snapshot): Snapshot;
