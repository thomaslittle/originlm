import { C, F, MONO, Wordmark, between, ramp } from "./kit";

export default function SceneEnd({ f }: { f: number }) {
  if (!between(f, 644, 800)) return null;
  const logoIn = ramp(f, 656, 678);
  const ruleIn = ramp(f, 672, 694);
  const tagIn = ramp(f, 680, 698);
  const metaIn = ramp(f, 700, 714);

  return (
    <div style={{ position: "absolute", inset: 0, opacity: 1 }}>
      <div
        style={{
          position: "absolute",
          left: 960,
          top: 332,
          transform: `translate(-50%, ${(1 - logoIn) * 22}px) scale(${0.985 + logoIn * 0.015})`,
          opacity: logoIn,
        }}
      >
        <Wordmark width={700} />
      </div>

      <div style={{ position: "absolute", left: 960 - 90, top: 552, width: 180, height: 2, backgroundColor: C.amber, opacity: 0.95, transform: `scaleX(${ruleIn})`, transformOrigin: "50% 50%" }} />

      <div
        style={{
          position: "absolute",
          left: 200,
          top: 588,
          width: 1520,
          textAlign: "center",
          fontFamily: F,
          fontSize: 58,
          fontWeight: 700,
          letterSpacing: "-0.015em",
          color: C.text,
          opacity: tagIn,
          transform: `translateY(${(1 - tagIn) * 14}px)`,
        }}
      >
        See your AI. Build with it.
      </div>

      <div
        style={{
          position: "absolute",
          left: 200,
          top: 684,
          width: 1520,
          textAlign: "center",
          fontFamily: F,
          fontSize: 24,
          fontWeight: 500,
          color: C.muted,
          opacity: metaIn,
        }}
      >
        A local control center for the AI already on your machine.
      </div>

      <div
        style={{
          position: "absolute",
          left: 200,
          top: 752,
          width: 1520,
          textAlign: "center",
          fontFamily: MONO,
          fontSize: 14,
          letterSpacing: "0.22em",
          color: C.faint,
          opacity: metaIn,
          textTransform: "uppercase",
        }}
      >
        alpha · MIT
      </div>
    </div>
  );
}
