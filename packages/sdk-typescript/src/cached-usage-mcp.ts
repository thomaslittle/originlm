/** Optional MCP view of an existing SDK cache. No client, connection, timer or provider operation. */
import type { EvidenceKind, QuotaPool, UsageWindow } from './generated.ts';
type UsageReading = UsageWindow['reading'];
import type { SnapshotStore } from './store.ts';

export interface CachedUsageMcpOptions {
  store: Pick<SnapshotStore, 'getSnapshot'>;
  /** Additional fail-closed projection of the broker client's already approved scope. */
  scope: { connectionIds: readonly string[]; accountIds: readonly string[] };
  /** Operator-verified mapping, never inferred from provider/model names or email. */
  mappings?: readonly { consumerAccountId: string; kitAccountId: string }[];
  maxObservationAgeMs?: number;
  now?: () => number;
}
export interface CachedUsageReport {
  state: 'cached' | 'unavailable' | 'invalid';
  mode?: 'live' | 'synthetic';
  revision?: string;
  observedAt?: string;
  quotaPools: QuotaPool[];
  connections: { id: string; providerId: string }[];
  mappings: { consumerAccountId: string; kitAccountId?: string; state: 'explicit' | 'unknown' }[];
  unmappedAccountIds: string[];
  contextCounters: 'not_reported';
  noticeCodes: string[];
}
export type CachedUsageMcpHandler = (message: unknown) => Record<string, unknown> | undefined;
const id = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9_.:-]{1,256}$/.test(value);
const object = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value);
const time = (value: unknown): string | undefined => typeof value === 'string' && /^\d{4}-\d\d-\d\dT/.test(value) && Number.isFinite(Date.parse(value)) ? value : undefined;
const decimal = (value: unknown): value is string => typeof value === 'string' && value.length <= 128 && /^\d+(\.\d+)?$/.test(value);
const kinds = new Set<EvidenceKind>(['official_protocol','official_api','local_config','local_log','community_endpoint','user_supplied','derived','synthetic']);
const versions = (value: unknown): value is string => typeof value === 'string' && /^\d[0-9A-Za-z.+_-]{0,63}$/.test(value);
function invalid(): never { throw new Error('INVALID_USAGE_CACHE'); }
function reading(value: UsageReading, now: number, maxAge: number, loading: boolean): UsageReading {
  if (value.state !== 'known') {
    if (!['unknown','unavailable'].includes(value.state)) invalid();
    return { state:value.state, reason:value.state === 'unavailable'
      ? (value.reason === 'CAPABILITY_UNSUPPORTED' ? 'CAPABILITY_UNSUPPORTED' : 'USAGE_UNAVAILABLE') : 'USAGE_UNKNOWN',
      ...(time(value.checkedAt) ? {checkedAt:time(value.checkedAt)} : {}) };
  }
  const observedAt = time(value.observedAt);
  const expiresAt = time(value.expiresAt);
  const age = observedAt ? now-Date.parse(observedAt) : Infinity;
  if (!observedAt || !['fresh','stale'].includes(value.freshness)
      || !['reported','observed','inferred'].includes(value.verification)
      || !kinds.has(value.source.kind) || !id(value.source.adapterId) || !versions(value.source.adapterVersion)
      || !decimal(value.value.used) || typeof value.value.unit !== 'string' || !/^[A-Za-z_]{1,32}$/.test(value.value.unit)) invalid();
  if (value.value.limit != null && !decimal(value.value.limit)) invalid();
  if (value.value.remaining != null && !decimal(value.value.remaining)) invalid();
  if (value.value.usedFraction != null && (!Number.isFinite(value.value.usedFraction) || value.value.usedFraction < 0)) invalid();
  return {
    state:'known', observedAt, verification:value.verification,
    freshness:loading || value.freshness === 'stale' || age < 0 || age > maxAge || (expiresAt && Date.parse(expiresAt) <= now) ? 'stale' : 'fresh',
    ...(expiresAt ? {expiresAt} : {}),
    source:{kind:value.source.kind,adapterId:value.source.adapterId,adapterVersion:value.source.adapterVersion,
      ...(versions(value.source.clientVersion) ? {clientVersion:value.source.clientVersion} : {})},
    value:{unit:value.value.unit,used:value.value.used,limit:value.value.limit ?? null,
      remaining:value.value.remaining ?? null,usedFraction:value.value.usedFraction ?? null},
  };
}
export const emptyCachedUsageReport = (state: 'unavailable'|'invalid'): CachedUsageReport => ({state,quotaPools:[],connections:[],mappings:[],
  unmappedAccountIds:[],contextCounters:'not_reported',noticeCodes:[state === 'invalid' ? 'INVALID_USAGE_CACHE' : 'CACHE_UNAVAILABLE']});

