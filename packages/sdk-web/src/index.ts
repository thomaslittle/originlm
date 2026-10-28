/** Browser bridge detection. Importing performs no request and starts nothing. */
export type Bridge = 'extension' | 'pairing' | 'none';

export interface DetectOptions {
  /** How long to wait for the extension content bridge. */
  extensionTimeoutMs?: number;
  /** Loopback origin used by the pairing fallback. */
  pairingOrigin?: string;
  /** How long to wait for the pairing endpoint. */
  pairingTimeoutMs?: number;
}

const PAGE_SOURCE = 'originlm-web';
const EXTENSION_SOURCE = 'originlm-extension';
/** Suggested opt-in HTTP port. The service installer currently enables IPC only. */
export const DEFAULT_PAIRING_ORIGIN = 'http://127.0.0.1:47821';

/**
 * The extension answers a page ping only when its content bridge is loaded.
 * No answer means "not available here", never an error.
 */
export function detectExtension(timeoutMs = 300): Promise<boolean> {
  if (typeof window === 'undefined' || !Number.isFinite(timeoutMs) || timeoutMs < 1 || timeoutMs > 30_000) return Promise.resolve(false);
  const id = `ping-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  return new Promise((resolve) => {
    let settled = false;
    const finish = (value: boolean) => {
      if (settled) return;
      settled = true;
      window.removeEventListener('message', onMessage);
      clearTimeout(timer);
      resolve(value);
    };
    function onMessage(event: MessageEvent) {
      const data = event.data as { source?: unknown; type?: unknown; id?: unknown } | null;
      if (event.source !== window || event.origin !== window.location.origin) return;
      if (!data || data.source !== EXTENSION_SOURCE || data.type !== 'pong' || data.id !== id) return;
      finish(true);
    }
    const timer = setTimeout(() => finish(false), timeoutMs);
    window.addEventListener('message', onMessage);
    window.postMessage({source: PAGE_SOURCE, type: 'ping', id}, window.location.origin);
  });
}

/**
 * Identify the public runtime-info response. CORS denial and opaque responses
 * are unavailable, never proof of an OriginLM runtime or an authenticated grant.
 */
export async function detectPairing(
  origin: string = DEFAULT_PAIRING_ORIGIN,
  timeoutMs = 400,
): Promise<boolean> {
  if (typeof fetch === 'undefined') return false;
  let endpoint: URL;
  try { endpoint = new URL(origin); } catch { return false; }
  if (!['http:', 'https:'].includes(endpoint.protocol) ||
      !['localhost', '127.0.0.1', '[::1]'].includes(endpoint.hostname) ||
      endpoint.username || endpoint.password || endpoint.search || endpoint.hash || endpoint.pathname !== '/') return false;
  if (!Number.isFinite(timeoutMs) || timeoutMs < 1 || timeoutMs > 30_000) return false;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(new URL('/runtime/info', endpoint).toString(), {
      mode: 'cors',
      credentials: 'omit',
      redirect: 'error',
      cache: 'no-store',
      signal: controller.signal,
    });
    if (!response.ok || !response.headers.get('content-type')?.startsWith('application/json') || !response.body) return false;
    const reader = response.body.getReader();
    let bytes = 0;
    let text = '';
    const decoder = new TextDecoder('utf-8', {fatal: true});
    try {
      while (true) {
        const chunk = await reader.read();
        if (chunk.done) break;
        bytes += chunk.value.byteLength;
        if (bytes > 4096) return false;
        text += decoder.decode(chunk.value, {stream: true});
      }
      const info = JSON.parse(text + decoder.decode());
      return info?.pairingRequired === true && /^1\.\d+$/.test(info.protocolVersion);
    } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Extension first, then pairing. `none` means the page should show the
 * "Connect your local AI" flow with an install link.
 */
export async function detect(options: DetectOptions = {}): Promise<Bridge> {
  if (await detectExtension(options.extensionTimeoutMs ?? 300)) return 'extension';
  if (await detectPairing(options.pairingOrigin, options.pairingTimeoutMs ?? 400)) return 'pairing';
  return 'none';
}

/** Badge copy for the three states, with at most three provider names. */
export function badgeLabel(bridge: Bridge, providers: string[] = [], authorized = false): string {
  if (bridge === 'none') return 'Not detected';
  if (!authorized) return bridge === 'pairing' ? 'Runtime available; pairing required' : 'Extension available; connect to continue';
  const shown = providers.slice(0, 3).join(', ');
  const extra = providers.length > 3 ? ` +${providers.length - 3}` : '';
  return shown ? `Local AI connected · ${shown}${extra}` : 'Local AI connected';
}
