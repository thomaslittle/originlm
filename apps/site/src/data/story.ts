/* The one request the page walks through. Method names and field names match
   packages/sdk-typescript/src/generated.ts (snapshot.get -> quotaPools[].windows[] with
   reading.state known | unknown | unavailable and resetsAt). Every value is example data
   and is labelled as such where it is shown. Unknown and unavailable are states, never 0. */

export type Reading =
  | { state: "known"; used: number }
  | { state: "unknown"; reason: string }
  | { state: "unavailable"; reason: string };

export type StoryNode = {
  id: "claude" | "codex" | "cursor" | "gemini" | "ollama";
  name: string;
  provider: string;
  account: string;
  auth: string;
  window: string | null;
  reading: Reading;
  resetsAt: string | null;
  route: string | null;
  eligible: boolean | null;
  why: string;
};

export const STORY_NODES: StoryNode[] = [
  { id: "claude", name: "Claude Code", provider: "claude_code", account: "Personal", auth: "signed in", window: "5-hour", reading: { state: "known", used: 62 }, resetsAt: "in 3h 12m", route: "route-claude-personal", eligible: true, why: "subscription, signed in" },
  { id: "codex", name: "Codex", provider: "codex", account: "Work", auth: "signed in", window: "5-hour", reading: { state: "known", used: 84 }, resetsAt: "in 41m", route: "route-codex-work", eligible: true, why: "subscription, signed in" },
  { id: "cursor", name: "Cursor", provider: "cursor", account: "Personal", auth: "cached", window: "monthly", reading: { state: "known", used: 41 }, resetsAt: "in 12d", route: null, eligible: null, why: "" },
  { id: "gemini", name: "Gemini CLI", provider: "gemini_cli", account: "Personal", auth: "not checked", window: null, reading: { state: "unknown", reason: "not reported by the client" }, resetsAt: null, route: "route-gemini-personal", eligible: false, why: "sign-in not verified" },
  { id: "ollama", name: "Ollama", provider: "ollama", account: "This device", auth: "reachable", window: null, reading: { state: "unavailable", reason: "local route, no quota window" }, resetsAt: null, route: "route-ollama-local", eligible: true, why: "local" },
];

export type StepId = "discover" | "select" | "grant" | "run";
export const STEPS: { id: StepId; label: string; method: string; caption: string }[] = [
  { id: "discover", label: "Discover", method: "snapshot.get", caption: "OriginLM finds the Claude Code, Codex, Cursor, Gemini CLI and Ollama installs already on a machine." },
  { id: "select", label: "Select", method: "selection.resolve", caption: "Pick a route by capability. Ineligible routes say why." },
  { id: "grant", label: "Grant", method: "grants.request", caption: "Discovery is not permission. grants.request only asks: it returns requires_host_approval and changes nothing." },
  { id: "run", label: "Run", method: "turns.start", caption: "Open a session, stream the turn. Typed events, from a cursor." },
];

export const CODE: Record<StepId, string> = {
  discover: `const { connections, quotaPools } = await ai.snapshot.get();`,
  select: `const [pick] = await ai.selection.resolve({
  requiredCapabilities: ["generation.text"],
  eligibleOnly: true,
});`,
  grant: `await ai.grants.request({
  connectionIds: [pick.connectionId],
  allowPaid: false,
}); // never approves itself`,
  run: `const session = await ai.sessions.create({
  selection: pick.selection, workspaceId: "notes",
});
const events = ai.events.subscribe({ sessionId: session.id });
await ai.turns.start({ sessionId: session.id, requestId, prompt });`,
};

export const REPLY = "Three notes mention the launch. Two are drafts; one has a date: Thursday.";

export const EVENTS: { type: string; detail: (chars: number) => string; at: number }[] = [
  { type: "state", detail: () => "running", at: 0.08 },
  { type: "text_delta", detail: (chars) => `${chars} chars`, at: 0.22 },
  { type: "usage", detail: () => "unit tokens, used 412", at: 0.86 },
  { type: "final", detail: () => "completed", at: 0.93 },
];

/* Where each scroll phase sits inside the story's 0..1 progress. Shared with the scene. */
export const phase = (progress: number) => {
  const part = (a: number, b: number) => Math.min(1, Math.max(0, (progress - a) / (b - a)));
  return { discover: part(0, 0.25), select: part(0.25, 0.5), grant: part(0.5, 0.75), run: part(0.75, 1) };
};
/** A row is found when its node pings in the scene (see eye-scene.ts `found`). */
export const rowFoundAt = (index: number) => 0.08 + index * 0.17 + 0.03;
