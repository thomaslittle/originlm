import { C, F, MONO, OUT, Tile, ramp, between } from "./kit";

const CLIENTS = [
  { name: "Codex", path: "~/.codex", x: 930, y: 160, rot: -3.6 },
  { name: "Claude Code", path: "~/.claude", x: 1240, y: 128, rot: 2.6 },
  { name: "Gemini CLI", path: "PATH · gemini", x: 1532, y: 190, rot: -2.1 },
  { name: "Grok", path: "~/.grok", x: 898, y: 378, rot: 3.4 },
  { name: "OpenCode", path: "~/.opencode", x: 1210, y: 360, rot: -2.4 },
  { name: "Cursor", path: "cursor-agent", x: 1520, y: 420, rot: 4.1 },
  { name: "GitHub Copilot", path: "~/.copilot", x: 958, y: 618, rot: -3.1 },
  { name: "Ollama", path: "127.0.0.1:11434", x: 1288, y: 650, rot: 2.2 },
  { name: "LM Studio", path: "127.0.0.1:1234", x: 1578, y: 700, rot: -3.8 },
];

type Strand = { p0: [number, number]; c: [number, number]; p1: [number, number]; label: string; lx: number; ly: number };

const STRANDS: Strand[] = [
  { p0: [1073, 230], c: [1200, 134], p1: [1383, 198], label: "metadata", lx: 1118, ly: 136 },
  { p0: [1383, 198], c: [1510, 132], p1: [1675, 260], label: "catalog", lx: 1568, ly: 138 },
  { p0: [1043, 448], c: [1150, 382], p1: [1353, 430], label: "auth status", lx: 946, ly: 318 },
  { p0: [1353, 430], c: [1470, 518], p1: [1663, 490], label: "", lx: 0, ly: 0 },
  { p0: [1103, 688], c: [1230, 638], p1: [1433, 720], label: "", lx: 0, ly: 0 },
  { p0: [1433, 720], c: [1540, 786], p1: [1723, 770], label: "", lx: 0, ly: 0 },
  { p0: [1073, 230], c: [1120, 356], p1: [1353, 430], label: "", lx: 0, ly: 0 },
  { p0: [1383, 198], c: [1420, 330], p1: [1663, 490], label: "", lx: 0, ly: 0 },
  { p0: [1043, 448], c: [1028, 580], p1: [1103, 688], label: "", lx: 0, ly: 0 },
];

const GLUE = ["paths.ts", "metadata.ts", "stream.ts", "retry.ts"];

function q(p0: number[], c: number[], p1: number[], t: number): [number, number] {
  const u = 1 - t;
  return [u * u * p0[0] + 2 * u * t * c[0] + t * t * p1[0], u * u * p0[1] + 2 * u * t * c[1] + t * t * p1[1]];
}

