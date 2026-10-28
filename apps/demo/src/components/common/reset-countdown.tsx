import { useSyncExternalStore } from 'react';

let now = Date.now();
const listeners = new Set<() => void>();
let timer: ReturnType<typeof setInterval> | undefined;
function subscribe(listener: () => void) {
  listeners.add(listener);
  if (!timer) timer = setInterval(() => { now = Date.now(); for (const notify of listeners) notify(); }, 1000);
  return () => { listeners.delete(listener); if (!listeners.size && timer) { clearInterval(timer); timer = undefined; } };
}
export function ResetCountdown({ at }: { at?: string | null }) {
  const current = useSyncExternalStore(subscribe, () => now, () => 0);
  const reset = at ? Date.parse(at) : NaN;
  if (!Number.isFinite(reset)) return <span>Reset not reported</span>;
  const seconds = Math.max(0, Math.ceil((reset - current) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor(seconds % 3600 / 60);
  return <time dateTime={at ?? undefined} title={new Date(reset).toLocaleString()}>{seconds ? `Resets in ${hours ? `${hours}h ` : ''}${minutes}m` : 'Reset time passed; refresh usage'}</time>;
}
