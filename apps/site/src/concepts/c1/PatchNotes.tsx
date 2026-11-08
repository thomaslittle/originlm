/* "Patch notes": grants as literal patch permissions. Each row is one example app's
   grant, cabled to one provider. Yank the cable to revoke it; the manifest beside it is
   the shared-broker grants file entry for the selected row. */
import { useRef, useState } from "react";
import { gsap, prefersReducedMotion } from "../shared/motion";
import { GRANT_SCOPES, grantManifest } from "../shared/facts";
import { highlight } from "../../lib/highlight";

type Scope = (typeof GRANT_SCOPES)[number]["id"];
type Row = { app: string; provider: string; providerId: string; scopes: Scope[] };

const ROWS: Row[] = [
  { app: "notes-app", provider: "Claude Code", providerId: "claude_code", scopes: ["metadata", "execution", "workspace"] },
  { app: "photo-sorter", provider: "Ollama", providerId: "ollama", scopes: ["metadata", "execution", "workspace"] },
  { app: "budget-cli", provider: "Codex", providerId: "codex", scopes: ["metadata"] },
];

const LIVE = "M2,14 Q50,40 98,14";
const YANKED = "M2,14 Q14,60 34,58";

export function PatchNotes() {
  const [revoked, setRevoked] = useState<Record<string, boolean>>({});
  const [selected, setSelected] = useState(ROWS[0].app);
  const paths = useRef<Record<string, SVGPathElement | null>>({});
  const reduced = prefersReducedMotion();

  const yank = (app: string) => {
    const path = paths.current[app];
    const next = !revoked[app];
    setRevoked((prev) => ({ ...prev, [app]: next }));
    setSelected(app);
    if (!path || reduced) return;
    gsap.fromTo(path, { attr: { d: next ? LIVE : YANKED } }, { attr: { d: next ? YANKED : LIVE }, duration: next ? 0.7 : 0.45, ease: next ? "elastic.out(1, 0.45)" : "back.out(1.8)" });
  };

  const row = ROWS.find((r) => r.app === selected)!;
  const scopes = Object.fromEntries(GRANT_SCOPES.map((s) => [s.id, !revoked[row.app] && row.scopes.includes(s.id)]));
  const manifest = revoked[row.app]
    ? `// ${row.app}: revoked with apps.revoke\n// (administrator credential, control center only).\n// The broker grant is deleted and every live\n// connection with this app identity is swept,\n// so the old token cannot reconnect.`
    : grantManifest({ app: row.app, providerId: row.providerId, scopes });

  return (
    <div className="c1-notes">
      <ul className="c1-notes-rows" aria-label="Example grants on this board">
        {ROWS.map((r) => {
          const off = !!revoked[r.app];
          return (
            <li key={r.app} className={`c1-note${off ? " is-off" : ""}${selected === r.app ? " is-selected" : ""}`}>
              <button type="button" className="c1-note-select" onClick={() => setSelected(r.app)} aria-pressed={selected === r.app}>
                <span className="c1-note-app">{r.app}</span>
                <span className="c1-note-run" aria-hidden="true">
                  <span className="c1-note-jack c1-note-jack--near" />
                  <span className="c1-note-jack c1-note-jack--far" />
                  <svg className="c1-note-cable" viewBox="0 0 100 52" preserveAspectRatio="none">
                    <path ref={(el) => { paths.current[r.app] = el; }} d={off ? YANKED : LIVE} className="c1-note-wire" vectorEffect="non-scaling-stroke" />
                  </svg>
                </span>
                <span className="c1-note-provider">{r.provider}</span>
              </button>
              <ul className="c1-scopes" aria-label={`${r.app} scopes`}>
                {GRANT_SCOPES.map((s) => {
                  const on = !off && r.scopes.includes(s.id);
                  return <li key={s.id} className={on ? "is-on" : ""}>{s.label}<span className="visually-hidden">{on ? ": granted" : ": not granted"}</span></li>;
                })}
              </ul>
              <button type="button" className={`c1-yank${off ? " is-off" : ""}`} onClick={() => yank(r.app)}>
                {off ? "Re-patch" : "Yank cable"}<span className="visually-hidden"> for {r.app}</span>
              </button>
              <p className="c1-note-status" aria-live="polite">{off ? "revoked · apps.revoke · sessions ended" : r.scopes.includes("execution") ? "may run turns in its workspace" : "may read, not run"}</p>
            </li>
          );
        })}
      </ul>

      <figure className="c1-manifest">
        <figcaption className="c1-console-head">
          <span className="c1-silk">grants.json · {row.app}</span>
          <span className="c1-chip">owner-only file · example</span>
        </figcaption>
        <pre tabIndex={0} aria-label={`Grant manifest for ${row.app}`}><code>{highlight(manifest)}</code></pre>
      </figure>
    </div>
  );
}
