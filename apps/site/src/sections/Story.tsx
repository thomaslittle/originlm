/* One request, four calls, scrubbed by scroll. Two renderings of the same content:

   .story-static  four steps, fully filled in, with stills of the scene. What reduced
                  motion, no WebGL and no JavaScript get; visually hidden but readable by
                  screen readers when the stage runs.
   .story-stage   the pinned viewport (CSS sticky, so React keeps its DOM). The step word
                  rolls, the quickstart types itself, and one snapshot table morphs with
                  the scene as each client is found, selected, granted and run.

   The stage is aria-hidden: it repeats the static content as motion. */
import { memo, useSyncExternalStore, type CSSProperties } from "react";
import { highlight, highlightTyped } from "../lib/highlight";
import { CODE, EVENTS, REPLY, STEPS, STORY_NODES, phase, rowFoundAt, type Reading, type StepId } from "../data/story";
import { storyStore } from "../scene/state";

const ORDER: StepId[] = ["discover", "select", "grant", "run"];

function ReadingCell({ reading, window, shown }: { reading: Reading; window: string | null; shown: boolean }) {
  if (!shown) return <td className="reading" data-state="pending"><span className="scan" /></td>;
  if (reading.state === "known") {
    return (
      <td className="reading" data-state="known">
        <span className="state">known</span>
        <span className="bar" style={{ "--used": reading.used / 100 } as CSSProperties}><i /></span>
        <span className="num">{reading.used}% used, {window} window</span>
      </td>
    );
  }
  return (
    <td className="reading" data-state={reading.state}>
      <span className="state">{reading.state}</span>
      <span className="why">{reading.reason}</span>
    </td>
  );
}

type PanelState = { found: number; step: number; selecting: boolean; approved: boolean; running: boolean; asking: boolean; typed: number; events: number };

/** Everything the table shows, as a handful of primitives: scrolling re-renders the table
    only when one of them changes, not on every scroll frame. */
function panelState(progress: number): PanelState {
  const p = phase(progress);
  const running = p.run > 0.04;
  return {
    found: STORY_NODES.filter((_, index) => p.discover >= rowFoundAt(index) || progress >= 0.25).length,
    step: progress < 0.25 ? 0 : progress < 0.5 ? 1 : progress < 0.75 ? 2 : 3,
    selecting: p.select > 0.15,
    approved: p.grant > 0.62,
    running,
    asking: p.grant > 0.12 && !running,
    typed: Math.round(REPLY.length * Math.min(1, Math.max(0, (p.run - 0.22) / 0.62))),
    events: EVENTS.filter((event) => p.run >= event.at).length,
  };
}

function DataPanel({ progress }: { progress: number }) {
  return <DataView {...panelState(progress)} />;
}

/** The snapshot table. Rows fill as nodes are found; selection, the grant and the run
    morph the same table instead of replacing it. */
