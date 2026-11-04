/* Shared, dependency-free state between the page (React), the scroll director and the
   WebGL scene. Lives in the main bundle; the director and the scene are lazy chunks that
   read and write it. Everything here is plain numbers so the scene can be posed for
   poster capture from a URL or a test. */

export type SceneLayout = "wide" | "narrow";

export type SceneState = {
  /** Scroll from the hero into the story, 0..1. */
  heroOut: number;
  /** Pinned story progress, 0..1: Discover, Select, Grant, Run in quarters. */
  story: number;
  /** Pinned "no server" pull-back, 0..1. */
  pull: number;
  /** The scene sinking under the documentation sections, 0..1. */
  sink: number;
  /** The closing install moment, 0..1. */
  cta: number;
  /** Fine-pointer position in client pixels, or null when there is none. */
  pointer: { x: number; y: number } | null;
  /** Client-pixel point the eye should look at instead of the pointer (the install button). */
  lookAt: { x: number; y: number } | null;
  /** performance.now() of a requested blink, or -1. */
  blinkAt: number;
  /** Seconds into the intro; null runs it on the clock. Set for stills. */
  introOverride: number | null;
  /** Fixed time for deterministic stills; null uses the clock. */
  timeOverride: number | null;
  /** Right edge (client px) of the hero and closing headlines, so the eye stays clear of them. */
  clearOf: { hero: number; cta: number };
};

export const sceneState: SceneState = {
  heroOut: 0,
  story: 0,
  pull: 0,
  sink: 0,
  cta: 0,
  pointer: null,
  lookAt: null,
  blinkAt: -1,
  introOverride: null,
  timeOverride: null,
  clearOf: { hero: 0, cta: 0 },
};

/* A tiny store for the story progress, so React can render the typed code and the
   morphing table from the same number that drives the scene. */
type Listener = () => void;
const listeners = new Set<Listener>();
let storyValue = 0;
let frame = 0;

export const storyStore = {
  get: () => storyValue,
  set(value: number) {
    const next = Math.min(1, Math.max(0, value));
    sceneState.story = next;
    if (next === storyValue) return;
    storyValue = next;
    if (typeof window === "undefined" || frame) return;
    frame = window.requestAnimationFrame(() => {
      frame = 0;
      listeners.forEach((listener) => listener());
    });
  },
  subscribe(listener: Listener) {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  },
};

/* Positions of projected anchors (node labels, outline labels), written by the scene each
   frame and read by the label layer without React. */
export type Anchor = { x: number; y: number; alpha: number; scale: number };
export const anchorSink: { write: ((anchors: Record<string, Anchor>) => void) | null } = { write: null };

export const NODE_IDS = ["claude", "codex", "cursor", "gemini", "ollama"] as const;
export type NodeId = (typeof NODE_IDS)[number];
