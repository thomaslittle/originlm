/** Light/dark theme as an external store: the <html> class is the source of truth. */
export type Theme = 'dark' | 'light';

const STORAGE_KEY = 'originlm-theme';
const listeners = new Set<() => void>();

export function getTheme(): Theme {
  return document.documentElement.classList.contains('dark') ? 'dark' : 'light';
}

export function subscribeTheme(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setTheme(theme: Theme): void {
  document.documentElement.classList.toggle('dark', theme === 'dark');
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Storage can be unavailable (private mode); the choice still applies to this page.
  }
  for (const listener of listeners) listener();
}

/** Called once before first render; dark is the default workspace theme. */
export function applyStoredTheme(): void {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem(STORAGE_KEY);
  } catch {
    stored = null;
  }
  document.documentElement.classList.toggle('dark', stored !== 'light');
}
