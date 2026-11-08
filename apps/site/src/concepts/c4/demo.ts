/* The hero demo's state: which route is picked, which routes your-new-app is granted,
   and the progress of one sample turn. Every streamed value is example data. */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { DEMO_ROUTES, tsCall, type DemoRoute } from "../shared/facts";

export type DemoEvent = { type: "state" | "text_delta" | "usage" | "final"; detail: string };
export type Phase = "idle" | "running" | "done";

/* Statements of tsCall() in order; a step is the line where each one starts. */
const STEP_MARKERS = ["selection.resolve", "sessions.create", "events.subscribe", "turns.start", "for await"];

export function stepLines(code: string): number[] {
  const lines = code.split("\n");
  return STEP_MARKERS.map((marker) => lines.findIndex((line) => line.includes(marker)));
}

const USAGE = "unit tokens, used 412";
const SAMPLE_USAGE = 412;

export type Demo = ReturnType<typeof useDemo>;

export function useDemo({ capture, reduced }: { capture: boolean; reduced: boolean }) {
  const [routeId, setRouteId] = useState(DEMO_ROUTES[0].id);
  const [granted, setGranted] = useState<Set<string>>(() => new Set(capture ? [DEMO_ROUTES[0].id] : []));
  const [touched, setTouched] = useState(capture);
  const [phase, setPhase] = useState<Phase>(capture ? "done" : "idle");
  const [step, setStep] = useState(capture ? STEP_MARKERS.length : -1);
  const [chars, setChars] = useState(capture ? DEMO_ROUTES[0].reply.length : 0);
  const [events, setEvents] = useState<DemoEvent[]>(() => (capture ? finishedEvents(DEMO_ROUTES[0]) : []));
  const [nudge, setNudge] = useState(0);
  const [tokens, setTokens] = useState(18240);
  const timers = useRef<number[]>([]);

  const route = useMemo(() => DEMO_ROUTES.find((r) => r.id === routeId) ?? DEMO_ROUTES[0], [routeId]);
  const code = useMemo(() => tsCall(route), [route]);
  const isGranted = granted.has(route.id);

  const clear = () => { timers.current.forEach((t) => window.clearTimeout(t)); timers.current = []; };
  useEffect(() => clear, []);

  const reset = useCallback(() => { clear(); setPhase("idle"); setStep(-1); setChars(0); setEvents([]); }, []);

  const pick = useCallback((id: string) => {
    setTouched(true);
    if (id === routeId) return;
    reset();
    setRouteId(id);
  }, [routeId, reset]);

  const toggleGrant = useCallback(() => {
    setTouched(true);
    setGranted((g) => {
      const next = new Set(g);
      if (next.has(route.id)) next.delete(route.id); else next.add(route.id);
      return next;
    });
    if (isGranted) reset();
  }, [route.id, isGranted, reset]);

  const run = useCallback(() => {
    if (!isGranted) { setNudge((n) => n + 1); return; }
    if (phase === "running") return;
    clear();
    setEvents([]);
    setChars(0);
    setPhase("running");
    if (reduced) {
      setStep(STEP_MARKERS.length);
      setChars(route.reply.length);
      setEvents(finishedEvents(route));
      setPhase("done");
      setTokens((t) => t + SAMPLE_USAGE);
      return;
    }
    const at = (ms: number, fn: () => void) => timers.current.push(window.setTimeout(fn, ms));
    STEP_MARKERS.forEach((_, i) => at(i * 280, () => setStep(i)));
    const startStream = STEP_MARKERS.length * 280;
    at(startStream - 200, () => setEvents([{ type: "state", detail: "running" }]));
    // text_delta in word-sized chunks, like a real stream.
    const words = route.reply.split(/(?<= )/);
    let count = 0;
    words.forEach((word, i) => {
      count += word.length;
      const n = count;
      at(startStream + i * 75, () => {
        setChars(n);
        setEvents((e) => {
          const rest = e.filter((x) => x.type !== "text_delta");
          return [...rest, { type: "text_delta", detail: `${n} chars` }];
        });
      });
    });
    const end = startStream + words.length * 75;
    at(end + 120, () => setEvents((e) => [...e, { type: "usage", detail: USAGE }]));
    at(end + 300, () => {
      setEvents((e) => [...e, { type: "final", detail: "completed" }]);
      setStep(STEP_MARKERS.length);
      setPhase("done");
      setTokens((t) => t + SAMPLE_USAGE);
    });
  }, [isGranted, phase, reduced, route]);

  /* The menu bar's sample token ticker drifts while the page is open. */
  useEffect(() => {
    if (capture || reduced) return;
    const id = window.setInterval(() => setTokens((t) => t + 7 + ((t * 13) % 23)), 2400);
    return () => window.clearInterval(id);
  }, [capture, reduced]);

  return {
    route, routes: DEMO_ROUTES, code, granted, isGranted, touched, phase, step, chars, events, nudge, tokens,
    pick, toggleGrant, run, reset,
  };
}

function finishedEvents(route: DemoRoute): DemoEvent[] {
  return [
    { type: "state", detail: "running" },
    { type: "text_delta", detail: `${route.reply.length} chars` },
    { type: "usage", detail: USAGE },
    { type: "final", detail: "completed" },
  ];
}
