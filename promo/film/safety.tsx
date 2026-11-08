import { C, Cloud, Dot, F, Key, MONO, Mark, PlayTile, between, ramp } from "./kit";

const BEATS = [
  { at: 484, text: "Local control plane." },
  { at: 516, text: "Discovery is not permission." },
  { at: 548, text: "Grants are explicit. Revocable." },
  { at: 580, text: "Credentials never reach your app." },
  { at: 612, text: "No telemetry. No hosted relay." },
];

function Bx({ name, x, y, w = 170, found, active }: { name: string; x: number; y: number; w?: number; found: number; active?: boolean }) {
  return (
    <div
      style={{
        position: "absolute",
        left: x,
        top: y,
        width: w,
        height: 46,
        boxSizing: "border-box",
        borderRadius: 10,
        border: "1px solid " + (active ? C.amber : "#4a4042"),
        backgroundColor: active ? "rgba(235,163,26,0.06)" : C.surface,
        display: "flex",
        alignItems: "center",
        padding: "0 13px",
      }}
    >
      <div style={{ fontFamily: F, fontSize: 15, fontWeight: 700, color: C.text, whiteSpace: "nowrap" }}>{name}</div>
      <div style={{ flex: 1 }} />
      <div style={{ opacity: found }}>
        <Dot color={C.success} size={5} />
      </div>
    </div>
  );
}