export default function ScenePile({ f }: { f: number }) {
  if (!between(f, 0, 158)) return null;
  const vis = 1 - ramp(f, 146, 160);
  const exit = ramp(f, 134, 150);
  const textOut = ramp(f, 134, 148);

  return (
    <div style={{ position: "absolute", inset: 0, opacity: vis }}>
      {/* the strands and their traffic */}
      <svg width={1920} height={1080} style={{ position: "absolute", left: 0, top: 0, opacity: 1 - exit }}>
        {STRANDS.map((s, i) => {
          const draw = ramp(f, 18 + i * 3, 52 + i * 3);
          const path = `M ${s.p0[0]} ${s.p0[1]} Q ${s.c[0]} ${s.c[1]} ${s.p1[0]} ${s.p1[1]}`;
          return (
            <g key={i} opacity={draw * 0.95}>
              <path d={path} fill="none" stroke={i < 6 ? "rgba(176,168,161,0.55)" : "rgba(176,168,161,0.28)"} strokeWidth={1.5} strokeDasharray="5 8" strokeDashoffset={-f * 1.1 - i * 7} />
              {i < 6
                ? [0, 1].map((k) => {
                    const t = (f * 0.0045 + i * 0.31 + k * 0.5) % 1;
                    const [px, py] = q(s.p0, s.c, s.p1, t);
                    return <circle key={k} cx={px} cy={py} r={2.8} fill={C.amber} opacity={0.95 - 0.5 * exit} />;
                  })
                : null}
            </g>
          );
        })}
      </svg>

      {/* glue labels */}
      {STRANDS.filter((s) => s.label).map((s, i) => {
        const p = ramp(f, 30 + i * 4, 44 + i * 4);
        return (
          <div
            key={s.label}
            style={{
              position: "absolute",
              left: s.lx,
              top: s.ly,
              padding: "5px 9px 4px 9px",
              backgroundColor: "rgba(30,26,27,0.94)",
              border: "1px solid " + C.border,
              borderRadius: 7,
              opacity: p * (1 - exit),
              transform: "translateY(" + (10 * (1 - p)) + "px)",
            }}
          >
            <div style={{ fontFamily: MONO, fontSize: 12, color: C.muted, letterSpacing: "0.02em", whiteSpace: "nowrap" }}>{s.label}</div>
          </div>
        );
      })}

      {/* leader lines from the glue labels */}
      <svg width={1920} height={1080} style={{ position: "absolute", left: 0, top: 0, opacity: (1 - exit) * ramp(f, 38, 56) }}>
        <g stroke="rgba(176,168,161,0.4)" strokeWidth={1}>
          <line x1={1155} y1={160} x2={1190} y2={180} />
          <line x1={1600} y1={162} x2={1602} y2={190} />
          <line x1={994} y1={342} x2={1064} y2={392} />
        </g>
      </svg>

      {/* the pile */}
      {CLIENTS.map((t, i) => {
        const inAt = 8 + i * 4.2;
        const p = ramp(f, inAt, inAt + 16);
        const float = Math.sin((f + i * 17) / 30) * 4;
        const push = ramp(f, 124, 146, OUT);
        const targetX = 1000 + (i % 3) * 28;
        const targetY = 204 + i * 78;
        const collapse = ramp(f, 136, 148);
        return (
          <div
            key={t.name}
            style={{
              position: "absolute",
              left: t.x + (targetX - t.x) * push,
              top: t.y + (targetY - t.y) * push + (1 - p) * 72 + float * (1 - push),
              opacity: p * (1 - collapse),
              transform: `scale(${1 - 0.62 * push})`,
              transformOrigin: "0% 0%",
            }}
          >
            <Tile name={t.name} path={t.path} x={0} y={0} rot={t.rot * (1 - push)} opacity={1} width={286} />
          </div>
        );
      })}

      {/* the statement */}
      <div
        style={{
          position: "absolute",
          left: 110 - textOut * 60,
          top: 168,
          width: 800,
          opacity: 1 - textOut,
        }}
      >
        <div style={{ fontFamily: F, fontSize: 126, fontWeight: 800, lineHeight: 0.9, letterSpacing: "-0.035em", color: C.text }}>Nine clients.</div>
        <div style={{ marginTop: 24, fontFamily: F, fontSize: 62, fontWeight: 700, lineHeight: 1.0, letterSpacing: "-0.02em", color: C.text }}>Nine integration layers.</div>
        <div style={{ marginTop: 44, fontFamily: MONO, fontSize: 20, lineHeight: 1.4, color: C.muted, whiteSpace: "nowrap" }}>
          discover · parse · authenticate · stream
        </div>

        {/* per-client glue stack */}
        <div style={{ marginTop: 54, width: 520 }}>
          <div style={{ fontFamily: MONO, fontSize: 10.5, letterSpacing: "0.18em", color: C.faint, opacity: ramp(f, 22, 32) }}>PER-CLIENT GLUE</div>
          {GLUE.map((file, i) => {
            const p = ramp(f, 34 + i * 3, 46 + i * 3);
            return (
              <div
                key={file}
                style={{
                  height: 38,
                  display: "flex",
                  alignItems: "center",
                  borderBottom: "1px solid " + C.border,
                  opacity: p,
                  transform: `translateX(${(1 - p) * -12}px)`,
                }}
              >
                <div style={{ width: 10, height: 10, border: "1px solid " + C.faint, borderRadius: 2 }} />
                <div style={{ marginLeft: 12, fontFamily: MONO, fontSize: 14.5, color: C.text }}>{file}</div>
                <div style={{ flex: 1 }} />
                <div style={{ fontFamily: MONO, fontSize: 12, color: C.faint }}>× 9</div>
              </div>
            );
          })}
        </div>

        <div style={{ marginTop: 26, fontFamily: F, fontSize: 23, fontWeight: 500, color: C.muted }}>once per client, in every app.</div>
      </div>

      {/* amber rule */}
      <div
        style={{
          position: "absolute",
          left: 90,
          top: 176,
          width: 3,
          height: 590 * ramp(f, 34, 100),
          backgroundColor: C.amber,
          opacity: 0.95 * (1 - textOut),
        }}
      />
    </div>
  );
}
