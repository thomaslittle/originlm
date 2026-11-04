import { useId, useRef, useState } from "react";
import { CodeBlock } from "../components/CodeBlock";
import { Title } from "../components/Title";

type Sample = { id: string; label: string; file: string; install: string; code: string };

const SAMPLES: Sample[] = [
  {
    id: "ts",
    label: "TypeScript",
    file: "summarize.ts",
    install: "npm install originlm@alpha",
    code: `import { createLocalAI } from "originlm";

const ai = await createLocalAI(); // a shared broker, or its own helper
try {
  const [pick] = await ai.selection.resolve({
    requiredCapabilities: ["generation.text"],
    eligibleOnly: true,
  });
  if (!pick) throw new Error("No eligible route on this machine");

  const session = await ai.sessions.create({
    selection: pick.selection,
    workspaceId: "notes",
    permissionMode: "read_only",
  });
  const events = ai.events.subscribe({ sessionId: session.id });
  await ai.turns.start({
    sessionId: session.id,
    requestId: crypto.randomUUID(),
    prompt: "Summarize the open questions in these notes.",
  });

  for await (const { payload } of events) {
    if (payload.type === "text_delta") process.stdout.write(payload.text);
    if (payload.type === "final") break;
  }
} finally {
  await ai.close();
}`,
  },
  {
    id: "rust",
    label: "Rust",
    file: "main.rs",
    install: "cargo add originlm@=0.1.0-alpha.25",
    code: `fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Attaches to a shared broker, or embeds the runtime in-process.
    let mut ai = originlm::LocalAi::start()?;

    // An explicit passive refresh. Probes and execution need policy.
    let snapshot = ai.scan()?;
    for route in &snapshot.routes {
        println!("{} on {}", route.display_name, route.provider_id);
    }
    Ok(())
}`,
  },
  {
    id: "python",
    label: "Python",
    file: "usage.py",
    install: "pip install originlm==0.1.0a25",
    code: `from originlm import Client

# Importing never scans, spawns a process or reads a credential.
with Client.auto() as ai:
    snapshot = ai.snapshot.get()
    for pool in snapshot["quotaPools"]:
        for window in pool["windows"]:
            reading = window["reading"]
            # "known", "unknown" or "unavailable". Never a guessed zero.
            print(window["label"], reading["state"], window.get("resetsAt"))`,
  },
];

const FACTS: [string, string][] = [
  ["Inert imports", "Importing the package never scans, launches a process or reads a credential. Discovery runs when you call it."],
  ["Generated types", "Domain and RPC types are generated from the Rust contracts, so all three clients match the runtime."],
  ["Honest readings", "Every value carries its source and freshness. Unknown and unavailable are states, not zeros."],
  ["Resumable streams", "Events resume from a cursor. A gap raises EVENT_GAP instead of silently dropping output."],
  ["No hidden retries", "An ambiguous turn start is never retried or moved to another account or billing route."],
  ["Owned lifetimes", "Closing a client stops its own helper and sessions, never another app’s work."],
];

export function Sdk() {
  const [active, setActive] = useState(SAMPLES[0].id);
  const base = useId();
  const refs = useRef<Record<string, HTMLButtonElement | null>>({});
  const current = SAMPLES.find((sample) => sample.id === active) ?? SAMPLES[0];

  function onKey(event: React.KeyboardEvent, index: number) {
    if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const last = SAMPLES.length - 1;
    const next = event.key === "Home" ? 0 : event.key === "End" ? last
      : event.key === "ArrowRight" ? (index + 1) % SAMPLES.length : (index - 1 + SAMPLES.length) % SAMPLES.length;
    setActive(SAMPLES[next].id);
    refs.current[SAMPLES[next].id]?.focus();
  }

  return (
    <section className="section sdk" id="sdk" aria-labelledby="sdk-title">
      <div className="container sdk-grid">
        <div className="sdk-intro">
          <Title id="sdk-title" text="Three SDKs, one protocol." />
          <p className="lead">
            TypeScript, Rust and Python clients speak the same JSON-RPC protocol to the same runtime.
            Pick a route by capability, open a session, stream the turn.
          </p>
          <dl className="facts">
            {FACTS.map(([term, text]) => (
              <div key={term}><dt>{term}</dt><dd>{text}</dd></div>
            ))}
          </dl>
        </div>

        <div className="sdk-code">
          <div className="tabs" role="tablist" aria-label="SDK language">
            {SAMPLES.map((sample, index) => (
              <button
                key={sample.id}
                ref={(node) => { refs.current[sample.id] = node; }}
                type="button"
                role="tab"
                id={`${base}-${sample.id}`}
                aria-selected={active === sample.id}
                aria-controls={`${base}-panel`}
                tabIndex={active === sample.id ? 0 : -1}
                onClick={() => setActive(sample.id)}
                onKeyDown={(event) => onKey(event, index)}
              >
                {sample.label}
              </button>
            ))}
          </div>
          <div role="tabpanel" id={`${base}-panel`} aria-labelledby={`${base}-${current.id}`}>
            <CodeBlock code={current.code} title={current.file} />
            <p className="sdk-install"><span aria-hidden="true">$ </span><code>{current.install}</code></p>
          </div>
        </div>
      </div>
    </section>
  );
}
