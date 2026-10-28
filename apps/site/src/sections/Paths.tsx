import { LayeredScreens } from "../components/LayeredScreens";
import { CheckCircle } from "../components/Icon";
import { CodeBlock } from "../components/CodeBlock";

const BUILD_SNIPPET = `import { createLocalAI } from "originlm";

const ai = await createLocalAI();
try {
  // Read evidence through the same runtime as the dashboard.
  const snapshot = await ai.snapshot.get();
  const { providers, models, quotaPools } = snapshot;
  console.log({ providers, models, quotaPools });
} finally {
  await ai.close();
}

// Discovery does not authorize inference.
// Execution needs a selected route and scoped grants.`;

export function Paths() {
  return (
    <div className="container">
      <div className="head-split">
        <h2 id="paths-title">One runtime. A dashboard for you, an SDK for your app.</h2>
        <p className="lead">
          Most tools pick a side: a usage view with no API, or an AI SDK with no idea what the user
          already pays for. OriginLM is both, because both read the same local state.
        </p>
      </div>

      <div className="halves">
        <article className="half half--see" aria-labelledby="see-title">
          <div className="half-copy">
            <h3 id="see-title">See your AI</h3>
            <p>
              <strong>For everyone who uses AI.</strong> A local control center for supported clients, models,
              accounts and subscriptions. See reported allowances and reset times, with missing or stale
              evidence clearly marked.
            </p>
            <ul className="ticks">
              <li><CheckCircle size={16} /> Reported usage, quotas and reset times</li>
              <li><CheckCircle size={16} /> Models per route, subscription or metered</li>
              <li><CheckCircle size={16} /> Observed cost trends and reported issues</li>
            </ul>
          </div>
          <LayeredScreens />
        </article>

        <article className="half half--build" id="developers" aria-labelledby="build-title">
          <div className="half-copy">
            <h3 id="build-title">Build with it</h3>
            <p>
              <strong>For developers.</strong> Put the user’s own AI in your app with one dependency. One
              canonical runtime for discovery and supported provider operations, with permission scoped to your app.
            </p>
            <ul className="ticks">
              <li><CheckCircle size={16} /> Typed SDKs for Rust, npm and Python</li>
              <li><CheckCircle size={16} /> Imports never scan, launch or read credentials</li>
              <li><CheckCircle size={16} /> Scoped, revocable grants for every app</li>
            </ul>
          </div>
          <CodeBlock code={BUILD_SNIPPET} title="your-app.ts" className="half-code" />
          <p className="shot-caption">
            <a className="text-link" href="#integrations">Framework-by-framework guides</a>
          </p>
        </article>
      </div>
    </div>
  );
}
