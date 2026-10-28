import type { LocalAIClient } from './client.ts';
import type { Snapshot, Model } from './generated.ts';
import type { CallOptions } from './types.ts';
import { LocalAIError } from './errors.ts';
export interface CatalogReadOptions extends CallOptions { maxModels?: number }
const fields: Exclude<keyof Snapshot,'models'>[] = ['protocolVersion','revision','observedAt','mode','providers','installations','profiles','accounts','connections','routes','quotaPools','diagnostics','cursor','health','issues'];
/** Explicit read only: no discovery, authentication, retries, or fallback after a failed page. */
export async function readCatalogSnapshot(client: LocalAIClient, options: CatalogReadOptions = {}): Promise<Snapshot> {
  const max = options.maxModels ?? 20_000;
  if (!Number.isSafeInteger(max) || max < 1 || max > 100_000) throw new LocalAIError('INVALID_LIMIT');
  const call = {signal:options.signal, timeoutMs:options.timeoutMs};
  if (!client.handshake?.methods.includes('models.page')) {
    const snapshot = await client.snapshot.get(undefined, call);
    if (!Array.isArray(snapshot.models)) throw new LocalAIError('PROTOCOL_INVALID');
    if (snapshot.models.length > max) throw new LocalAIError('FRAME_TOO_LARGE');
    return snapshot;
  }
  const header = await client.snapshot.get({fields}, call);
  if (!header || typeof header.revision !== 'string' || !Array.isArray(header.routes)) throw new LocalAIError('PROTOCOL_INVALID');
  const models: Model[] = [], ids = new Set<string>(), cursors = new Set<string>();
  const routes = new Set(header.routes.map(r=>r.id));
  let cursor: string | undefined, bytes = 0;
  do {
    if (call.signal?.aborted) throw new LocalAIError('CANCELLED');
    const page = await client.models.page({limit:200, ...(cursor ? {cursor} : {})}, call);
    if (!page || page.revision !== header.revision) throw new LocalAIError('SELECTION_STALE');
    if (!Array.isArray(page.models) || page.models.length > 500) throw new LocalAIError('PROTOCOL_INVALID');
    for (const model of page.models) {
      if (!model || typeof model.id !== 'string' || !model.id || ids.has(model.id) || !routes.has(model.routeId)) throw new LocalAIError('PROTOCOL_INVALID');
      ids.add(model.id); models.push(model); bytes += new TextEncoder().encode(JSON.stringify(model)).length;
      if (models.length > max || bytes > 64*1024*1024) throw new LocalAIError('FRAME_TOO_LARGE');
    }
    if (page.nextCursor !== null && (typeof page.nextCursor !== 'string' || !page.nextCursor || page.nextCursor.length > 256 || !page.models.length || cursors.has(page.nextCursor))) throw new LocalAIError('PROTOCOL_INVALID');
    cursor = page.nextCursor ?? undefined;
    if (cursor) cursors.add(cursor);
  } while (cursor);
  const final = await client.snapshot.get({fields:['revision']}, call);
  if (final.revision !== header.revision) throw new LocalAIError('SELECTION_STALE');
  return {...header, models};
}
