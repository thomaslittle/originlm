/* Sample wire events. Every one is example data, labelled SAMPLE wherever it is shown.
   The shapes follow the real story: routes from DEMO_ROUTES, grants that ask and do not
   approve themselves, unknown readings that stay unknown, paid routes that are refused
   without allowPaid. App names are invented examples. */
import { DEMO_ROUTES } from "../shared/facts";

export type WireEvent = { id: number; time: string; text: string; tone: "ok" | "hold" | "info" };

const APPS = ["photo-sorter", "notes-app", "pr-reviewer", "inbox-triage", "recipe-scaler", "cli-buddy"];
const r = (id: string) => DEMO_ROUTES.find((route) => route.id === id)!;
const route = (id: string) => `${r(id).providerId}/${r(id).model}`;

type Template = (app: string) => Pick<WireEvent, "text" | "tone">;
const TEMPLATES: Template[] = [
  (app) => ({ text: `app=${app} → route=${route("claude")} · grant=execution · stream ok`, tone: "ok" }),
  (app) => ({ text: `app=${app} · snapshot.get · passive · no secrets read`, tone: "info" }),
  (app) => ({ text: `app=${app} → route=${route("codex")} · grant=none · requires_host_approval`, tone: "hold" }),
  (app) => ({ text: `app=${app} → route=${route("ollama")} · local · stream ok`, tone: "ok" }),
  (app) => ({ text: `app=${app} → route=gemini_cli · sign-in not verified · skipped`, tone: "hold" }),
  (app) => ({ text: `app=${app} → route=${route("codex")} · grant=execution · stream ok`, tone: "ok" }),
  (app) => ({ text: `app=${app} → route=openai-api · allowPaid=false · refused`, tone: "hold" }),
  (app) => ({ text: `app=${app} · usage.read · gemini_cli quota · unknown, not reported`, tone: "info" }),
  (app) => ({ text: `app=${app} → route=${route("gemini")} · grant=execution · stream ok`, tone: "ok" }),
  (app) => ({ text: `admin · apps.revoke app=${app} · its sessions ended`, tone: "info" }),
  (app) => ({ text: `app=${app} · turns.cancel · own turn only · cancelled`, tone: "info" }),
  (app) => ({ text: `app=${app} → route=${route("claude")} · approval_request · awaiting host`, tone: "hold" }),
];

const pad = (n: number) => String(n).padStart(2, "0");
const START = 10 * 3600 + 41 * 60 + 7;
const STEPS = [3, 1, 4, 2, 6, 1, 5, 3, 2, 7, 1, 4];

/** Deterministic: the nth event is always the same, so captures are stable. */
export function eventAt(n: number): WireEvent {
  let t = START;
  for (let i = 0; i < n; i++) t += STEPS[i % STEPS.length];
  t %= 86400;
  const time = `${pad(Math.floor(t / 3600))}:${pad(Math.floor((t % 3600) / 60))}:${pad(t % 60)}`;
  const app = APPS[(n * 5 + Math.floor(n / APPS.length)) % APPS.length];
  return { id: n, time, ...TEMPLATES[n % TEMPLATES.length](app) };
}

export const FIRST_EVENTS = (count: number) => Array.from({ length: count }, (_, i) => eventAt(i));