export const cachedUsageToolDefinition = Object.freeze({
  name:'kit_usage_cached',
  description:'Read scoped cached provider quota windows only. Unknown is not zero. No provider calls, refresh, credentials, inference or interest renewal. Explicit account mappings only; context counters are not plan quota.',
  inputSchema:Object.freeze({type:'object',properties:Object.freeze({}),additionalProperties:false}),
  annotations:Object.freeze({readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:false}),
});

/** Age an owned sanitized projection; never promote stale data or infer a reset. */
export function ageCachedUsageReport(report: CachedUsageReport, now: number, maxAge: number): CachedUsageReport {
  try {
    if (!Number.isFinite(now) || !Number.isSafeInteger(maxAge) || maxAge < 1000 || maxAge > 86_400_000) invalid();
    const copy = structuredClone(report);
    for (const pool of copy.quotaPools) for (const window of pool.windows) {
      window.reading = reading(window.reading, now, maxAge, false);
    }
    return copy;
  } catch { return emptyCachedUsageReport('invalid'); }
}

/** Construction is inert; each read projects getSnapshot() once, without store actions. */
export function createCachedUsageReader(options: CachedUsageMcpOptions): () => CachedUsageReport {
  const checked = (values: readonly string[]) => {
    if (!Array.isArray(values) || values.length > 32 || values.some(value=>!id(value))) throw new Error('INVALID_USAGE_SCOPE');
    return new Set(values);
  };
  const connections = checked(options.scope.connectionIds), accounts = checked(options.scope.accountIds);
  const mappings = (options.mappings ?? []).map(mapping=>({consumerAccountId:mapping.consumerAccountId,kitAccountId:mapping.kitAccountId}));
  if (mappings.length > 32 || mappings.some(mapping=>!id(mapping.consumerAccountId) || !accounts.has(mapping.kitAccountId))
      || new Set(mappings.map(mapping=>mapping.consumerAccountId)).size !== mappings.length) throw new Error('INVALID_USAGE_MAPPING');
  const maxAge = options.maxObservationAgeMs ?? 60_000;
  if (!Number.isSafeInteger(maxAge) || maxAge < 1000 || maxAge > 86_400_000) throw new Error('INVALID_USAGE_AGE');
  const store = options.store, clock = options.now ?? Date.now;
  const report = (): CachedUsageReport => {
    try {
      const state = store.getSnapshot();
      if (!state.snapshot || state.error) return emptyCachedUsageReport('unavailable');
      const snapshot = state.snapshot, now = clock();
      if (!Number.isFinite(now) || !['live','synthetic'].includes(snapshot.mode)
          || snapshot.quotaPools.length > 128 || snapshot.connections.length > 512 || snapshot.routes.length > 2048
          || snapshot.accounts.length > 512 || !id(snapshot.revision) || !time(snapshot.observedAt)) invalid();
      const visibleConnections = snapshot.connections.filter(connection=>connections.has(connection.id));
      if (visibleConnections.some(connection=>!id(connection.providerId))) invalid();
      const visibleIds = new Set(visibleConnections.map(connection=>connection.id));
      const routes = new Set(snapshot.routes.filter(route=>visibleIds.has(route.connectionId)).map(route=>route.id));
      const visibleAccounts = new Set(snapshot.accounts.filter(account=>accounts.has(account.id)).map(account=>account.id));
      const present = new Set(visibleConnections.flatMap(connection=>connection.accountId && visibleAccounts.has(connection.accountId) ? [connection.accountId] : []));
      let windowCount = 0;
      const pools: QuotaPool[] = [];
      for (const pool of snapshot.quotaPools) {
        if (!Array.isArray(pool.connectionIds) || pool.connectionIds.length > 512
            || !Array.isArray(pool.routeIds) || pool.routeIds.length > 2048) invalid();
        const ids = pool.connectionIds.filter(connection=>visibleIds.has(connection));
        if (!ids.length || (pool.accountId != null && !accounts.has(pool.accountId))) continue;
        if (!id(pool.id) || !Array.isArray(pool.windows) || pool.windows.length > 128) invalid();
        const windows: UsageWindow[] = pool.windows.map(window=>{
          if (++windowCount > 512 || !id(window.id)) invalid();
          return {id:window.id,label:'Usage window',reading:reading(window.reading,now,maxAge,state.loading),
            resetsAt:time(window.resetsAt) ?? null,
            durationSeconds:Number.isSafeInteger(window.durationSeconds) && (window.durationSeconds ?? 0) > 0 ? window.durationSeconds : null};
        });
        if (pool.accountId) present.add(pool.accountId);
        pools.push({id:pool.id,accountId:pool.accountId ?? null,connectionIds:ids,
          routeIds:pool.routeIds.filter(route=>routes.has(route) && id(route)),
          sharing:typeof pool.sharing === 'string' && /^[a-z_]{1,64}$/.test(pool.sharing) ? pool.sharing : 'unknown',windows});
      }
      const result: CachedUsageReport = {
        state:'cached',mode:snapshot.mode,revision:snapshot.revision,observedAt:snapshot.observedAt,
        quotaPools:pools,connections:visibleConnections.map(connection=>({id:connection.id,providerId:connection.providerId})),
        mappings:mappings.map(mapping=>present.has(mapping.kitAccountId)
          ? {...mapping,state:'explicit'} : {consumerAccountId:mapping.consumerAccountId,state:'unknown'}),
        unmappedAccountIds:[...new Set(pools.flatMap(pool=>pool.accountId ? [pool.accountId] : []))]
          .filter(account=>!mappings.some(mapping=>mapping.kitAccountId === account)).sort(),
        contextCounters:'not_reported',noticeCodes:['CACHE_ONLY','MISSING_WINDOWS_ARE_UNKNOWN','NO_ACCOUNT_MAPPING_INFERENCE','NO_RESET_RECOVERY_INFERENCE'],
      };
      if (new TextEncoder().encode(JSON.stringify(result)).length > 128*1024) invalid();
      return result;
    } catch { return emptyCachedUsageReport('invalid'); }
  };
  return report;
}

