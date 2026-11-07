import "./index.css";
import { usePageChrome } from "../shared/motion";
import { PITCH } from "../shared/facts";

type Concept = { id: string; n: string; name: string; pitch: string; idea: string; palette: string[] };

export const CONCEPTS: Concept[] = [
  { id: "c1", n: "01", name: "The Switchboard", pitch: "A patch bay for your AI. Drag a cable from your app to a model and watch the call it makes.", idea: "Dark, playable patch bay", palette: ["#0e0f0e", "#3dff8b", "#ffb43a", "#e9ebe6"] },
  { id: "c2", n: "02", name: "One Runtime", pitch: "A printed manifesto against dashboard-ification. Your AI is not a chart.", idea: "Editorial manifesto, light", palette: ["#f4efe6", "#141210", "#d6261b", "#6b645b"] },
  { id: "c3", n: "03", name: "The Origin Stone", pitch: "One dark monolith, the local runtime, that every route, grant and session orbits.", idea: "One WebGL object, whole page", palette: ["#050506", "#c9d4e3", "#7fa8ff", "#8a8f99"] },
  { id: "c4", n: "04", name: "The Desktop", pitch: "The page is a working desktop running OriginLM. Pick a model, flip the grant, press Run.", idea: "The page is a working OS", palette: ["#151a21", "#5cf08f", "#e8edf3", "#8fa3b8"] },
  { id: "c5", n: "05", name: "The Wire", pitch: "A live wire service for your own AI. Every app you build subscribes to it.", idea: "Tactile brutalist broadsheet", palette: ["#ffffff", "#0a0a0a", "#ff4f00", "#585858"] },
];

const SHOTS = import.meta.glob("../../../concept-shots/*-1440-hero.png", { eager: true, query: "?url", import: "default" }) as Record<string, string>;
const shotFor = (id: string) => Object.entries(SHOTS).find(([path]) => path.endsWith(`/${id}-1440-hero.png`))?.[1];

export default function ConceptsIndex() {
  usePageChrome("OriginLM: five landing concepts", "cx-index-body", "#f2f1ee");
  return (
    <div className="cx-index">
      <a className="cx-skip" href="#main">Skip to content</a>
      <header className="cx-head">
        <p className="cx-kicker">OriginLM · design exploration · not the published site</p>
        <h1>Five landing directions.</h1>
        <p className="cx-lead">
          Each concept leads with one idea: <strong>{PITCH.headline}</strong> Monitoring comes late and small in
          every one. The published page at <a href="/">/</a> is unchanged.
        </p>
      </header>
      <main id="main">
        <ol className="cx-grid">
          {CONCEPTS.map((c) => {
            const src = shotFor(c.id);
            return (
              <li key={c.id} className="cx-card">
                <a href={`/${c.id}`} className="cx-link" aria-labelledby={`${c.id}-name`} aria-describedby={`${c.id}-pitch`}>
                  <span className="cx-shot">
                    {src
                      ? <img src={src} alt="" width={1440} height={900} loading="lazy" />
                      : <span className="cx-shot-empty" aria-hidden="true">/{c.id}</span>}
                  </span>
                  <span className="cx-meta">
                    <span className="cx-n" aria-hidden="true">{c.n}</span>
                    <span className="cx-text">
                      <span className="cx-name" id={`${c.id}-name`}>{c.name}</span>
                      <span className="cx-idea">{c.idea} · /{c.id}</span>
                      <span className="cx-pitch" id={`${c.id}-pitch`}>{c.pitch}</span>
                    </span>
                    <span className="cx-swatches" aria-hidden="true">
                      {c.palette.map((color) => <span key={color} style={{ background: color }} />)}
                    </span>
                  </span>
                </a>
              </li>
            );
          })}
        </ol>
      </main>
      <footer className="cx-foot">
        <p>Copy is limited to claims in the README, the integration docs and the site's data files. Example data is labelled where it appears.</p>
      </footer>
    </div>
  );
}
