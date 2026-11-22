/* Clean 3-stage logical request flow:
   01 Installed agents  ->  02 OriginLM runtime & SDK (route + grant + code)  ->  03 Your app output */
import { useEffect, useState } from "react";
import horizontalLogo from "./assets/brand/originlm-horizontal-dark.svg";
import { DEMO_ROUTES, PROVIDERS, VERSION, tsCall, type DemoRoute } from "./data/facts";
import { highlight } from "./lib/highlight";
import { ProviderIcon } from "./ProviderIcon";

type AgentState = "exec" | "partial" | "refused";
type Agent = {
  id: string;
  name: string;
  sub: string;
  state: AgentState;
  route: DemoRoute;
  note?: string;
};

const note = (id: string) => PROVIDERS.find((p) => p.id === id)?.note ?? "";
const route = (id: string) => DEMO_ROUTES.find((r) => r.id === id)!;
const bare = (id: string, name: string, providerId: string): DemoRoute => ({
  id, name, providerId, query: id, model: "", kind: "subscription", reply: "",
});

const AGENTS: Agent[] = [
  { id: "codex", name: "Codex", sub: "subscription", state: "exec", route: route("codex") },
  { id: "claude", name: "Claude Code", sub: "subscription", state: "exec", route: route("claude") },
  { id: "ollama", name: "Ollama", sub: "local runtime", state: "exec", route: route("ollama") },
  { id: "gemini", name: "Antigravity", sub: "Gemini 4 Neon · conditional", state: "partial", route: route("gemini"), note: note("antigravity") },
  { id: "cursor", name: "Cursor", sub: "metadata only", state: "refused", route: bare("cursor", "Cursor", "cursor"), note: note("cursor") },
  { id: "copilot", name: "GitHub Copilot", sub: "detected only", state: "refused", route: bare("copilot", "GitHub Copilot", "copilot"), note: note("copilot") },
];

const PROMPT = "Summarize the open questions in these notes.";

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined") return true;
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches || new URLSearchParams(window.location.search).has("still");
  } catch {
    return false;
  }
}

function grantRequestSnippet(agent: Agent): string {
  return `import { createLocalAI } from "originlm";

const ai = await createLocalAI();
try {
  await ai.discovery.refresh({ level: "passive" });
  const { connections } = await ai.snapshot.get();
  const pick = connections.find(
    (c) => c.providerId === "${agent.route.providerId}",
  );
  if (!pick) {
    console.log("${agent.name} not detected on this machine.");
  } else {
    // Discovery is not permission. Ask the host first:
    const res = await ai.grants.request({
      connectionIds: [pick.id],
      workspaceId: "notes",
      allowPaid: false,
    });
    console.log(res); // -> { status: "requires_host_approval" }
  }
} finally {
  await ai.close();
}`;
}

type LogEvent = { n: string; k: string; v: string; tone?: "ok" | "hold" };

function initialEvents(agent: Agent): LogEvent[] {
  return [
    { n: "01", k: "resolve", v: `query "${agent.route.query}"` },
    { n: "02", k: "grant", v: "approved · workspace notes", tone: "ok" },
    { n: "03", k: "state", v: "running", tone: "ok" },
    { n: "04", k: "final", v: `completed (${agent.route.reply.length} chars)`, tone: "ok" },
  ];
}

