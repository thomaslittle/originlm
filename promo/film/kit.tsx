import { Easing, Img, interpolate, staticFile } from "raea";

export const W = 1920;
export const H = 1080;
export const FPS = 30;
export const DUR = 800;

export const C = {
  bg: "#1e1a1b",
  surface: "#262122",
  raised: "#2e2829",
  border: "#3b3334",
  text: "#faf9f5",
  muted: "#b0a8a1",
  faint: "#6f625c",
  amber: "#eba31a",
  amberSoft: "#f7bd4b",
  ink: "#231f20",
  success: "#7cc08f",
  danger: "#e0776b",
  paper: "#faf9f5",
  lightBorder: "#e5dfd4",
  lightMuted: "#6b635c",
  lightSuccess: "#4c6f58",
  lightWarning: "#876012",
  terracotta: "#c97e5a",
  sage: "#a9b29f",
};

export const CLAMP = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
export const OUT = Easing.out(Easing.cubic);
export const INOUT = Easing.inOut(Easing.cubic);

export const F = "var(--brand-font-heading)";
export const MONO = "var(--brand-mono)";

export function ramp(f: number, a: number, b: number, easing = OUT): number {
  return interpolate(f, [a, b], [0, 1], { ...CLAMP, easing });
}

export function between(f: number, a: number, b: number): boolean {
  return f >= a && f <= b;
}

export function mix(f: number, a: number, b: number, from: number, to: number, easing = OUT): number {
  return interpolate(f, [a, b], [from, to], { ...CLAMP, easing });
}

/** Frame-pure pseudo-random in [0, 1) from a seed. */
export function rand(seed: number): number {
  const x = Math.sin(seed * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

export function Grid({ shift = 0, opacity = 0.42 }: { shift?: number; opacity?: number }) {
  const xs: number[] = [];
  const ys: number[] = [];
  for (let x = -80; x <= W + 80; x += 80) xs.push(x);
  for (let y = -80; y <= H + 80; y += 80) ys.push(y);
  return (
    <svg width={W} height={H} style={{ position: "absolute", left: 0, top: 0, opacity }}>
      {xs.map((x, i) => (
        <line key={`x${i}`} x1={x + shift} y1={-80} x2={x + shift} y2={H + 80} stroke={C.muted} strokeWidth={1} opacity={0.16} />
      ))}
      {ys.map((y, i) => (
        <line key={`y${i}`} x1={-80} y1={y + shift * 0.55} x2={W + 80} y2={y + shift * 0.55} stroke={C.muted} strokeWidth={1} opacity={0.16} />
      ))}
    </svg>
  );
}

export function Vignette({ amber = 0.05 }: { amber?: number }) {
  return (
    <>
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage:
            "radial-gradient(ellipse at 62% 38%, rgba(235,163,26," + amber + ") 0%, rgba(30,26,27,0) 52%)",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage:
            "radial-gradient(ellipse at 50% 50%, rgba(30,26,27,0) 46%, rgba(15,13,14,0.55) 100%)",
        }}
      />
    </>
  );
}

export function Label({ children, color = C.muted, size = 15, tracking = "0.2em" }: { children: any; color?: string; size?: number; tracking?: string }) {
  return (
    <div style={{ fontFamily: F, fontSize: size, fontWeight: 700, letterSpacing: tracking, textTransform: "uppercase", color, lineHeight: 1 }}>
      {children}
    </div>
  );
}

export function Dot({ color = C.success, size = 8 }: { color?: string; size?: number }) {
  return <div style={{ width: size, height: size, borderRadius: size / 2, backgroundColor: color, flexShrink: 0 }} />;
}

