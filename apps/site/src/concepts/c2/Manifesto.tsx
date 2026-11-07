/* Concept 2 — ONE RUNTIME. A printed manifesto against dashboard-ification: the page
   argues in type, then proves it with one spread of code. Every claim comes from
   ../shared/facts.ts; example values are labelled where they appear. */
import "./c2.css";
import { Fragment, useLayoutEffect, useMemo, useRef, useState } from "react";
import { highlight } from "../../lib/highlight";
import { gsap, ScrollTrigger, useLenis, usePageChrome, useReducedMotion } from "../shared/motion";
import {
  CONTROL_CENTER, DEMO_ROUTES, DISCOVERED, DISCOVERED_TAIL, DOCS, GRANT_SCOPES, HOW, INSTALL, OP_LABELS,
  PITCH, PROBLEM, PROVIDERS, QUICKSTART, REPO, SAFETY, STATUS, STATUS_LONG, TAGLINE, TOOLKIT, VERSION,
  grantManifest, tsCall, type ProviderEntry,
} from "../shared/facts";

const ROMAN = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
const PROBLEM_CLIENTS = DISCOVERED.slice(0, 5);
const PROBLEM_APPS = ["App one", "App two", "App three", "App four", "App five"];

function Copy({ value, label }: { value: string; label: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      className="c2-copy"
      onClick={async () => {
        try { await navigator.clipboard.writeText(value); } catch { /* clipboard unavailable */ }
        setDone(true);
        window.setTimeout(() => setDone(false), 1500);
      }}
      aria-label={`Copy ${label}`}
    >
      <span aria-live="polite">{done ? "Copied" : "Copy"}</span>
    </button>
  );
}

function RunHead({ folio, title, right, page }: { folio: string; title: string; right: string; page: string }) {
  return (
    <div className="c2-runhead" aria-hidden="true">
      <span className="c2-runhead-folio">{folio}</span>
      <span className="c2-runhead-title">{title}</span>
      <span className="c2-runhead-right">{right}</span>
      <span className="c2-runhead-page">{page}</span>
    </div>
  );
}

/* Book-index page numbers: each op is a "page" (1 Found … 8 Resume); consecutive
   confirmed ops collapse to ranges, partial ops carry a dagger. */
function evidence(entry: ProviderEntry): { text: string; partial: boolean }[] {
  const marks = OP_LABELS.map((op, i) => ({ n: i + 1, state: entry.ops[op.key] })).filter((m) => m.state !== "none");
  const out: { text: string; partial: boolean }[] = [];
  let i = 0;
  while (i < marks.length) {
    if (marks[i].state === "partial") { out.push({ text: `${marks[i].n}`, partial: true }); i++; continue; }
    let j = i;
    while (j + 1 < marks.length && marks[j + 1].state === "yes" && marks[j + 1].n === marks[j].n + 1) j++;
    out.push({ text: j > i ? `${marks[i].n}–${marks[j].n}` : `${marks[i].n}`, partial: false });
    i = j + 1;
  }
  return out;
}

type IndexEntry = { name: string; kind: string; marks: { text: string; partial: boolean }[]; sub: string };
function buildIndex(): [string, IndexEntry[]][] {
  const entries: IndexEntry[] = PROVIDERS.map((p) => ({
    name: p.name,
    kind: p.kind === "API route" ? "API route, configured" : p.kind.toLowerCase(),
    marks: evidence(p),
    sub: p.note,
  }));
  /* Kiro is in the README's discovery list but not in the site's condensed matrix. */
  if (!entries.some((e) => e.name === "Kiro") && DISCOVERED.includes("Kiro")) {
    entries.push({ name: "Kiro", kind: "native client", marks: [{ text: "1", partial: false }], sub: "Listed in the README's passive-discovery set. See the support matrix for each operation." });
  }
  entries.sort((a, b) => a.name.localeCompare(b.name));
  const groups = new Map<string, IndexEntry[]>();
  for (const e of entries) {
    const letter = e.name[0].toUpperCase();
    groups.set(letter, [...(groups.get(letter) ?? []), e]);
  }
  return [...groups.entries()];
}

