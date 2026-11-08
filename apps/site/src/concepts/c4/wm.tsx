/* The tiny window manager behind The Desktop: window state (open / minimized /
   closed), stacking order, and where each window's dock icon is, so a minimized
   window can fly into it. */
import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react";

export type WinId =
  | "originlm" | "app" | "before" | "after" | "discover" | "grant" | "call" | "sdk" | "toolkit"
  | "providers" | "grants" | "safety" | "sessions" | "usage" | "install";
export type DockId = "originlm" | "app" | "docs" | "providers" | "grants" | "sessions" | "usage" | "install";
export type WinState = "open" | "min" | "closed";

export const DOCK_OF: Record<WinId, DockId> = {
  originlm: "originlm", app: "app",
  before: "docs", after: "docs", discover: "docs", grant: "docs", call: "docs", sdk: "docs", toolkit: "docs",
  providers: "providers", grants: "grants", safety: "grants", sessions: "sessions", usage: "usage", install: "install",
};
/* Where each dock app lives on the page. */
export const DOCK_SECTION: Record<DockId, string> = {
  originlm: "demo", app: "demo", docs: "problem", providers: "providers", grants: "grants",
  sessions: "sessions", usage: "usage", install: "install",
};

const ALL: WinId[] = Object.keys(DOCK_OF) as WinId[];

type WM = {
  states: Record<WinId, WinState>;
  setState: (id: WinId, state: WinState) => void;
  z: Record<WinId, number>;
  focus: (id: WinId) => void;
  registerDock: (id: DockId, el: HTMLElement | null) => void;
  dockEl: (id: DockId) => HTMLElement | null;
  bounce: (id: DockId) => void;
  bounced: DockId | null;
  reduced: boolean;
  narrow: boolean;
  showDock: () => void;
  dockShown: boolean;
  setDockShown: (shown: boolean) => void;
};

const Ctx = createContext<WM | null>(null);

export function WindowManager({ children, initial, reduced, narrow }: { children: ReactNode; initial: Partial<Record<WinId, WinState>>; reduced: boolean; narrow: boolean }) {
  const [states, setStates] = useState<Record<WinId, WinState>>(() => Object.fromEntries(ALL.map((id) => [id, initial[id] ?? "open"])) as Record<WinId, WinState>);
  const [order, setOrder] = useState<WinId[]>(ALL);
  const [bounced, setBounced] = useState<DockId | null>(null);
  const [dockShown, setDockShown] = useState(true);
  const docks = useRef<Partial<Record<DockId, HTMLElement | null>>>({});
  const bounceTimer = useRef<number>(0);

  const setState = useCallback((id: WinId, state: WinState) => setStates((s) => (s[id] === state ? s : { ...s, [id]: state })), []);
  const focus = useCallback((id: WinId) => setOrder((o) => (o[o.length - 1] === id ? o : [...o.filter((x) => x !== id), id])), []);
  const registerDock = useCallback((id: DockId, el: HTMLElement | null) => { docks.current[id] = el; }, []);
  const dockEl = useCallback((id: DockId) => docks.current[id] ?? null, []);
  const bounce = useCallback((id: DockId) => {
    setBounced(id);
    window.clearTimeout(bounceTimer.current);
    bounceTimer.current = window.setTimeout(() => setBounced(null), 650);
  }, []);
  const showDock = useCallback(() => setDockShown(true), []);

  const z = useMemo(() => Object.fromEntries(order.map((id, i) => [id, 10 + i])) as Record<WinId, number>, [order]);
  const value = useMemo<WM>(() => ({ states, setState, z, focus, registerDock, dockEl, bounce, bounced, reduced, narrow, showDock, dockShown, setDockShown }),
    [states, setState, z, focus, registerDock, dockEl, bounce, bounced, reduced, narrow, showDock, dockShown]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useWM(): WM {
  const wm = useContext(Ctx);
  if (!wm) throw new Error("useWM outside WindowManager");
  return wm;
}

/** Scroll to an in-page id through the shared Lenis anchor handler (falls back to a
 *  normal jump when Lenis is off). */
export function scrollToId(id: string) {
  const a = document.createElement("a");
  a.href = `#${id}`;
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  a.remove();
}
