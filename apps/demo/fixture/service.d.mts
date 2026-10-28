import type { Snapshot, Event } from 'originlm';

export interface FixtureService {
  handle(method: string, params?: unknown): unknown;
  replay(after?: string): Event[];
  subscribe(listener: (event: Event) => void): () => void;
  close(): void;
}

export function createFixtureService(options?: {
  snapshot?: Snapshot;
  turnError?: { code: string; message: string } | null;
}): FixtureService;