export default function Manifesto() {
  usePageChrome("OriginLM — One Runtime", "c2-body", "#f3eee4");
  const reduced = useReducedMotion();
  useLenis(!reduced);
  const root = useRef<HTMLDivElement>(null);
  const [routeId, setRouteId] = useState(DEMO_ROUTES[0].id);
  const route = DEMO_ROUTES.find((r) => r.id === routeId) ?? DEMO_ROUTES[0];
  const code = useMemo(() => tsCall(route), [route]);
  const lines = code.split("\n");
  const half = Math.ceil(lines.length / 2) + 1;
  const index = useMemo(buildIndex, []);
  const manifest = useMemo(() => grantManifest({ app: "notes-app", providerId: "codex", scopes: { metadata: true, execution: true, workspace: true, tools: false, paid: false } }), []);

  useLayoutEffect(() => {
    if (reduced || !root.current) return;
    const ctx = gsap.context(() => {
      /* Hero: the three lines arrive like type being set, then lock together on scroll. */
      const lines = gsap.utils.toArray<HTMLElement>(".c2-h1-line");
      gsap.from(lines, { yPercent: 60, opacity: 0, duration: 1.1, ease: "power3.out", stagger: 0.14 });
      gsap.from(".c2-hero-dek, .c2-hero-kicker, .c2-cue", { opacity: 0, y: 12, duration: 0.9, delay: 0.55, stagger: 0.08, ease: "power2.out" });
      const hero = gsap.timeline({
        scrollTrigger: { trigger: ".c2-hero", start: "top top", end: "+=110%", scrub: 0.6, pin: ".c2-hero-pin", anticipatePin: 1 },
      });
      hero
        .fromTo(lines[0], { "--w": 300, "--o": 144, "--s": 100, "--ls": "0.02em", x: "0vw" }, { "--w": 600, "--o": 96, "--s": 0, "--ls": "-0.045em", x: "0vw", ease: "none" }, 0)
        .fromTo(lines[1], { "--w": 260, "--o": 144, "--s": 100, "--ls": "0.12em", x: "18vw" }, { "--w": 600, "--o": 96, "--s": 0, "--ls": "-0.045em", x: "0vw", ease: "none" }, 0)
        .fromTo(lines[2], { "--w": 260, "--o": 144, "--s": 100, "--ls": "0.06em", x: "6vw" }, { "--w": 600, "--o": 96, "--s": 0, "--ls": "-0.045em", x: "0vw", ease: "none" }, 0)
        .fromTo(".c2-h1-rule", { scaleX: 0 }, { scaleX: 1, ease: "none" }, 0.2)
        .to(".c2-cue", { opacity: 0, ease: "none", duration: 0.2 }, 0);

      /* Section titles: weight and optical size shift as they cross the viewport. */
      gsap.utils.toArray<HTMLElement>(".c2-kinetic").forEach((el) => {
        gsap.fromTo(el, { "--w": 280, "--s": 100, "--ls": "0.04em", yPercent: 18 }, {
          "--w": 560, "--s": 0, "--ls": "-0.035em", yPercent: 0, ease: "none",
          scrollTrigger: { trigger: el, start: "top 92%", end: "top 40%", scrub: 0.5 },
        });
      });

      /* Marginalia and pull-quotes fade in like print being set. */
      gsap.utils.toArray<HTMLElement>(".c2-set").forEach((el) => {
        gsap.fromTo(el, { opacity: 0, letterSpacing: "0.08em", filter: "blur(2px)" }, {
          opacity: 1, letterSpacing: "normal", filter: "blur(0px)", duration: 1.1, ease: "power2.out",
          scrollTrigger: { trigger: el, start: "top 86%", once: true },
        });
      });

      /* 01: twenty-five integrations collapse into one. */
      const grid = gsap.timeline({ scrollTrigger: { trigger: ".c2-mass", start: "top 70%", end: "bottom 45%", scrub: 0.6 } });
      grid
        .fromTo(".c2-mass-cell", { opacity: 1 }, { opacity: 0.16, stagger: { each: 0.02, from: "random" }, ease: "none" }, 0)
        .fromTo(".c2-mass-strike", { scaleX: 0 }, { scaleX: 1, ease: "none" }, 0.3)
        .fromTo(".c2-mass-one", { opacity: 0, yPercent: 30, "--w": 300 }, { opacity: 1, yPercent: 0, "--w": 600, ease: "none" }, 0.45);

      /* Big folio numerals drift against the scroll. */
      gsap.utils.toArray<HTMLElement>(".c2-folio-big").forEach((el) => {
        gsap.fromTo(el, { yPercent: 20 }, { yPercent: -20, ease: "none", scrollTrigger: { trigger: el.parentElement, start: "top bottom", end: "bottom top", scrub: true } });
      });

      /* Install: the last line sets itself heavy as you arrive. */
      gsap.fromTo(".c2-cta-title", { "--w": 260, "--s": 100, "--ls": "0.05em" }, {
        "--w": 600, "--s": 0, "--ls": "-0.045em", ease: "none",
        scrollTrigger: { trigger: ".c2-cta", start: "top 85%", end: "top 25%", scrub: 0.5 },
      });
    }, root);
    const refresh = window.setTimeout(() => ScrollTrigger.refresh(), 400);
    return () => { window.clearTimeout(refresh); ctx.revert(); };
  }, [reduced]);

  return (
    <div className="c2" ref={root} data-motion={reduced ? "off" : "on"}>
      <a className="c2-skip" href="#main">Skip to content</a>

      <header className="c2-masthead">
        <a className="c2-wordmark" href="#top" aria-label="OriginLM, top of page">OriginLM</a>
        <nav aria-label="Chapters" className="c2-nav">
          <ol>
            <li><a href="#problem"><span>01</span> Problem</a></li>
            <li><a href="#runtime"><span>02</span> Runtime</a></li>
            <li><a href="#index"><span>03</span> Index</a></li>
            <li><a href="#articles"><span>§</span> Articles</a></li>
            <li><a href="#appendix"><span>A</span> Appendix</a></li>
          </ol>
        </nav>
        <a className="c2-mast-cta" href="#install">Install</a>
      </header>

      <main id="main">
        {/* ——— Hero ——— */}
        <section className="c2-hero" id="top" aria-labelledby="c2-h1">
          <div className="c2-hero-pin">
            <div className="c2-hero-top" aria-hidden="true">
              <span>One Runtime</span>
              <span>A manifesto in three chapters and an appendix</span>
              <span>No. {VERSION}</span>
            </div>
            <p className="c2-hero-kicker">Your AI is not a chart.</p>
            <h1 id="c2-h1" className="c2-h1">
              <span className="c2-h1-line">Every model you pay&nbsp;for.</span>
              <span className="c2-h1-line c2-h1-red">Now inside</span>
              <span className="c2-h1-line">software you&nbsp;write.</span>
            </h1>
            <span className="c2-h1-rule" aria-hidden="true" />
            <div className="c2-hero-foot">
              <p className="c2-hero-dek">{PITCH.body}</p>
              <p className="c2-cue" aria-hidden="true">[&ensp;scroll to set the type&ensp;↓&ensp;]</p>
            </div>
          </div>
        </section>

        {/* ——— 01 The Problem ——— */}
        <section className="c2-chapter c2-problem" id="problem" aria-labelledby="c2-problem-title">
          <RunHead folio="01" title="The Problem" right="Five apps, five integrations" page="p. 3" />
          <div className="c2-grid">
            <span className="c2-folio-big" aria-hidden="true">01</span>
            <div className="c2-chapter-head">
              <p className="c2-eyebrow">Chapter one · The Problem</p>
              <h2 id="c2-problem-title" className="c2-h2 c2-kinetic">{PROBLEM.line}</h2>
            </div>
            <div className="c2-body-col c2-dropcap">
              <p>
                Build an app that talks to Codex and you write Codex’s paths, its metadata parser, its streaming
                protocol. Build a second app and you write them again. Add Claude Code, Grok, Gemini CLI, Antigravity:
                again, and again, and again.
              </p>
              <p>
                The work is not the feature you wanted to ship. It is {PROBLEM.burdens.join(", ").replace(/, ([^,]*)$/, " and $1")},
                once per client, once per app, kept current by hand.
              </p>
            </div>
            <aside className="c2-margin c2-set" aria-label="Margin note">
              <span className="c2-margin-mark">¹</span> Five illustrative apps against the first five clients in the
              README’s discovery list. 5 × 5 = 25 integrations to write and maintain.
            </aside>
          </div>

          <figure className="c2-mass" aria-labelledby="c2-mass-cap">
            <div className="c2-mass-table" role="table" aria-label="Five apps, each integrating five AI clients">
              <div className="c2-mass-row c2-mass-headrow" role="row">
                <span role="columnheader" className="c2-mass-corner">App ↓ / client →</span>
                {PROBLEM_CLIENTS.map((client) => <span role="columnheader" key={client}>{client}</span>)}
              </div>
              {PROBLEM_APPS.map((app) => (
                <div className="c2-mass-row" role="row" key={app}>
                  <span role="rowheader" className="c2-mass-app">{app}</span>
                  {PROBLEM_CLIENTS.map((client) => (
                    <span role="cell" className="c2-mass-cell" key={client}>
                      {PROBLEM.burdens.map((b) => <span key={b}>{b}</span>)}
                    </span>
                  ))}
                </div>
              ))}
              <span className="c2-mass-strike" aria-hidden="true" />
              <span className="c2-mass-one" aria-hidden="true">One integration.</span>
            </div>
            <figcaption id="c2-mass-cap" className="c2-figcap">
              <span>Fig. 1</span> Twenty-five integrations, or one: your app speaks to OriginLM, and OriginLM speaks to each client.
            </figcaption>
          </figure>

          <blockquote className="c2-pull c2-set">
            <p>“{PROBLEM.fix}”</p>
            <footer>From the README</footer>
          </blockquote>
        </section>

        {/* ——— 02 The Runtime ——— */}
        <section className="c2-chapter c2-runtime" id="runtime" aria-labelledby="c2-runtime-title">
          <RunHead folio="02" title="The Runtime" right="Discover, grant, call" page="p. 7" />
          <div className="c2-grid">
            <span className="c2-folio-big" aria-hidden="true">02</span>
            <div className="c2-chapter-head">
              <p className="c2-eyebrow">Chapter two · The Runtime</p>
              <h2 id="c2-runtime-title" className="c2-h2 c2-kinetic">Discover. Grant. Call.</h2>
              <p className="c2-standfirst">
                One local runtime and a Rust toolkit sit between your software and the AI already on the machine. It does
                three things, in this order, and never skips the second.
              </p>
            </div>
          </div>

          <div className="c2-essay">
            {HOW.map((step, i) => (
              <article className="c2-essay-col" key={step.id} aria-labelledby={`c2-how-${step.id}`}>
                <p className="c2-essay-n">{ROMAN[i]}.</p>
                <h3 id={`c2-how-${step.id}`} className="c2-h3">{step.verb}</h3>
                <p className="c2-essay-method"><code>{step.method}</code></p>
                <p className="c2-essay-body c2-dropcap">{step.body}<sup className="c2-fnref">{i + 1}</sup></p>
              </article>
            ))}
          </div>
          <ol className="c2-footnotes">
            {HOW.map((step, i) => <li key={step.id}><sup>{i + 1}</sup> {step.detail}</li>)}
          </ol>

          {/* The spread: the whole call, printed across two facing pages. */}
          <figure className="c2-spread" aria-labelledby="c2-spread-cap">
            <div className="c2-spread-bar">
              <p className="c2-spread-label"><span>Plate I</span> The whole call, set for</p>
              <div className="c2-routes" role="group" aria-label="Route to set the call for">
                {DEMO_ROUTES.map((r) => (
                  <button key={r.id} type="button" aria-pressed={r.id === route.id} onClick={() => setRouteId(r.id)}>
                    {r.name}
                  </button>
                ))}
              </div>
            </div>
            <div className="c2-spread-pages">
              {[lines.slice(0, half), lines.slice(half)].map((page, p) => (
                <pre className={`c2-page c2-page-${p ? "verso" : "recto"}`} key={p} tabIndex={0} aria-label={p ? "Code, second page" : "Code, first page"}>
                  <code>
                    {page.map((line, i) => {
                      const n = (p ? half : 0) + i + 1;
                      const changed = line.includes("query:");
                      return (
                        <span className={`c2-line${changed ? " c2-line-changed" : ""}`} key={n}>
                          <span className="c2-ln" aria-hidden="true">{n}</span>
                          <span className="c2-lc">{line ? highlight(line) : " "}</span>
                          {changed && <span className="c2-line-note" aria-hidden="true">← the only line that changes</span>}
                        </span>
                      );
                    })}
                  </code>
                </pre>
              ))}
              <span className="c2-gutter" aria-hidden="true" />
            </div>
            <div className="c2-spread-foot">
              <figcaption id="c2-spread-cap" className="c2-figcap">
                <span>Plate I</span> {lines.length} lines, as printed, from the import to the final event. Pick another
                route above: only the <code>query</code> changes. Native model ids are preserved, with no universal
                Low/Medium/High.
              </figcaption>
              <div className="c2-proof" aria-live="polite">
                <p className="c2-proof-label">Proof · example output, streamed as <code>text_delta</code> events</p>
                <p className="c2-proof-text">{route.reply}</p>
                <p className="c2-proof-meta">
                  {route.name} · {route.kind === "local" ? "local route" : "the user’s own subscription"} · example model id <code>{route.model}</code>
                </p>
              </div>
            </div>
          </figure>
        </section>

        {/* ——— SDK / quickstart ——— */}
        <section className="c2-chapter c2-sdk" id="sdk" aria-labelledby="c2-sdk-title">
          <RunHead folio="02·b" title="The Runtime, continued" right="Three languages, one protocol" page="p. 11" />
          <div className="c2-grid">
            <div className="c2-chapter-head">
              <p className="c2-eyebrow">Quickstart</p>
              <h2 id="c2-sdk-title" className="c2-h2 c2-kinetic">Three languages. One protocol.</h2>
              <p className="c2-standfirst">
                The TypeScript and Python SDKs are typed protocol clients generated from the Rust contracts. Importing a
                package never scans, launches a process or reads a credential.
              </p>
            </div>
            <aside className="c2-contents c2-set" aria-labelledby="c2-box-title">
              <h3 id="c2-box-title" className="c2-small-head">Contents of the toolkit</h3>
              <ol>
                {TOOLKIT.map(([name, text]) => (
                  <li key={name}><strong>{name}.</strong> {text}</li>
                ))}
              </ol>
            </aside>
          </div>
          <div className="c2-quick">
            {([
              ["Node", INSTALL[0].cmd, QUICKSTART.node, "hello.mjs"],
              ["Python", INSTALL[1].cmd, QUICKSTART.python, "hello.py"],
              ["Rust", INSTALL[2].cmd, QUICKSTART.rust, "src/main.rs"],
            ] as const).map(([lang, cmd, src, file]) => (
              <article className="c2-quick-col" key={lang} aria-labelledby={`c2-q-${lang}`}>
                <h3 id={`c2-q-${lang}`} className="c2-h3 c2-h3-small">{lang}</h3>
                <div className="c2-cmd">
                  <code><span aria-hidden="true">$ </span>{cmd}</code>
                  <Copy value={cmd} label={`${lang} install command`} />
                </div>
                <p className="c2-file">{file}</p>
                <pre tabIndex={0} aria-label={`${lang} quickstart`}><code>{highlight(src)}</code></pre>
              </article>
            ))}
          </div>
          <p className="c2-source-note">Set verbatim from <a href={DOCS.quickstarts}>docs/integration/quickstarts.md</a>.</p>
        </section>

        {/* ——— 03 The Index ——— */}
        <section className="c2-chapter c2-index" id="index" aria-labelledby="c2-index-title">
          <RunHead folio="03" title="The Index" right="Every client, by evidence" page="p. 13" />
          <div className="c2-grid">
            <span className="c2-folio-big" aria-hidden="true">03</span>
            <div className="c2-chapter-head">
              <p className="c2-eyebrow">Chapter three · The Index</p>
              <h2 id="c2-index-title" className="c2-h2 c2-kinetic">Index of the AI already on the machine.</h2>
              <p className="c2-standfirst">
                Passive discovery finds the clients a user already has, from metadata only: {DISCOVERED.join(", ")}, {DISCOVERED_TAIL}.
                Finding an installation is not a subscription, and never permission to run it.
              </p>
            </div>
            <aside className="c2-key c2-set" aria-labelledby="c2-key-title">
              <h3 id="c2-key-title" className="c2-small-head">How to read the numbers</h3>
              <ol>
                {OP_LABELS.map((op) => <li key={op.key}>{op.label}</li>)}
              </ol>
              <p><span className="c2-dagger">†</span> Implemented but conditional, fixture-tested only, or gated by a capability the provider may not report. Never an inferred yes.</p>
              <p><a href={DOCS.matrix}>The full support matrix</a></p>
            </aside>
          </div>

          <div className="c2-index-cols">
            {index.map(([letter, entries]) => (
              <div className="c2-index-group" key={letter}>
                <h3 className="c2-index-letter">{letter}</h3>
                <dl>
                  {entries.map((e) => (
                    <Fragment key={e.name}>
                      <dt>
                        <span className="c2-index-name">{e.name},</span> <span className="c2-index-kind">{e.kind}</span>
                        <span className="c2-leader" aria-hidden="true" />
                        <span className="c2-index-pages">
                          <span className="visually-hidden">operations: </span>
                          {e.marks.length === 0 ? "—" : e.marks.map((m, i) => (
                            <span key={m.text}>{i ? ", " : ""}{m.text}{m.partial && <span className="c2-dagger" title="partial">†</span>}</span>
                          ))}
                        </span>
                      </dt>
                      <dd>{e.sub}</dd>
                    </Fragment>
                  ))}
                </dl>
              </div>
            ))}
          </div>
        </section>

        {/* ——— Grants & safety ——— */}
        <section className="c2-chapter c2-articles" id="articles" aria-labelledby="c2-articles-title">
          <RunHead folio="§" title="Articles of Permission" right="Grants and safety" page="p. 17" />
          <div className="c2-grid">
            <span className="c2-folio-big" aria-hidden="true">§</span>
            <div className="c2-chapter-head">
              <p className="c2-eyebrow">The Articles</p>
              <h2 id="c2-articles-title" className="c2-h2 c2-kinetic">Discovery is not permission.</h2>
              <p className="c2-standfirst">
                Seeing that a client is installed lets an app read, not run. The host app grants each scope explicitly, and
                can take it back. <code>grants.request</code> only asks: it returns <code>requires_host_approval</code> and
                changes nothing.
              </p>
            </div>
          </div>
          <ol className="c2-article-list">
            {SAFETY.map(([title, text], i) => (
              <li key={title} className="c2-article">
                <p className="c2-article-n">Article {ROMAN[i]}</p>
                <h3 className="c2-h3">{title}.</h3>
                <p>{text}</p>
              </li>
            ))}
          </ol>
          <div className="c2-schedule">
            <div>
              <h3 className="c2-small-head" id="c2-sched-title">Schedule A · The five scopes</h3>
              <table className="c2-table" aria-labelledby="c2-sched-title">
                <thead><tr><th scope="col">Scope</th><th scope="col">Field</th><th scope="col">Grants</th></tr></thead>
                <tbody>
                  {GRANT_SCOPES.map((s) => (
                    <tr key={s.id}><th scope="row">{s.label}</th><td><code>{s.field}</code></td><td>{s.note}</td></tr>
                  ))}
                </tbody>
              </table>
              <p className="c2-table-note">Each scope is separate, revocable and per application.</p>
            </div>
            <figure className="c2-manifest">
              <pre tabIndex={0} aria-label="grants.json example"><code>{highlight(manifest)}</code></pre>
              <figcaption className="c2-figcap">
                <span>Fig. 2</span> A shared-broker grants manifest: metadata and execution for one example app, on one
                provider, in one workspace. Paid stays off. No protocol request can widen its own grant.{" "}
                <a href={DOCS.broker}>Shared broker guide</a>
              </figcaption>
            </figure>
          </div>
        </section>

        {/* ——— Appendix: the control center ——— */}
        <section className="c2-appendix" id="appendix" aria-labelledby="c2-appendix-title">
          <div className="c2-appendix-inner">
            <p className="c2-eyebrow">Appendix A</p>
            <h2 id="c2-appendix-title" className="c2-appendix-title">The control center</h2>
            <div className="c2-appendix-cols">
              <p>
                {CONTROL_CENTER.line} {CONTROL_CENTER.note} History and grants stay on your machine. If you want charts of
                what your apps used, they are here, at the back, where an appendix belongs.
              </p>
              <ol className="c2-appendix-toc">
                {CONTROL_CENTER.views.map((v, i) => (
                  <li key={v}><span>{v}</span><span className="c2-leader" aria-hidden="true" /><span>A.{i + 1}</span></li>
                ))}
              </ol>
              <div className="c2-cmd c2-cmd-small">
                <code><span aria-hidden="true">$ </span>{CONTROL_CENTER.run}</code>
                <Copy value={CONTROL_CENTER.run} label="desktop app command" />
              </div>
            </div>
          </div>
        </section>

        {/* ——— Install / CTA ——— */}
        <section className="c2-cta" id="install" aria-labelledby="c2-cta-title">
          <p className="c2-eyebrow">To set it yourself</p>
          <h2 id="c2-cta-title" className="c2-cta-title">Put the models you already pay for inside software you&nbsp;write.</h2>
          <ul className="c2-install">
            {INSTALL.map((i) => (
              <li key={i.id} className="c2-cmd c2-cmd-large">
                <span className="c2-cmd-label">{i.label}</span>
                <code><span aria-hidden="true">$ </span>{i.cmd}</code>
                <Copy value={i.cmd} label={`${i.label} install command`} />
              </li>
            ))}
          </ul>
          <p className="c2-cta-status">{STATUS_LONG}</p>
          <p className="c2-cta-links">
            <a href={DOCS.quickstarts} className="c2-btn c2-btn-ink">Read the quickstarts</a>
            <a href={REPO} className="c2-btn">Source on GitHub</a>
          </p>
        </section>
      </main>

      <footer className="c2-colophon">
        <div className="c2-colophon-grid">
          <div>
            <p className="c2-colophon-mark">OriginLM</p>
            <p>{TAGLINE} {STATUS}.</p>
          </div>
          <div>
            <h2 className="c2-small-head">Colophon</h2>
            <p>Set in Fraunces, Red Hat Display and Martian Mono on warm paper. No telemetry, no hosted relay. Model ids and replies shown on this page are examples.</p>
          </div>
          <nav aria-label="Further reading">
            <h2 className="c2-small-head">Further reading</h2>
            <ul>
              <li><a href={DOCS.threat}>Threat model</a></li>
              <li><a href={DOCS.privacy}>Privacy boundary</a></li>
              <li><a href={DOCS.security}>Security policy</a></li>
              <li><a href={DOCS.license}>MIT license</a></li>
              <li><a href="/concepts">All five concepts</a></li>
            </ul>
          </nav>
        </div>
        <p className="c2-colophon-end" aria-hidden="true">— end —</p>
      </footer>
    </div>
  );
}
