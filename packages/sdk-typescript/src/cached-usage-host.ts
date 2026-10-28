/** In-process cache producer and T3 caller binding. No transport or account client. */
import {
  ageCachedUsageReport, cachedUsageToolDefinition, createCachedUsageReader, emptyCachedUsageReport,
  type CachedUsageMcpOptions, type CachedUsageReport,
} from './cached-usage-mcp.ts';
import type {SnapshotState, SnapshotStore} from './store.ts';

export interface CachedUsageProducer {
  /** Explicitly observe the owner's existing local cache; idempotent while attached. */
  attach(): void;
  /** Clears published rows before detaching; does not dispose the owner's store. */
  stop(): void;
  dispose(): void;
  /** Only reads an owned sanitized projection; never calls the source store. */
  read(): CachedUsageReport;
}

export function createCachedUsageProducer(options: Omit<CachedUsageMcpOptions, 'store'> & {
  store: Pick<SnapshotStore, 'getSnapshot'|'subscribe'>;
}): CachedUsageProducer {
  const store = options.store, clock = options.now ?? Date.now;
  const maxAge = options.maxObservationAgeMs ?? 60_000;
  const cleared: SnapshotState = {snapshot:null,loading:false,error:null};
  let incoming = cleared, published = emptyCachedUsageReport('unavailable');
  const project = createCachedUsageReader({...options,store:{getSnapshot:()=>incoming}});
  let attached = false, disposed = false, epoch = 0, off: (()=>void)|undefined;
  const stop = () => {
    attached = false; epoch++;
    published = emptyCachedUsageReport('unavailable'); incoming = cleared;
    const unsubscribe = off; off = undefined;
    try { unsubscribe?.(); } catch { /* Retirement must not retain rows on source failure. */ }
  };
  return {
    attach() {
      if (attached || disposed) return;
      attached = true;
      const version = ++epoch;
      const publish = () => {
        if (!attached || disposed || version !== epoch) return;
        try { incoming = store.getSnapshot(); published = project(); }
        catch { published = emptyCachedUsageReport('unavailable'); }
        finally { incoming = cleared; } // Retain only the sanitized usage projection.
      };
      try { off = store.subscribe(publish); publish(); }
      catch { stop(); }
    },
    stop,
    dispose() { if (disposed) return; disposed = true; stop(); },
    read() {
      try {
        published = ageCachedUsageReport(published,clock(),maxAge);
        return structuredClone(published);
      } catch { published = emptyCachedUsageReport('invalid'); return structuredClone(published); }
    },
  };
}

/** Minimal structural view of T3's trusted per-request McpInvocationContext. */
export interface T3UsageInvocation {
  readonly environmentId: string;
  readonly client?: {readonly sessionId: string};
  readonly thread?: unknown;
}
export interface T3CachedUsageTool {
  readonly definition: typeof cachedUsageToolDefinition;
  read(invocation: T3UsageInvocation|undefined, args: unknown): CachedUsageReport;
  /** Revokes this binding only; a peer may still use the same producer. */
  revoke(): void;
}

/** No implicit permission from an orchestration credential, model name or provider. */
export function createT3CachedUsageTool(options: {
  source: Pick<CachedUsageProducer, 'read'>;
  environmentId: string;
  clientSessionIds: readonly string[];
}): T3CachedUsageTool {
  const valid = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9_.:-]{1,256}$/.test(value);
  if (!valid(options.environmentId) || !Array.isArray(options.clientSessionIds) || options.clientSessionIds.length > 32
      || options.clientSessionIds.some(session=>!valid(session))) throw new Error('INVALID_CACHED_USAGE_BINDING');
  const environment = options.environmentId, sessions = new Set(options.clientSessionIds), source = options.source;
  let revoked = false;
  return {
    definition:cachedUsageToolDefinition,
    read(invocation,args) {
      if (revoked || !invocation || invocation.environmentId !== environment || invocation.thread !== undefined
          || !invocation.client || !sessions.has(invocation.client.sessionId)) throw new Error('CACHED_USAGE_ACCESS_DENIED');
      if (!args || typeof args !== 'object' || Array.isArray(args) || Object.keys(args).length) throw new Error('INVALID_CACHED_USAGE_ARGUMENTS');
      return source.read();
    },
    revoke() { revoked = true; sessions.clear(); },
  };
}