const DataView = memo(function DataView({ found: foundCount, step, selecting, approved, running, asking, typed, events }: PanelState) {
  return (
    <div className="data" data-step={ORDER[step]} data-selecting={selecting || undefined} data-asking={asking || undefined} data-approved={approved || undefined} data-running={running || undefined}>
      <p className="data-head">
        <span>runtime</span>
        <code className="method">{STEPS[step].method}</code>
        <span className="tag">example data</span>
      </p>
      <div className="data-scroll">
        <table className="snap">
          <caption className="visually-hidden">Connections and quota readings in the snapshot, example data</caption>
          <thead>
            <tr>
              <th scope="col">connection</th>
              <th scope="col">auth</th>
              <th scope="col">reading.state</th>
              <th scope="col">resetsAt</th>
              <th scope="col" className="col-route">route</th>
            </tr>
          </thead>
          <tbody>
            {STORY_NODES.map((node, index) => {
              const found = index < foundCount;
              const picked = index === 0 && selecting;
              return (
                <tr key={node.id} data-found={found || undefined} data-picked={picked || undefined} data-ineligible={(selecting && node.eligible === false) || undefined} data-rest={(running && !picked) || undefined}>
                  <th scope="row"><span className="who">{node.provider}</span><span className="acct">{node.account}</span></th>
                  <td>{found ? node.auth : ""}</td>
                  <ReadingCell reading={node.reading} window={node.window} shown={found} />
                  <td className="num">{found ? node.resetsAt ?? "—" : ""}</td>
                  <td className="col-route">
                    {node.route ? (
                      <span data-eligible={node.eligible ? "yes" : "no"}>
                        <code>{node.route}</code>
                        <span>{node.eligible ? "eligible" : `not eligible: ${node.why}`}</span>
                      </span>
                    ) : "—"}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className="grant-sheet" data-open={asking || undefined}>
        <code className="grant-status">{approved ? "{ grant: approved by the host }" : "{ status: \"requires_host_approval\" }"}</code>
        <div className="approval" role="group" aria-label="Host approval, an example of your app's own UI">
          <p className="approval-kicker">Your host decides</p>
          <p className="approval-title"><b>notes-app</b> wants to run turns</p>
          <dl className="approval-grid">
            <div><dt>Connection</dt><dd>Claude Code, Personal</dd></div>
            <div><dt>Workspace</dt><dd>~/notes</dd></div>
            <div><dt>Billing</dt><dd>subscription only</dd></div>
            <div><dt>Paid routes</dt><dd>off</dd></div>
          </dl>
          <p className="approval-state">{approved ? "Approved in the host’s own UI" : "Waiting for the host"}</p>
        </div>
      </div>

      <div className="run-pane" data-open={running || undefined}>
        <ol className="events" aria-label="Session events">
          {EVENTS.map((event, index) => {
            const shown = index < events;
            return (
              <li key={event.type} data-shown={shown || undefined}>
                <span className="seq num">{index + 1}</span>
                <span className="type">{event.type}</span>
                <span className="detail">{event.detail(typed)}</span>
              </li>
            );
          })}
        </ol>
        <p className="reply">
          <span className="reply-ghost" aria-hidden="true">{REPLY}</span>
          <span className="reply-text">{running ? REPLY.slice(0, typed) : ""}{running && typed < REPLY.length ? <span className="caret" /> : null}</span>
        </p>
      </div>
    </div>
  );
});

function fold(code: string) {
  const lines = code.split("\n");
  return lines.length < 3 ? code : `${lines[0]} … ${lines[lines.length - 1].trim()}`;
}

function TypedCode({ progress }: { progress: number }) {
  const counts = ORDER.map((id, index) => Math.round(CODE[id].length * Math.min(1, Math.max(0, (progress - index * 0.25 - 0.015) / 0.15))));
  return <CodeView step={Math.min(3, Math.floor(progress * 4))} counts={counts.join(",")} />;
}

const CodeView = memo(function CodeView({ step, counts: countKey }: { step: number; counts: string }) {
  const counts = countKey.split(",").map(Number);
  return (
    <pre className="story-code-pre">
      <code>
        {ORDER.map((id, index) => {
          const count = counts[index];
          const state = index < step ? "past" : index === step ? "current" : "next";
          // A finished call folds to one line, so the open one always has the room.
          const source = state === "past" ? fold(CODE[id]) : CODE[id];
          return (
            <span key={id} className="chunk" data-state={state}>
              {highlightTyped(source, state === "past" ? source.length : count)}
              {state === "current" && count < CODE[id].length ? <span className="caret" /> : null}
              {index < ORDER.length - 1 && count > 0 ? "\n" : null}
            </span>
          );
        })}
      </code>
    </pre>
  );
});

function Stage() {
  const progress = useSyncExternalStore(storyStore.subscribe, storyStore.get, () => 0);
  const step = Math.min(3, Math.floor(progress * 4 + 1e-6));
  return (
    <div className="story-stage" aria-hidden="true" data-step={ORDER[step]} style={{ "--step": step } as CSSProperties}>
      <div className="story-pin">
        <ol className="story-rail">
          {STEPS.map((item, index) => (
            <li key={item.id} data-state={index < step ? "past" : index === step ? "current" : "next"}>
              <span className="n num">{index + 1}</span>
              <span>{item.label}</span>
              <span className="fill" style={{ "--f": Math.min(1, Math.max(0, progress * 4 - index)) } as CSSProperties} />
            </li>
          ))}
        </ol>
        <div className="story-word" aria-hidden="true">
          <span className="slot">{STEPS.map((item) => <span key={item.id}>{item.label}.</span>)}</span>
        </div>
        <div className="story-caption">
          <span className="slot">{STEPS.map((item) => <span key={item.id}>{item.caption}</span>)}</span>
        </div>
        <div className="story-code">
          <p className="file">quickstart.ts</p>
          <TypedCode progress={progress} />
        </div>
        <DataPanel progress={progress} />
      </div>
    </div>
  );
}

export function Story() {
  return (
    <section className="story" id="story" aria-labelledby="story-title">
      <div className="story-static">
        <div className="container">
          <h2 id="story-title" className="story-title">Discover, select, grant, run.</h2>
          <p className="lead">One request through the real API, step by step. Method names and fields match the TypeScript SDK; the values are example data.</p>
          <ol className="steps">
            {STEPS.map((item, index) => (
              <li key={item.id} className="step">
                <div className="step-copy">
                  <h3><span className="n num">{index + 1}</span> {item.label}</h3>
                  <p>{item.caption}</p>
                  <pre className="step-code"><code>{highlight(CODE[item.id])}</code></pre>
                </div>
                <img
                  className="step-still"
                  src={`/scene/still-${item.id}.webp`}
                  alt={STILL_ALT[item.id]}
                  width={920}
                  height={576}
                  loading="lazy"
                  decoding="async"
                />
                <DataPanel progress={STATIC_AT[item.id]} />
              </li>
            ))}
          </ol>
        </div>
      </div>
      <Stage />
    </section>
  );
}

/* Each static step shows the table as that step leaves it; the grant waits for the host. */
const STATIC_AT: Record<StepId, number> = { discover: 0.245, select: 0.49, grant: 0.64, run: 0.999 };

const STILL_ALT: Record<StepId, string> = {
  discover: "The OriginLM eye with five points of light around it, Claude Code, Codex, Cursor, Gemini CLI and Ollama, each joined to it by a thread.",
  select: "One point, Claude Code, pulled toward the viewer and ringed; the others fall out of focus.",
  grant: "A six-blade aperture closed over the selected client while the host decides.",
  run: "Light travelling along the selected thread into the eye.",
};
