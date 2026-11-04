import { useEffect, useState } from "react";

type Window = { label: string; used: number; resetsInSec: number };
type Row =
  | { kind: "quota"; name: string; plan: string; windows: Window[] }
  | { kind: "unknown"; name: string; plan: string; reason: string }
  | { kind: "local"; name: string; plan: string; detail: string };

/* Labeled example data. Real readings come from each client; values a client does not
   report are shown as unknown, never as zero. */
const ROWS: Row[] = [
  {
    kind: "quota", name: "Claude Code", plan: "Personal",
    windows: [
      { label: "5-hour", used: 62, resetsInSec: 3 * 3600 + 12 * 60 },
      { label: "7-day", used: 38, resetsInSec: 4 * 86400 + 2 * 3600 },
    ],
  },
  {
    kind: "quota", name: "Codex", plan: "Work",
    windows: [
      { label: "5-hour", used: 84, resetsInSec: 41 * 60 },
      { label: "weekly", used: 51, resetsInSec: 2 * 86400 + 9 * 3600 },
    ],
  },
  {
    kind: "quota", name: "Cursor", plan: "Personal",
    windows: [{ label: "monthly", used: 41, resetsInSec: 12 * 86400 }],
  },
  { kind: "unknown", name: "Gemini CLI", plan: "signed in", reason: "usage not reported by the client" },
  { kind: "local", name: "Ollama", plan: "local", detail: "7 models, no quota" },
];

const CELLS = 20;

function formatReset(total: number) {
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  return `${m}m ${String(s).padStart(2, "0")}s`;
}

function Meter({ used }: { used: number }) {
  const filled = Math.round((used / 100) * CELLS);
  return (
    <span className="meter" data-hot={used >= 80 || undefined} role="img" aria-label={`${used}% used`}>
      {Array.from({ length: CELLS }, (_, i) => <i key={i} data-on={i < filled || undefined} />)}
    </span>
  );
}

export function UsagePanel() {
  // Counts down only after hydration, so the server and first client render match.
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const id = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(id);
  }, []);

  return (
    <figure className="usage-panel">
      <figcaption className="usage-head">
        <span><b>originlm</b> usage</span>
        <span className="usage-tag">example data</span>
      </figcaption>

      <ul className="usage-rows">
        {ROWS.map((row) => (
          <li key={row.name} className="usage-row">
            <div className="usage-who">
              <strong>{row.name}</strong>
              <span>{row.plan}</span>
            </div>

            {row.kind === "quota" && (
              <ul className="usage-windows">
                {row.windows.map((w) => (
                  <li key={w.label}>
                    <span className="w-label">{w.label}</span>
                    <Meter used={w.used} />
                    <span className="w-pct num">{w.used}%</span>
                    <span className="w-reset num">resets {formatReset(Math.max(0, w.resetsInSec - elapsed))}</span>
                  </li>
                ))}
              </ul>
            )}
            {row.kind === "unknown" && <p className="usage-note"><b>unknown</b> {row.reason}</p>}
            {row.kind === "local" && <p className="usage-note">{row.detail}</p>}
          </li>
        ))}
      </ul>

      <p className="usage-foot">Each reading carries its source and freshness. Missing means unknown, not zero.</p>
    </figure>
  );
}
