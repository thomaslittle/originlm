/** Safe SDK errors never include raw response bodies, tokens, or local paths. */
export class LocalAIError extends Error {
  readonly code: string;
  readonly retryable: boolean;
  readonly retryAfterMs?: number;
  constructor(code: string, retryable = false, retryAfterMs?: number) {
    const safe = /^[A-Z][A-Z0-9_]{0,63}$/.test(code) ? code : 'PROTOCOL_INVALID';
    super(`Local AI request failed (${safe})`);
    this.name = 'LocalAIError'; this.code = safe; this.retryable = retryable;
    if (retryAfterMs !== undefined) this.retryAfterMs = retryAfterMs;
  }
}
export function remoteError(data: unknown): LocalAIError {
  if (!data || typeof data !== 'object' || Array.isArray(data)) return new LocalAIError('REMOTE_ERROR');
  const value = data as { code?: unknown; retryable?: unknown; retryAfterMs?: unknown };
  const code = typeof value.code === 'string' ? value.code : 'REMOTE_ERROR';
  const retryable = value.retryable === true;
  if (value.retryAfterMs !== undefined
    && (!retryable || !Number.isSafeInteger(value.retryAfterMs) || (value.retryAfterMs as number) < 0 || (value.retryAfterMs as number) > 86_400_000)) {
    return new LocalAIError('PROTOCOL_INVALID');
  }
  return new LocalAIError(code, retryable, value.retryAfterMs as number | undefined);
}
export function requireMethod(method: string): void {
  if (typeof method !== 'string' || method.length > 128 || !/^[a-zA-Z][a-zA-Z0-9]*(?:\.[a-zA-Z][a-zA-Z0-9]*)+$/.test(method)) throw new LocalAIError('INVALID_METHOD');
}
export function limit(value: number, min = 1024, max = 16 * 1024 * 1024): number {
  if (!Number.isSafeInteger(value) || value < min || value > max) throw new LocalAIError('INVALID_LIMIT');
  return value;
}
