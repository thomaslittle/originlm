import { LocalAIError } from './errors.ts';
import type { CallOptions, Handshake, Transport } from './types.ts';
import type { MethodInputs, MethodOutputs, Snapshot, Event, InventoryChange } from './generated.ts';
const families = {
  runtime: ['health', 'closeClient'], costs: ['read', 'setPrices'], discovery: ['refresh', 'watch', 'cancel'], snapshot: ['get'],
  connections: ['list', 'setupInstructions', 'authenticate', 'probe'], models: ['list', 'page'], usage: ['read'], capabilities: ['read'],
  selection: ['resolve', 'validate'], grants: ['request', 'list', 'revoke'],
  health: ['get'], issues: ['list', 'acknowledge', 'snooze'], notifications: ['poll', 'configure', 'ack'], apps: ['list', 'revoke'],
  sessions: ['create', 'get', 'suspend', 'resume', 'updateOptions', 'close'], inputs: ['put'], turns: ['start', 'cancel'], approvals: ['respond'],
  artifacts: ['read'], diagnostics: ['report'], adapters: ['list'],
} as const;
type Method = <T = unknown>(params?: unknown, options?: CallOptions) => Promise<T>;
type Input<K extends string> = K extends keyof MethodInputs ? MethodInputs[K] : unknown;
type Output<K extends string> = K extends keyof MethodOutputs ? MethodOutputs[K] : unknown;
type TypedMethod<K extends string> = {} extends Input<K>
  ? <T = Output<K>>(params?: Input<K>, options?: CallOptions) => Promise<T>
  : <T = Output<K>>(params: Input<K>, options?: CallOptions) => Promise<T>;
