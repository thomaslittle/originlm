/** Protocol transport types. Provider domain records remain canonical in Rust. */
export interface CallOptions { signal?: AbortSignal; timeoutMs?: number }
export interface EventQuery { sessionId?: string; after?: string; scope?: 'inventory' }
export interface Transport {
  request<T = unknown>(method: string, params?: unknown, options?: CallOptions): Promise<T>;
  subscribe?<T = unknown>(query?: EventQuery, options?: CallOptions): AsyncIterable<T>;
  close(): Promise<void>;
}
export interface Handshake { protocolVersion: string; methods: string[]; runtimeVersion?: string; capabilities?: string[] }
export interface RpcFailure { code: number; message: string; data?: { code?: string; retryable?: boolean } }
export interface SSEFrame { id?: string; event: string; data: string }
