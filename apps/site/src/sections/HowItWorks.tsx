import { useEffect, useRef, useState } from "react";
import { CodeBlock } from "../components/CodeBlock";

const STEPS = [
  {
    id: "discover",
    label: "Discover",
    title: "Discover what is already there",
    body: "A passive inventory of installations, profiles and endpoints. It reads install locations and provider-reported metadata, never browser cookies, and never on import.",
    code: `// Passive inventory: locations, profiles, endpoints.
await ai.discovery.refresh({ level: "passive" });
const { connections, providers } = await ai.snapshot.get();`,
  },
  {
    id: "grant",
    label: "Grant",
    title: "Ask for exactly what you need",
    body: "Discovery is not permission. The SDK can request host approval; that request does not grant access. A trusted host supplies policy for routes, workspaces and metered billing.",
    code: `// A request only; the trusted host must approve policy.
const request = await ai.grants.request({
  scopes: ["snapshot.read", "sessions.run"],
  connectionIds: [selectedConnection],
  allowPaid: false,
});
// Result: { status: "requires_host_approval" }`,
  },
  {
    id: "session",
    label: "Session",
    title: "Run in a session you own",
    body: "A selection names the route, model and revision. The session streams events; cancellation only touches work this app started.",
    code: `const session = await ai.sessions.create({
  selection: { routeId, modelId, revision },
  workspaceId,
});
const turn = await ai.turns.start({
  sessionId: session.id,
  prompt,
  requestId: crypto.randomUUID(),
});`,
  },
];

function DiscoverVisual() {
  return (
    <figure className="shot shot--panel" data-slot="connections" data-kind="product">
      <img
        src="/screens/control-center-connections.webp"
        width={2880}
        height={1800}
        loading="lazy"
        alt="The control center's Connections screen with example data: authenticated profiles for Codex, Antigravity, Grok and OpenCode, each with plan, profile, account and installation."
      />
    </figure>
  );
}

function GrantVisual() {
  return (
    <figure className="shot shot--panel" data-slot="runtime" data-kind="product">
      <img src="/screens/control-center-diagnostics.webp" width={2880} height={2000} loading="lazy"
        alt="OriginLM Diagnostics in example mode, showing runtime and grant evidence. This is not an approval prompt." />
    </figure>
  );
}

function SessionVisual() {
  return (
    <figure className="shot shot--panel" data-slot="playground" data-kind="product">
      <img src="/screens/control-center-playground.webp" width={2880} height={2000} loading="lazy"
        alt="The real OriginLM Playground in example mode, ready for explicit route selection and a gated turn. No provider request has run." />
    </figure>
  );
}

const VISUALS = [DiscoverVisual, GrantVisual, SessionVisual];

export function HowItWorks() {
  const [active, setActive] = useState(0);
  const sectionRef = useRef<HTMLElement>(null);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const manualUntil = useRef(0);

  useEffect(() => {
    let frame = 0;
    const onScroll = () => {
      if (frame) return;
      frame = window.requestAnimationFrame(() => {
        frame = 0;
        if (performance.now() < manualUntil.current) return;
        const element = sectionRef.current;
        if (!element) return;
        const rect = element.getBoundingClientRect();
        const viewport = window.innerHeight;
        if (rect.bottom < viewport * 0.25 || rect.top > viewport * 0.75) return;
        const progress = Math.min(1, Math.max(0, (viewport * 0.62 - rect.top) / Math.max(1, rect.height - viewport * 0.35)));
        setActive(Math.min(2, Math.floor(progress * 3)));
      });
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => {
      window.removeEventListener("scroll", onScroll);
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, []);

  function select(index: number) {
    manualUntil.current = performance.now() + 4500;
    setActive(index);
  }

  return (
    <section className="section how" id="how" aria-labelledby="how-title" ref={sectionRef}>
      <div className="container">
        <div className="head-split">
          <h2 id="how-title">Discover, grant, session. Permission before execution.</h2>
          <p className="lead">
            These SDK fragments assume a connected client and a host-selected route and workspace.
            A grant request alone never enables execution.
          </p>
        </div>

        <div className="how-grid">
          <ol className="how-steps" role="tablist" aria-label="How OriginLM works" aria-orientation="vertical">
            {STEPS.map((step, index) => (
              <li key={step.id} role="presentation">
                <button
                  type="button"
                  role="tab"
                  ref={(node) => { tabRefs.current[index] = node; }}
                  tabIndex={active === index ? 0 : -1}
                  onKeyDown={(event) => {
                    const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
                    if (!keys.includes(event.key)) return;
                    event.preventDefault();
                    const next = event.key === "Home" ? 0 : event.key === "End" ? STEPS.length - 1
                      : (index + (event.key === "ArrowDown" ? 1 : -1) + STEPS.length) % STEPS.length;
                    select(next);
                    tabRefs.current[next]?.focus();
                  }}
                  className="how-step"
                  aria-selected={active === index}
                  aria-controls={`how-panel-${step.id}`}
                  id={`how-tab-${step.id}`}
                  data-active={active === index}
                  onClick={() => select(index)}
                >
                  <span className="how-step-text">
                    <h3>{step.title}</h3>
                    <p>{step.body}</p>
                  </span>
                </button>
              </li>
            ))}
          </ol>

          <div className="how-panels">
            {STEPS.map((step, index) => {
              const Visual = VISUALS[index];
              return (
                <div
                  className="how-panel"
                  key={step.id}
                  id={`how-panel-${step.id}`}
                  role="tabpanel"
                  aria-labelledby={`how-tab-${step.id}`}
                  data-active={active === index}
                  hidden={active !== index}
                >
                  <div className="how-visual">
                    <Visual />
                  </div>
                  <CodeBlock code={step.code} title={`${step.id}.ts`} />
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </section>
  );
}
