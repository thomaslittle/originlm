import { C, Dot, F, MONO, Mark, OUT, ramp, between } from "./kit";

const ROWS = [
  { name: "Codex", ev: "~/.codex · app-server", state: "found" },
  { name: "Claude Code", ev: "~/.claude · native profile", state: "found" },
  { name: "Gemini CLI", ev: "PATH · ACP profile", state: "found" },
  { name: "Grok", ev: "~/.grok · ACP profile", state: "found" },
  { name: "OpenCode", ev: "PATH · serve", state: "found" },
  { name: "Cursor", ev: "cursor-agent · cached plan", state: "found" },
  { name: "GitHub Copilot", ev: "~/.copilot", state: "found" },
  { name: "Ollama", ev: "127.0.0.1:11434", state: "endpoint" },
  { name: "LM Studio", ev: "127.0.0.1:1234", state: "endpoint" },
];

function Chip({ state }: { state: string }) {
  const endpoint = state === "endpoint";
  const color = endpoint ? C.amber : C.success;
  return (
    <div
      style={{
        padding: "6px 11px 5px 11px",
        borderRadius: 999,
        border: "1px solid " + color,
        backgroundColor: endpoint ? "rgba(235,163,26,0.08)" : "rgba(124,192,143,0.08)",
        fontFamily: MONO,
        fontSize: 11,
        letterSpacing: "0.1em",
        textTransform: "uppercase",
        color,
        lineHeight: 1,
      }}
    >
      {state}
    </div>
  );
}

export default function SceneInventory({ f }: { f: number }) {
  if (!between(f, 126, 318)) return null;
  const enter = ramp(f, 136, 156);
  const exit = ramp(f, 292, 302);
  const leftIn = ramp(f, 148, 166);
  const scanP = ramp(f, 144, 238, OUT);

  return (
    <div style={{ position: "absolute", inset: 0, opacity: 1 }}>
      {/* statement */}
      <div
        style={{
          position: "absolute",
          left: 110 - exit * 50,
          top: 184,
          width: 740,
          opacity: leftIn * (1 - exit),
          transform: `translateY(${(1 - leftIn) * 22}px)`,
        }}
      >
        <div style={{ fontFamily: F, fontSize: 92, fontWeight: 800, lineHeight: 0.96, letterSpacing: "-0.035em", color: C.text }}>
          One local<br />control center.
        </div>
        <div style={{ marginTop: 38, fontFamily: F, fontSize: 33, fontWeight: 500, lineHeight: 1.38, color: C.muted, width: 600 }}>
          Finds what is installed.<br />Keeps the evidence.
        </div>
        <div style={{ marginTop: 26, fontFamily: F, fontSize: 27, fontWeight: 500, color: C.amber }}>Unknowns are never filled in.</div>
        <div
          style={{
            marginTop: 54,
            display: "inline-flex",
            alignItems: "center",
            gap: 12,
            padding: "14px 18px",
            border: "1px solid " + C.border,
            borderRadius: 10,
            backgroundColor: C.surface,
          }}
        >
          <div style={{ fontFamily: MONO, fontSize: 18, color: C.text }}>originlm scan --json</div>
          <div style={{ width: 7, height: 7, borderRadius: 4, backgroundColor: C.amber }} />
        </div>

        <div style={{ marginTop: 44, width: 540 }}>
          <div style={{ fontFamily: MONO, fontSize: 10.5, letterSpacing: "0.18em", color: C.faint }}>EVIDENCE, PER FIELD</div>
          <div style={{ marginTop: 12, fontFamily: MONO, fontSize: 15.5, lineHeight: 1.7, color: C.muted }}>
            provider · installation · profile<br />
            account · connection · route · model
          </div>
        </div>
      </div>

      {/* inventory panel */}
      <div
        style={{
          position: "absolute",
          left: 920 + (1 - enter) * 70 - exit * 90,
          top: 120,
          width: 900,
          height: 840,
          boxSizing: "border-box",
          backgroundColor: C.surface,
          border: "1px solid " + C.border,
          borderRadius: 22,
          boxShadow: "0 40px 100px rgba(0,0,0,0.42)",
          opacity: enter * (1 - exit),
          overflow: "hidden",
        }}
      >
        <div style={{ height: 84, display: "flex", alignItems: "center", padding: "0 28px", borderBottom: "1px solid " + C.border }}>
          <Mark width={26} />
          <div style={{ marginLeft: 12, fontFamily: F, fontSize: 13, fontWeight: 700, letterSpacing: "0.2em", color: C.muted }}>LOCAL INVENTORY</div>
          <div style={{ flex: 1 }} />
          <Dot color={C.success} size={7} />
          <div style={{ marginLeft: 9, fontFamily: MONO, fontSize: 12, color: C.muted }}>9 found</div>
        </div>

        <div style={{ position: "relative", height: 702 }}>
          {ROWS.map((r, i) => {
            const at = 142 + i * 5;
            const p = ramp(f, at, at + 12);
            return (
              <div
                key={r.name}
                style={{
                  height: 78,
                  display: "flex",
                  alignItems: "center",
                  padding: "0 28px",
                  borderBottom: i < ROWS.length - 1 ? "1px solid " + C.border : "none",
                  opacity: p,
                  transform: `translateX(${(1 - p) * 26}px)`,
                }}
              >
                <div style={{ width: 46, fontFamily: MONO, fontSize: 12, color: C.faint }}>{String(i + 1).padStart(2, "0")}</div>
                <div style={{ width: 292, fontFamily: F, fontSize: 28, fontWeight: 700, letterSpacing: "-0.01em", color: C.text }}>{r.name}</div>
                <div style={{ flex: 1, fontFamily: MONO, fontSize: 14, color: C.muted, whiteSpace: "nowrap", overflow: "hidden" }}>{r.ev}</div>
                <Chip state={r.state} />
              </div>
            );
          })}

          {/* scan line */}
          <div
            style={{
              position: "absolute",
              left: 0,
              right: 0,
              top: 78 + scanP * 702,
              height: 2,
              backgroundColor: C.amber,
              opacity: f > 144 && f < 242 ? 0.65 * (1 - ramp(f, 230, 242)) : 0,
              boxShadow: "0 0 18px rgba(235,163,26,0.4)",
            }}
          />
        </div>

        <div style={{ height: 54, display: "flex", alignItems: "center", padding: "0 28px", borderTop: "1px solid " + C.border, backgroundColor: "#211d1e" }}>
          <div style={{ fontFamily: MONO, fontSize: 11.5, letterSpacing: "0.06em", color: C.faint, whiteSpace: "nowrap" }}>
            PASSIVE DISCOVERY · METADATA ONLY · NO SECRETS READ · NO NETWORK CALLS
          </div>
        </div>
      </div>
    </div>
  );
}