export function Tile({
  name,
  path,
  x,
  y,
  rot,
  opacity,
  lift = 0,
  width = 286,
}: {
  name: string;
  path: string;
  x: number;
  y: number;
  rot: number;
  opacity: number;
  lift?: number;
  width?: number;
}) {
  return (
    <div
      style={{
        position: "absolute",
        left: x,
        top: y,
        width,
        height: 140,
        boxSizing: "border-box",
        padding: "20px 22px 18px 22px",
        borderRadius: 18,
        backgroundColor: C.surface,
        border: "1px solid " + C.border,
        boxShadow: "0 30px 60px rgba(0,0,0,0.38)",
        opacity,
        transform: `translateY(${lift}px) rotate(${rot}deg)`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ fontFamily: F, fontSize: 25, fontWeight: 700, color: C.text, letterSpacing: "-0.01em", lineHeight: 1 }}>{name}</div>
        <Dot color={C.faint} size={7} />
      </div>
      <div style={{ marginTop: 12, fontFamily: MONO, fontSize: 13, color: C.muted, whiteSpace: "nowrap", overflow: "hidden" }}>{path}</div>
      <div style={{ display: "flex", gap: 5, marginTop: 16 }}>
        <div style={{ width: 34, height: 2, backgroundColor: C.border }} />
        <div style={{ width: 18, height: 2, backgroundColor: C.border }} />
        <div style={{ width: 9, height: 2, backgroundColor: C.faint }} />
      </div>
    </div>
  );
}

export function Check({ size = 18, color = C.success }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 18 18" style={{ flexShrink: 0 }}>
      <rect x="1" y="1" width="16" height="16" rx="4" fill="none" stroke={color} strokeWidth="1.5" />
      <path d="M4.5 9.2 L7.5 12.2 L13.5 5.8" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Key({ x, y, size = 46, color = C.amber, opacity = 1 }: { x: number; y: number; size?: number; color?: string; opacity?: number }) {
  return (
    <svg width={size * 2} height={size} viewBox="0 0 92 46" style={{ position: "absolute", left: x, top: y, opacity }}>
      <circle cx="17" cy="23" r="12" fill="none" stroke={color} strokeWidth="3" />
      <circle cx="17" cy="23" r="3.5" fill={color} />
      <path d="M29 23 H79 M66 23 V36 M78 23 V32" fill="none" stroke={color} strokeWidth="3" strokeLinecap="square" />
    </svg>
  );
}

export function Cloud({ x, y, size = 150, color = C.muted, opacity = 1, slash = 0 }: { x: number; y: number; size?: number; color?: string; opacity?: number; slash?: number }) {
  return (
    <svg width={size} height={size * 0.7} viewBox="0 0 150 105" style={{ position: "absolute", left: x, top: y, opacity }}>
      <path d="M32 78 C15 78 8 65 14 54 C19 45 30 42 37 46 C40 27 58 18 75 24 C90 12 117 20 119 42 C136 43 143 56 138 68 C134 78 122 82 110 81 L40 81" fill="none" stroke={color} strokeWidth="3" strokeLinejoin="round" />
      {slash > 0 ? (
        <line x1={14 + 122 * (1 - slash)} y1={88} x2={14 + 122 * (1 - slash) + 122 * slash} y2={16} stroke={C.amber} strokeWidth="4" strokeLinecap="round" />
      ) : null}
    </svg>
  );
}

export function PlayTile({ x, y, size = 64, opacity = 1, slash = 0 }: { x: number; y: number; size?: number; opacity?: number; slash?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" style={{ position: "absolute", left: x, top: y, opacity }}>
      <rect x="2" y="2" width="60" height="60" rx="12" fill="none" stroke={C.border} strokeWidth="2" />
      <path d="M25 19 L45 32 L25 45 Z" fill="none" stroke={C.muted} strokeWidth="2.5" strokeLinejoin="round" />
      {slash > 0 ? <line x1={10 + 44 * (1 - slash)} y1={54} x2={10 + 44 * (1 - slash) + 44 * slash} y2={10} stroke={C.amber} strokeWidth="3.5" strokeLinecap="round" /> : null}
    </svg>
  );
}

export function Cross({ x, y, size = 20, color = C.amber, opacity = 1 }: { x: number; y: number; size?: number; color?: string; opacity?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 20 20" style={{ position: "absolute", left: x, top: y, opacity }}>
      <path d="M3 3 L17 17 M17 3 L3 17" stroke={color} strokeWidth="2.5" strokeLinecap="round" />
    </svg>
  );
}

export const WORDMARK = staticFile("brand/originlm-horizontal-dark.svg");
export const MARK = staticFile("brand/originlm-mark-amber.svg");

export function Wordmark({ width = 700, style = {} }: { width?: number; style?: any }) {
  return <Img src={WORDMARK} style={{ width, display: "block", ...style }} />;
}

export function Mark({ width = 120, style = {} }: { width?: number; style?: any }) {
  return <Img src={MARK} style={{ width, display: "block", ...style }} />;
}
