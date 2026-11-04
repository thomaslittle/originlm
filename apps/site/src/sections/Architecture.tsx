/* "No server, because there isn't one." The camera pulls back until the whole scene sits
   inside one machine outline (drawn by the scene). The sentence arrives word by word with
   the pull (--pull, written by the director). Under it, the request path in plain HTML:
   it reflows to a list on phones and reads in order without CSS. */
import { Fragment, type CSSProperties } from "react";

const NODES = [
  {
    id: "app",
    title: "Your app",
    body: "Electron, Tauri, GPUI, Node, Python, or a browser extension page.",
    chips: ["user clicks Connect"],
  },
  {
    id: "sdk",
    title: "originlm SDK",
    body: "npm, crates.io and PyPI. Imports are inert until you call it.",
    chips: ["shared broker", "stdio helper", "loopback HTTP + pairing", "native messaging"],
  },
  {
    id: "runtime",
    title: "OriginLM runtime",
    body: "Discovery, evidence, grants, sessions and events. Runs on the user’s machine.",
    chips: ["grants", "sessions", "events"],
  },
  {
    id: "clients",
    title: "Installed clients",
    body: "The tools the user already signed in to, driven through their own interfaces.",
    chips: ["Claude Code", "Codex", "Cursor", "OpenCode", "Gemini CLI", "Grok", "Ollama"],
  },
  {
    id: "provider",
    title: "Provider",
    body: "The route’s provider, under its terms. Local routes stay on the device.",
    chips: ["subscription", "metered", "local"],
  },
];

const LINE = "Nothing passes through an OriginLM server, because there isn’t one.";

export function Architecture() {
  const words = LINE.split(" ");
  return (
    <section className="noserver" id="how" aria-labelledby="arch-title">
      <div className="noserver-track">
        <div className="noserver-pin">
          <div className="container noserver-copy">
            <h2 id="arch-title" className="noserver-title">One local runtime between your app and the AI the user already has.</h2>
            <p className="noserver-lead">
              Your code talks to the SDK. The SDK talks to a runtime on the same machine, which drives
              the clients the user installed.
            </p>
            <p className="noserver-line">
              {words.map((word, index) => (
                <Fragment key={index}>
                  <span className="nw" style={{ "--i": index, "--n": words.length } as CSSProperties}>{word}</span>
                  {index < words.length - 1 ? " " : null}
                </Fragment>
              ))}
            </p>
          </div>
        </div>
      </div>

      <div className="container arch">
        <ol className="arch-flow" aria-label="Request path">
          {NODES.map((node, index) => (
            <li key={node.id} className="arch-node" data-node={node.id} data-outside={node.id === "provider" || undefined}>
              <span className="arch-n num" aria-hidden="true">{String(index + 1).padStart(2, "0")}</span>
              <h3>{node.title}</h3>
              <p>{node.body}</p>
              <ul className="arch-chips" aria-label={`${node.title} details`}>
                {node.chips.map((chip) => <li key={chip}>{chip}</li>)}
              </ul>
            </li>
          ))}
        </ol>
        <p className="arch-boundary">
          <span aria-hidden="true" className="arch-edge" />
          Steps one to four run on the user’s machine. The provider sits outside it, unless the route is local.
        </p>
        <aside className="arch-side" aria-labelledby="arch-side-title">
          <h3 id="arch-side-title">Control center</h3>
          <p>
            The user’s view of the same runtime: what each account has left, which apps are
            connected, and a revoke button that ends their sessions. <a href="#usage">See it below</a>.
          </p>
        </aside>
      </div>
    </section>
  );
}