/** Constructor is inert. Only tools/call reads getSnapshot(); no store actions are accepted. */
export function createCachedUsageMcp(options: CachedUsageMcpOptions): CachedUsageMcpHandler {
  const report = createCachedUsageReader(options);
  let phase: 'new'|'offered'|'ready' = 'new';
  return message => {
    const packet = object(message) ? message : {};
    const requestId = typeof packet.id === 'string' && packet.id.length <= 128 || Number.isSafeInteger(packet.id) ? packet.id : null;
    const error = (code: number, text: string) => ({jsonrpc:'2.0',id:requestId,error:{code,message:text}});
    const success = (result: unknown) => ({jsonrpc:'2.0',id:requestId,result});
    if (packet.jsonrpc !== '2.0' || typeof packet.method !== 'string' || (packet.params !== undefined && !object(packet.params))) return error(-32600,'Invalid MCP request');
    const params = (packet.params ?? {}) as Record<string,unknown>;
    if (!Object.hasOwn(packet,'id')) {
      if (packet.method === 'notifications/initialized' && phase === 'offered') phase = 'ready';
      return undefined;
    }
    if (requestId === null) return error(-32600,'Invalid MCP request ID');
    if (packet.method === 'ping') return success({});
    if (packet.method === 'initialize') {
      if (phase !== 'new') return error(-32600,'Already initialized');
      if (typeof params.protocolVersion !== 'string' || !object(params.capabilities) || !object(params.clientInfo)) return error(-32602,'Invalid initialization');
      phase = 'offered';
      return success({protocolVersion:'2025-06-18',capabilities:{tools:{listChanged:false}},
        serverInfo:{name:'originlm-cached-usage',version:'0.1.0-alpha.21'}});
    }
    if (phase !== 'ready') return error(-32000,'Initialize MCP first');
    if (packet.method === 'tools/list') {
      if (Object.keys(params).some(key=>key !== '_meta')) return error(-32602,'No tool-list cursor');
      return success({tools:[cachedUsageToolDefinition]});
    }
    if (packet.method === 'tools/call') {
      if (params.name !== 'kit_usage_cached' || Object.keys(params).some(key=>!['name','arguments','_meta'].includes(key))
          || (params.arguments !== undefined && (!object(params.arguments) || Object.keys(params.arguments).length))) return error(-32602,'Invalid cached-usage arguments');
      const data = report();
      return success({content:[{type:'text',text:JSON.stringify(data)}],structuredContent:data,isError:false});
    }
    return error(-32601,'MCP method unavailable');
  };
}
