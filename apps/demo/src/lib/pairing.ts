import { getPairingCode, issuePairingCode, type LocalAIClient } from 'originlm';
import { connectRuntime } from '@/lib/runtime';

export interface PairingCode {
  code: string;
  expiresInSeconds: number;
  /** True when a fresh code had to be issued because none was current. */
  issued: boolean;
}

/** Ask the local helper for the current pairing code, issuing one when it has none (HTTP 403). */
export async function fetchPairingCode(): Promise<PairingCode> {
  const baseUrl = window.location.origin;
  try {
    return { ...(await getPairingCode({ baseUrl })), issued: false };
  } catch (error) {
    if ((error as { code?: string } | null)?.code !== 'HTTP_403') throw error;
    return { ...(await issuePairingCode({ baseUrl })), issued: true };
  }
}

/** Exchange a pairing code for an app-session token and open an SDK client with it. */
export async function pairWithCode(code: string): Promise<LocalAIClient> {
  const response = await fetch('/v1/pair', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ code }),
    credentials: 'omit',
    cache: 'no-store',
    redirect: 'error',
  });
  if (!response.ok) throw new Error('PAIRING_DENIED');
  const data = (await response.json()) as { token?: unknown };
  if (typeof data.token !== 'string') throw new Error('INVALID_PAIRING');
  return connectRuntime(window.location.origin, data.token);
}