function StageConnector({ tone }: { tone: "signal" | "amber" }) {
  return (
    <div className={`sb-link sb-link--${tone}`} aria-hidden="true">
      <svg className="sb-link-svg sb-link-svg--h" viewBox="0 0 28 16" fill="none">
        <line x1="0" y1="8" x2="28" y2="8" stroke="currentColor" strokeWidth="1.5" />
        <polyline points="12,4 16.5,8 12,12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      <svg className="sb-link-svg sb-link-svg--v" viewBox="0 0 16 28" fill="none">
        <line x1="8" y1="0" x2="8" y2="28" stroke="currentColor" strokeWidth="1.5" />
        <polyline points="4,12 8,16.5 12,12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

export function RouteWorkbench() {
  const [selectedId, setSelectedId] = useState<string>("codex");
  /* Codex starts granted so first paint streams/renders immediately; switching to another
     executable agent holds at requires_host_approval until execution is granted explicitly. */
  const [grants, setGrants] = useState<Record<string, boolean>>({ codex: true });
  const [copied, setCopied] = useState(false);

  const agent = AGENTS.find((a) => a.id === selectedId) ?? AGENTS[0];
  const canGrant = agent.state !== "refused";
  const isGranted = canGrant && !!grants[agent.id];
  const isHold = canGrant && !isGranted;
  const flowTone = isGranted ? "signal" : "amber";

  const toggleGrant = () => {
    if (!canGrant) return;
    setGrants((prev) => ({ ...prev, [agent.id]: !prev[agent.id] }));
  };

  const waitingForGrant = canGrant && !isGranted;
  const code = waitingForGrant ? grantRequestSnippet(agent) : tsCall(agent.route, PROMPT);
  const codeLines = code.split("\n");

  /* SSR & initial hydration render the full settled Codex reply so no-JS and prerendered
     HTML always display complete output; client interactions animate smoothly after mount. */
  const [events, setEvents] = useState<LogEvent[]>(() => initialEvents(AGENTS[0]));
  const [chars, setChars] = useState<number>(() => AGENTS[0].route.reply.length);
  const [mounted, setMounted] = useState(false);
  const reply = agent.route.reply;

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    if (!mounted) return;
    const reduced = prefersReducedMotion();
    const head: LogEvent[] = [{ n: "01", k: "resolve", v: `query "${agent.route.query}"` }];

    if (agent.state === "refused") {
      const all: LogEvent[] = [...head, { n: "02", k: "result", v: "[] no eligible route", tone: "hold" }];
      if (reduced) { setEvents(all); setChars(0); return; }
      setEvents(head); setChars(0);
      const t = window.setTimeout(() => setEvents(all), 180);
      return () => window.clearTimeout(t);
    }

    if (!isGranted) {
      const held: LogEvent[] = [...head, { n: "02", k: "grant", v: "requires_host_approval", tone: "hold" }];
      if (reduced) { setEvents(held); setChars(0); return; }
      setEvents(head); setChars(0);
      const t = window.setTimeout(() => setEvents(held), 160);
      return () => window.clearTimeout(t);
    }

    const run: LogEvent[] = [
      ...head,
      { n: "02", k: "grant", v: "approved · workspace notes", tone: "ok" },
      { n: "03", k: "state", v: "running", tone: "ok" },
    ];
    const done: LogEvent = { n: "04", k: "final", v: `completed (${reply.length} chars)`, tone: "ok" };

    if (reduced) { setEvents([...run, done]); setChars(reply.length); return; }
    setEvents([]); setChars(0);
    const timers: number[] = [];
    run.forEach((ev, i) => timers.push(window.setTimeout(() => setEvents((prev) => [...prev, ev]), 80 + i * 130)));
    const startAt = 80 + run.length * 130 + 80;
    let n = 0;
    let interval = 0;
    timers.push(window.setTimeout(() => {
      interval = window.setInterval(() => {
        n = Math.min(reply.length, n + 2);
        setChars(n);
        if (n >= reply.length) {
          window.clearInterval(interval);
          setEvents((prev) => [...prev, done]);
        }
      }, 22);
    }, startAt));
    return () => { timers.forEach(window.clearTimeout); window.clearInterval(interval); };
  }, [agent, isGranted, mounted, reply]);

  return (
    <div className="sb-wb" aria-label="Interactive OriginLM routing flow (example data)">
      {/* STAGE 01: INSTALLED AGENTS */}
      <div className="sb-stage sb-stage--agents">
        <div className="sb-stage-head">
          <span className="sb-label">01 · Installed agents</span>
          <span className="sb-label">Example machine</span>
        </div>
        <ul className="sb-agent-list" role="listbox" aria-label="Select an installed agent">
          {AGENTS.map((a) => {
            const active = a.id === agent.id;
            return (
              <li key={a.id}>
                <button
                  type="button"
                  role="option"
                  aria-selected={active}
                  data-state={a.state}
                  className={`sb-agent${active ? " is-active" : ""}${active && isGranted ? " is-granted" : ""}${active && !isGranted ? " is-hold" : ""}`}
                  onClick={() => setSelectedId(a.id)}
                >
                  <ProviderIcon id={a.id} className="sb-agent-icon" />
                  <span className="sb-agent-body">
                    <span className="sb-agent-top">
                      <span className="sb-agent-name">{a.name}</span>
                      <span className="sb-agent-arrow" aria-hidden="true">{active ? "→" : ""}</span>
                    </span>
                    <span className="sb-agent-sub">{a.sub}</span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      </div>

      {/* CONNECTOR 1 -> 2 */}
      <StageConnector tone={flowTone} />

      {/* STAGE 02: ORIGINLM RUNTIME & SDK */}
      <div className="sb-stage sb-stage--runtime">
        <div className="sb-stage-head sb-stage-head--brand">
          <span className="sb-stage-brand">
            <span className="sb-label" aria-hidden="true">02 ·</span>
            <img
              src={horizontalLogo}
              alt="OriginLM"
              className="sb-stage-logo"
              width={87}
              height={20}
            />
          </span>
          <span className={`sb-tag${waitingForGrant ? " sb-tag--amber" : ""}`}>
            {waitingForGrant ? "requires_host_approval" : `SDK ${VERSION}`}
          </span>
        </div>

        <div className={`sb-gate${isGranted ? " is-granted" : isHold ? " is-hold" : " is-locked"}`}>
          <div className="sb-gate-info">
            <span className="sb-gate-title" id="sb-gate-title">
              <span className={`sb-dot ${isGranted ? "sb-dot--signal" : isHold ? "sb-dot--amber" : "sb-dot--none"}`} aria-hidden="true" />
              Execution grant · {agent.name}
            </span>
            <span className="sb-gate-sub" id="sb-gate-sub">
              {agent.state === "refused"
                ? `${agent.route.providerId} · metadata/detection only`
                : isGranted
                  ? `route ${agent.route.providerId} · ${agent.route.model} (example)`
                  : `hold · discovery is not permission`}
            </span>
          </div>
          <button
            type="button"
            role="switch"
            className="sb-rocker"
            disabled={!canGrant}
            aria-checked={isGranted}
            aria-labelledby="sb-gate-title"
            aria-describedby="sb-gate-sub"
            onClick={toggleGrant}
          >
            <span className="sb-rocker-knob" aria-hidden="true" />
          </button>
        </div>

        <figure className="sb-codebox">
          <figcaption className="sb-codebox-head">
            <span className="sb-label">{waitingForGrant ? "your-app/src/grant.ts" : "your-app/src/ask.ts"}</span>
            <button
              type="button"
              className="sb-btn sb-btn--sm"
              aria-label="Copy SDK snippet"
              onClick={() => {
                navigator.clipboard?.writeText(code).then(() => { setCopied(true); window.setTimeout(() => setCopied(false), 1400); }).catch(() => {});
              }}
            >
              {copied ? "Copied" : "Copy"}
            </button>
          </figcaption>
          <pre tabIndex={0} aria-label={`SDK code for ${agent.name}`}>
            <code>
              {codeLines.map((line, i) => {
                const hot = !waitingForGrant && line.includes("query:");
                const warn = waitingForGrant && (line.includes("grants.request") || line.includes("requires_host_approval"));
                return (
                  <span key={i} className={`sb-line${hot ? " is-hot" : ""}${warn ? " is-warn" : ""}`}>
                    <span className="sb-ln" aria-hidden="true">{String(i + 1).padStart(2, " ")}</span>
                    {highlight(line)}
                    {"\n"}
                  </span>
                );
              })}
            </code>
          </pre>
          <div className="sb-codebox-foot">
            {waitingForGrant ? (
              <span><strong>Grant required:</strong> flip Execution grant above to authorize {agent.name}.</span>
            ) : agent.state === "refused" ? (
              <span><strong>{agent.name}:</strong> metadata only — <code>selection.resolve</code> returns <code>[]</code>.</span>
            ) : (
              <span><strong className="is-ok">One integration:</strong> switch agents and only the provider and <code>query</code> change.</span>
            )}
          </div>
        </figure>
      </div>

      {/* CONNECTOR 2 -> 3 */}
      <StageConnector tone={flowTone} />

      {/* STAGE 03: YOUR APP OUTPUT */}
      <div className="sb-stage sb-stage--output" aria-live="polite">
        <div className="sb-stage-head">
          <span className="sb-label">03 · Your app output</span>
          <span className={`sb-tag ${isGranted ? "sb-tag--signal" : "sb-tag--amber"}`}>
            {agent.state === "refused" ? "refused" : isGranted ? "sample · live" : "hold · no grant"}
          </span>
        </div>

        <div className="sb-prompt-box">
          <span className="sb-label">Prompt</span>
          <p>{PROMPT}</p>
        </div>

        <div className="sb-out-well">
          <div className="sb-out-meta">
            <span className="sb-label">Response</span>
            <span className="sb-label">
              {agent.route.model ? `${agent.route.providerId} · ${agent.route.model}` : agent.route.providerId}
            </span>
          </div>

          {agent.state === "refused" ? (
            <p className="sb-out-note">{agent.note}</p>
          ) : !isGranted ? (
            <div className="sb-out-hold">
              <p className="sb-out-note">
                Discovery is not permission. Grant execution for your app on {agent.name} to stream output.
              </p>
              <button type="button" className="sb-btn sb-btn--sm sb-btn--amber" onClick={toggleGrant}>
                Grant execution on {agent.name}
              </button>
            </div>
          ) : (
            <>
              <p className="sb-out-reply">
                {reply.slice(0, chars)}
                {mounted && chars < reply.length ? <span className="sb-caret" aria-hidden="true" /> : null}
              </p>
              {agent.state === "partial" && <p className="sb-out-note">Conditional: {agent.note}</p>}
            </>
          )}
        </div>

        <div className="sb-ev-box">
          <span className="sb-label">events.subscribe</span>
          <ol className="sb-events">
            {events.map((ev) => (
              <li key={ev.n} className={ev.tone ? `is-${ev.tone}` : ""}>
                <span className="sb-ev-n">{ev.n}</span>
                <span className="sb-k">{ev.k}</span>
                <span className="sb-ev-v">{ev.v}</span>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </div>
  );
}