export default function SceneSafety({ f }: { f: number }) {
  if (!between(f, 472, 662)) return null;
  const inP = ramp(f, 478, 500);
  const out = ramp(f, 648, 662);
  const found = ramp(f, 518, 530);
  const executeIn = ramp(f, 510, 522);
  const grantIn = ramp(f, 552, 564);
  const keyIn = ramp(f, 582, 594);
  const cloudIn = ramp(f, 486, 502);
  const relayIn = ramp(f, 612, 622);
  const dim = 1 - 0.9 * ramp(f, 614, 628);
  const snap = ramp(f, 620, 626);

  return (
    <div style={{ position: "absolute", inset: 0, opacity: inP * (1 - out) }}>
      {/* statement stack */}
      <div style={{ position: "absolute", left: 150, top: 206 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          {BEATS.map((b, i) => {
            const next = i < BEATS.length - 1 ? BEATS[i + 1].at : 999;
            const on = ramp(f, b.at, b.at + 6) * (1 - ramp(f, next, next + 6));
            return (
              <div key={i} style={{ width: 34, height: 2, backgroundColor: on > 0.5 ? C.amber : C.border, opacity: on > 0.5 ? 1 : 0.55 }} />
            );
          })}
          <div style={{ marginLeft: 8, fontFamily: MONO, fontSize: 11, color: C.faint, letterSpacing: "0.12em" }}>
            0{Math.min(5, Math.max(1, BEATS.filter((b) => f >= b.at).length))} / 05
          </div>
        </div>

        <div style={{ position: "relative", marginTop: 46, width: 620, height: 250 }}>
          {BEATS.map((b, i) => {
            const next = i < BEATS.length - 1 ? BEATS[i + 1].at : 999;
            const p = ramp(f, b.at, b.at + 6);
            const q = ramp(f, next - 4, next);
            const opacity = p * (1 - q);
            return (
              <div
                key={i}
                style={{
                  position: "absolute",
                  left: 0,
                  top: 0,
                  width: 620,
                  opacity,
                  transform: `translateY(${(1 - p) * 20 - q * 26}px)`,
                  fontFamily: F,
                  fontSize: 50,
                  fontWeight: 700,
                  lineHeight: 1.06,
                  letterSpacing: "-0.02em",
                  color: i === 4 ? C.amber : C.text,
                }}
              >
                {b.text}
              </div>
            );
          })}
        </div>

      </div>

      {/* the diagram */}
      <div style={{ position: "absolute", left: 780, top: 150, width: 1120, height: 780 }}>
        {/* static wires */}
        <svg width={1120} height={780} style={{ position: "absolute", left: 0, top: 0 }}>
          {[
            [240, 123, 320, 300],
            [240, 283, 320, 300],
            [240, 443, 320, 300],
            [550, 123, 500, 300],
            [550, 283, 500, 300],
          ].map((l, i) => (
            <g key={i} opacity={0.8 * dim}>
              <line x1={l[0]} y1={l[1]} x2={l[2]} y2={l[3]} stroke={C.border} strokeWidth={1.5} />
              <circle cx={l[0]} cy={l[1]} r={2.5} fill={C.faint} />
            </g>
          ))}
          {/* app to core, the grant path */}
          <line x1={380} y1={500} x2={380} y2={390} stroke={f > 552 ? C.amber : C.border} strokeWidth={2} opacity={0.9 * dim} />
          {f > 552 ? (
            <line x1={380} y1={500} x2={380} y2={390} stroke={C.amberSoft} strokeWidth={4} strokeDasharray="3 11" strokeDashoffset={-f * 1.6} opacity={0.45 * dim} />
          ) : null}

          {/* boundary */}
          <rect x={30} y={40} width={760} height={590} rx={24} fill="none" stroke={C.border} strokeWidth={2} opacity={0.95 * dim} />

          {/* core ring */}
          <circle cx={410} cy={300} r={126} fill="none" stroke={C.amber} strokeWidth={1.5} strokeDasharray="3 13" strokeDashoffset={f * 0.9} opacity={0.45 * dim} />
        </svg>

        <div style={{ position: "absolute", left: 55, top: 56, opacity: 0.9 * dim }}>
          <div style={{ fontFamily: MONO, fontSize: 10.5, letterSpacing: "0.18em", color: C.faint }}>LOCAL CONTROL PLANE</div>
        </div>

        {/* core */}
        <div
          style={{
            position: "absolute",
            left: 320,
            top: 210,
            width: 180,
            height: 180,
            borderRadius: 40,
            backgroundColor: C.surface,
            border: "1px solid " + C.border,
            boxShadow: "0 30px 70px rgba(0,0,0,0.35)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            opacity: dim,
          }}
        >
          <Mark width={118} />
        </div>

        <div style={{ opacity: dim }}>
          <Bx name="Codex" x={70} y={100} found={found} />
          <Bx name="OpenCode" x={70} y={260} found={found} />
          <Bx name="Ollama" x={70} y={420} found={found} />
          <Bx name="Claude Code" x={550} y={100} found={found} />
          <Bx name="LM Studio" x={550} y={260} found={found} />
        </div>

        {/* app */}
        <div
          style={{
            position: "absolute",
            left: 270,
            top: 500,
            width: 220,
            height: 52,
            boxSizing: "border-box",
            borderRadius: 12,
            border: "1px solid " + (grantIn > 0.2 ? C.amber : C.border),
            backgroundColor: C.raised,
            display: "flex",
            alignItems: "center",
            padding: "0 14px",
            opacity: dim,
          }}
        >
          <div style={{ width: 16, height: 16, borderRadius: 4, border: "1px solid " + C.muted }} />
          <div style={{ marginLeft: 10, fontFamily: F, fontSize: 12, fontWeight: 700, letterSpacing: "0.14em", color: C.text }}>YOUR APP</div>
        </div>

        {/* grant token */}
        {f >= 552 ? (
          <div
            style={{
              position: "absolute",
              left: 300,
              top: 422,
              width: 172,
              height: 38,
              boxSizing: "border-box",
              borderRadius: 8,
              border: "1px solid " + (grantIn > 0.2 ? C.amber : C.border),
              backgroundColor: C.surface,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              opacity: grantIn * dim,
              transform: `translateY(${(1 - grantIn) * 10}px)`,
            }}
          >
            <div style={{ fontFamily: MONO, fontSize: 11.5, color: grantIn > 0.2 ? C.amber : C.muted, letterSpacing: "0.02em" }}>
              {grantIn > 0.5 ? "granted · revocable" : "grant"}
            </div>
          </div>
        ) : null}

        {/* execute, outside */}
        <div style={{ position: "absolute", left: 852, top: 300, opacity: executeIn * dim }}>
          <PlayTile x={0} y={0} size={68} opacity={1} slash={ramp(f, 516, 528)} />
          <div style={{ position: "absolute", left: 0, top: 78, fontFamily: MONO, fontSize: 10.5, letterSpacing: "0.14em", whiteSpace: "nowrap", color: f > 540 ? C.amber : C.faint }}>EXECUTE</div>
        </div>

        {/* key, inside */}
        {f >= 582 ? (
          <div style={{ opacity: keyIn * dim }}>
            <Key x={590} y={470} size={40} opacity={1} />
            <div style={{ position: "absolute", left: 590, top: 432, width: 220, fontFamily: MONO, fontSize: 10.5, lineHeight: 1.4, color: C.muted }}>
              client owns credentials
            </div>
            <svg width={1120} height={780} style={{ position: "absolute", left: 0, top: 0 }}>
              <line x1={590} y1={500} x2={490} y2={500} stroke={C.faint} strokeWidth={1.5} strokeDasharray="4 5" />
              <line x1={536} y1={482} x2={536} y2={518} stroke={C.amber} strokeWidth={3.5} />
            </svg>
          </div>
        ) : null}

        {/* cloud / relay, outside */}
        <div style={{ position: "absolute", left: 800, top: 34, opacity: cloudIn }}>
          <Cloud x={0} y={0} size={190} opacity={0.55 + 0.45 * ramp(f, 612, 622)} slash={snap} />
          <div style={{ position: "absolute", left: 8, top: 124, width: 210, fontFamily: MONO, fontSize: 10.5, letterSpacing: "0.12em", color: f > 612 ? C.amber : C.faint }}>
            {f > 612 ? "NO HOSTED RELAY" : "HOSTED RELAY"}
          </div>
        </div>
        {relayIn > 0 ? (
          <svg width={1120} height={780} style={{ position: "absolute", left: 0, top: 0 }}>
            <line x1={790} y1={118} x2={790 + 82 * relayIn} y2={118} stroke={C.faint} strokeWidth={1.5} strokeDasharray="4 6" opacity={1 - snap * 0.55} />
            <g opacity={snap}>
              <line x1={818} y1={108} x2={838} y2={128} stroke={C.amber} strokeWidth={2.5} strokeLinecap="round" />
              <line x1={838} y1={108} x2={818} y2={128} stroke={C.amber} strokeWidth={2.5} strokeLinecap="round" />
            </g>
          </svg>
        ) : null}
      </div>
    </div>
  );
}
