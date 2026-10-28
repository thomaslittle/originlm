import { LocalAIError, limit } from './errors.ts';
import type { SSEFrame } from './types.ts';
/** Bounded SSE parser. EOF without a terminating blank line does not dispatch. */
export async function* parseSSE(stream: ReadableStream<Uint8Array>, options: { maxEventBytes?: number } = {}): AsyncGenerator<SSEFrame> {
  const max = limit(options.maxEventBytes ?? 1024 * 1024);
  const reader = stream.getReader();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  const encoder = new TextEncoder();
  let line = '', data: string[] = [], event = 'message', id: string | undefined, size = 0, skipLF = false;
  function processLine(): SSEFrame | null {
    const current = line; line = '';
    if (current === '') {
      const frame = data.length ? { ...(id === undefined ? {} : { id }), event, data: data.join('\n') } : null;
      data = []; event = 'message'; size = 0;
      return frame;
    }
    if (current.startsWith(':')) return null;
    const colon = current.indexOf(':');
    const field = colon < 0 ? current : current.slice(0, colon);
    let value = colon < 0 ? '' : current.slice(colon + 1);
    if (value.startsWith(' ')) value = value.slice(1);
    if (field === 'data') data.push(value);
    else if (field === 'event') event = value || 'message';
    else if (field === 'id' && !value.includes('\0')) id = value;
    return null;
  }
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) { decoder.decode(); break; }
      const text = decoder.decode(next.value, { stream: true });
      for (const char of text) {
        if (skipLF) { skipLF = false; if (char === '\n') continue; }
        size += encoder.encode(char).byteLength;
        if (size > max) throw new LocalAIError('FRAME_TOO_LARGE');
        if (char === '\r' || char === '\n') {
          skipLF = char === '\r';
          const frame = processLine(); if (frame) yield frame;
        } else line += char;
      }
    }
  } catch (error) {
    if (error instanceof LocalAIError) throw error;
    throw new LocalAIError('PROTOCOL_INVALID');
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
}