type Families = { [F in keyof typeof families]: { [M in typeof families[F][number]]: TypedMethod<`${F}.${M}`> } };
interface SnapshotMethods {
  get<T = Snapshot>(params?: { fields?: undefined }, options?: CallOptions): Promise<T>;
  get<K extends keyof Snapshot>(params: { fields: K[] }, options?: CallOptions): Promise<Pick<Snapshot, K>>;
}
export type LocalAIClient = Omit<Families, 'snapshot'> & {
  /** How the runtime is running: a shared broker, a private child, or a caller-supplied transport. */
  mode: TransportMode;
  /** Present when an incompatible shared protocol selected a private helper. */
  fallbackReason: 'FALLBACK_PROTOCOL_MISMATCH' | null;
  auth: { chatgpt: {
    status: TypedMethod<'auth.chatgpt.status'>;
    start: TypedMethod<'auth.chatgpt.start'>;
    cancel: TypedMethod<'auth.chatgpt.cancel'>;
    disconnect: TypedMethod<'auth.chatgpt.disconnect'>;
  } };
  snapshot: SnapshotMethods;
  handshake: Readonly<Handshake>;
  inventory: {
    subscribe(query?: { after?: string }, options?: CallOptions): AsyncIterable<InventoryChange>;
  };
  close(): Promise<void>;
  events: { subscribe<T = Event>(query?: { sessionId?: string; after?: string; scope?: 'inventory' }, options?: CallOptions): AsyncIterable<T> };
};
export type TransportMode = 'shared' | 'child' | 'custom';
export interface AutoTransportOptions {
  /** Override the helper binary. Defaults to LAI_HELPER_PATH, then the packaged binary. */
  executable?: string;
  /** Override the shared broker address. */
  address?: string;
  /** Override the grant descriptor for the shared broker. */
  token?: string;
  /** Provider ids the private helper may probe (passive discovery is separate). */
  probeProviders?: string[];
  /** Disable passive discovery in the private helper. */
  passive?: boolean;
  /** Explicit additions to the minimal inherited helper environment. */
  env?: Record<string, string>;
  cwd?: string;
  waitMs?: number;
}
export interface CreateLocalAIOptions {
  /** An explicit transport. Omit to resolve one for this platform. */
  transport?: Transport;
  /** Options for the automatic transport. */
  auto?: AutoTransportOptions;
  clientName?: string;
  signal?: AbortSignal;
}
async function autoTransport(options: CreateLocalAIOptions): Promise<Transport> {
  const node = (globalThis as { process?: { versions?: { node?: string } } }).process;
  if (!node?.versions?.node) throw new LocalAIError('TRANSPORT_REQUIRED');
  // Resolved at runtime so a browser bundle never pulls in the Node entry.
  const entry = new URL('../node/auto.mjs', import.meta.url).href;
  const auto = (await import(/* @vite-ignore */ entry)) as {
    openAutoTransport: (options: AutoTransportOptions) => Promise<Transport>;
  };
  return auto.openAutoTransport(options.auto ?? {});
}
export async function createLocalAI(options: CreateLocalAIOptions = {}): Promise<LocalAIClient> {
  const transport = options.transport ?? (await autoTransport(options));
  let handshake: Handshake;
  try {
    handshake = await transport.request<Handshake>('runtime.handshake', { protocolVersion: '1.0', clientName: options.clientName ?? 'originlm-sdk' }, { signal: options.signal });
    if (!handshake || !/^1\.\d+$/.test(handshake.protocolVersion) || !Array.isArray(handshake.methods) || !handshake.methods.every(m => typeof m === 'string')) throw new LocalAIError('PROTOCOL_INCOMPATIBLE');
  } catch (error) { await transport.close(); throw error; }
  const methods = new Set(handshake.methods); let closed = false; let closing: Promise<void> | undefined;
  const requireAvailable = (method: string) => {
    if (closed) throw new LocalAIError('CLOSED');
    if (!methods.has(method)) throw new LocalAIError('CAPABILITY_UNSUPPORTED');
  };
  const client: Record<string, unknown> = {};
  for (const [family, names] of Object.entries(families)) {
    const group: Record<string, Method> = {};
    for (const name of names) group[name] = async <T>(params: unknown = {}, call: CallOptions = {}) => {
      const method = `${family}.${name}`; requireAvailable(method);
      return transport.request<T>(method, params, call);
    };
    client[family] = Object.freeze(group);
  }
  client.auth = Object.freeze({ chatgpt: Object.freeze(Object.fromEntries(
    ['status', 'start', 'cancel', 'disconnect'].map(name => [name, async <T>(params: unknown = {}, call: CallOptions = {}) => {
      const method = `auth.chatgpt.${name}`; requireAvailable(method);
      return transport.request<T>(method, params, call);
    }]),
  )) });
  return Object.assign(client, {
    mode: (transport as { mode?: TransportMode }).mode ?? 'custom',
    fallbackReason: (transport as { fallbackReason?: 'FALLBACK_PROTOCOL_MISMATCH' }).fallbackReason ?? null,
    handshake: Object.freeze({ ...handshake, methods: Object.freeze([...handshake.methods]) }),
    async close() {
      if (closing) return closing;
      closed=true;
      closing=(async()=>{
        try { if(methods.has('runtime.closeClient')) await transport.request('runtime.closeClient',{}, {timeoutMs:1000}); }
        catch { /* A disconnected peer must not prevent local cleanup or leak native errors. */ }
        finally { await transport.close(); }
      })();
      return closing;
    },
    events: Object.freeze({ async *subscribe<T>(query = {}, call = {}) {
      requireAvailable('events.subscribe');
      if (!transport.subscribe) throw new LocalAIError('CAPABILITY_UNSUPPORTED');
      yield* transport.subscribe<T>(query, call);
    } }),
    inventory: Object.freeze({ async *subscribe<T = InventoryChange>(query: { after?: string } = {}, call: CallOptions = {}) {
      requireAvailable('events.subscribe');
      if (!transport.subscribe) throw new LocalAIError('CAPABILITY_UNSUPPORTED');
      yield* transport.subscribe<T>({ scope: 'inventory', ...query }, call);
    } }),
  }) as unknown as LocalAIClient;
}
