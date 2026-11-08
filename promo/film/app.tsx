import { C, Check, Dot, F, MONO, Mark, OUT, between, ramp } from "./kit";

const CODE = [
  { t: 'import { decode } from "./codec";', k: "kw" },
  { t: 'import type { Chunk, Sink } from "./types";', k: "kw" },
  { t: "", k: "ctx" },
  { t: "export async function readStream(", k: "kw" },
  { t: "  res: Response,", k: "ctx" },
  { t: "  onChunk: (c: Chunk) => void,", k: "ctx" },
  { t: ") {", k: "ctx" },
  { t: "  const reader = res.body!.getReader();", k: "ctx" },
  { t: "  try {", k: "kw" },
  { t: "    while (true) {", k: "kw" },
  { t: "      const { done, value } = await reader.read();", k: "ctx" },
  { t: "      if (done) break;", k: "kw" },
  { t: "      onChunk(decode(value));", k: "ctx" },
  { t: "    }", k: "ctx" },
  { t: "  } finally {", k: "kw" },
  { t: "    await reader.cancel();", k: "ctx" },
  { t: "  }", k: "ctx" },
  { t: "}", k: "ctx" },
  { t: "", k: "ctx" },
  { t: "// tail can still be buffered when done arrives", k: "comment" },
  { t: "", k: "ctx" },
  { t: "+   onChunk(flushTail());", k: "add" },
];

const FILES = ["src/", "  transport.ts", "  codec.ts", "  retry.ts", "  stream.test.ts", "package.json", "README.md"];

const ANSWER =
  "When done arrives, the tail chunk is still buffered. Flush the decoder in finally, then test a truncated stream.";

function codeColor(k: string): string {
  if (k === "kw") return C.lightWarning;
  if (k === "comment") return C.lightMuted;
  if (k === "add") return C.lightSuccess;
  return C.ink;
}

export default function SceneApp({ f }: { f: number }) {
  if (!between(f, 292, 500)) return null;
  const t = f - 300;
  const inP = ramp(t, 0, 20);
  const out = ramp(t, 178, 198);
  const dialogIn = ramp(t, 32, 48);
  const dialogOut = ramp(t, 86, 98);
  const granted = t >= 74;
  const overlay = dialogIn * (1 - dialogOut);
  const words = ANSWER.split(" ");
  const streamN = Math.max(0, Math.min(words.length, Math.floor((t - 104) * 0.52)));
  const caretOn = Math.floor(t / 9) % 2 === 0;

  return (
    <div style={{ position: "absolute", inset: 0 }}>
      <div
        style={{
          position: "absolute",
          left: 180,
          top: 112 + (1 - inP) * 30 - out * 90,
          width: 1560,
          height: 768,
          borderRadius: 18,
          backgroundColor: "#f7f4ed",
          border: "1px solid " + C.lightBorder,
          boxShadow: "0 50px 120px rgba(0,0,0,0.5)",
          opacity: inP * (1 - out),
          overflow: "hidden",
          transform: `scale(${0.985 + inP * 0.015})`,
        }}
      >
        {/* window chrome */}
        <div style={{ height: 62, display: "flex", alignItems: "center", padding: "0 22px", borderBottom: "1px solid " + C.border, backgroundColor: C.bg }}>
          <div style={{ width: 22, height: 22, borderRadius: 7, backgroundColor: C.terracotta, color: "#fff", fontFamily: F, fontSize: 13, fontWeight: 800, display: "flex", alignItems: "center", justifyContent: "center" }}>A</div>
          <div style={{ marginLeft: 10, fontFamily: F, fontSize: 22, fontWeight: 800, color: C.text, letterSpacing: "-0.02em", lineHeight: 1 }}>Atlas</div>
          <div style={{ marginLeft: 8, fontFamily: MONO, fontSize: 11, color: C.muted, letterSpacing: "0.08em" }}>REVIEW</div>
          <div style={{ flex: 1 }} />
          <div style={{ fontFamily: MONO, fontSize: 12, color: C.muted }}>feat/stream-retry / src/transport.ts</div>
          <div style={{ flex: 1 }} />
          <div style={{ padding: "5px 9px", border: "1px solid " + C.border, borderRadius: 6, fontFamily: MONO, fontSize: 11, color: C.muted }}>3 files · +48 −6</div>
        </div>

        <div style={{ display: "flex", height: 652 }}>
          {/* file rail */}
          <div style={{ width: 230, backgroundColor: "#f1ede5", borderRight: "1px solid " + C.lightBorder, padding: "18px 0" }}>
            <div style={{ padding: "0 20px 12px 20px", fontFamily: MONO, fontSize: 10, letterSpacing: "0.16em", color: C.lightMuted }}>CHANGES</div>
            {FILES.map((file, i) => {
              const selected = i === 1;
              return (
                <div
                  key={file}
                  style={{
                    height: 34,
                    display: "flex",
                    alignItems: "center",
                    paddingLeft: 20,
                    fontFamily: MONO,
                    fontSize: 12.5,
                    color: selected ? C.ink : C.lightMuted,
                    backgroundColor: selected ? "#e8e1d5" : "transparent",
                    borderLeft: selected ? "3px solid " + C.terracotta : "3px solid transparent",
                    whiteSpace: "pre",
                  }}
                >
                  {file}
                </div>
              );
            })}
            <div style={{ marginTop: 26, padding: "0 20px", fontFamily: MONO, fontSize: 10, letterSpacing: "0.16em", color: C.lightMuted }}>CHECKS</div>
            <div style={{ marginTop: 12, padding: "0 20px", display: "flex", alignItems: "center", gap: 8 }}>
              <Dot color={C.lightSuccess} size={6} />
              <div style={{ fontFamily: MONO, fontSize: 11.5, color: C.lightMuted }}>stream.test.ts</div>
            </div>
            <div style={{ marginTop: 8, padding: "0 20px", display: "flex", alignItems: "center", gap: 8 }}>
              <Dot color={C.lightWarning} size={6} />
              <div style={{ fontFamily: MONO, fontSize: 11.5, color: C.lightMuted }}>1 tail case</div>
            </div>
          </div>

          {/* diff */}
          <div style={{ width: 830, backgroundColor: "#fbf9f4" }}>
            <div style={{ height: 40, display: "flex", alignItems: "center", padding: "0 18px", borderBottom: "1px solid " + C.lightBorder, fontFamily: MONO, fontSize: 11, color: C.lightMuted }}>
              @@ −41,7 +41,9 @@ readStream
            </div>
            {CODE.map((line, i) => (
              <div
                key={i}
                style={{
                  height: 27,
                  display: "flex",
                  alignItems: "center",
                  backgroundColor: line.k === "add" ? "rgba(76,111,88,0.12)" : "transparent",
                }}
              >
                <div style={{ width: 46, textAlign: "right", paddingRight: 12, fontFamily: MONO, fontSize: 10.5, color: "#a49b8f" }}>{line.t === "" ? "" : 41 + i}</div>
                <div style={{ width: 20, textAlign: "center", fontFamily: MONO, fontSize: 11.5, color: line.k === "add" ? C.lightSuccess : "#b3aa9e" }}>
                  {line.k === "add" ? "+" : " "}
                </div>
                <div style={{ fontFamily: MONO, fontSize: 14, color: codeColor(line.k), whiteSpace: "pre" }}>{line.t}</div>
              </div>
            ))}
          </div>

          {/* chat */}
          <div style={{ width: 500, backgroundColor: "#faf9f5", borderLeft: "1px solid " + C.lightBorder, display: "flex", flexDirection: "column" }}>
            <div style={{ height: 56, display: "flex", alignItems: "center", padding: "0 20px", borderBottom: "1px solid " + C.lightBorder }}>
              <div style={{ fontFamily: F, fontSize: 15, fontWeight: 800, color: C.ink }}>Review</div>
              <div style={{ flex: 1 }} />
              <Dot color={granted ? C.lightSuccess : C.lightMuted} size={6} />
              <div style={{ marginLeft: 8, fontFamily: MONO, fontSize: 11.5, color: C.lightMuted }}>Claude Code · Haiku</div>
            </div>

            <div style={{ flex: 1, padding: "20px 20px 0 20px", overflow: "hidden" }}>
              <div style={{ display: "flex", justifyContent: "flex-end" }}>
                <div style={{ maxWidth: 360, backgroundColor: "#ece6da", borderRadius: "14px 14px 4px 14px", padding: "12px 15px", fontFamily: F, fontSize: 15.5, lineHeight: 1.45, color: C.ink }}>
                  Why does the last chunk disappear under load?
                </div>
              </div>

              {!granted ? (
                <div style={{ marginTop: 34, opacity: ramp(t, 48, 60) }}>
                  <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: "0.16em", color: C.lightMuted }}>NO SESSION</div>
                  <div style={{ marginTop: 10, fontFamily: F, fontSize: 14.5, lineHeight: 1.5, color: C.lightMuted }}>
                    Grant Atlas a session to ask Claude Code about this diff.
                  </div>
                </div>
              ) : (
                <div style={{ marginTop: 26 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <Mark width={16} />
                    <div style={{ fontFamily: MONO, fontSize: 10.5, color: C.lightMuted }}>streaming · Claude Code</div>
                  </div>
                  <div style={{ marginTop: 10, fontFamily: F, fontSize: 16.5, lineHeight: 1.5, color: C.ink }}>
                    {words.slice(0, Math.max(0, streamN - 1)).join(" ")}
                    {streamN > 0 ? <span style={{ color: C.terracotta }}> {words[streamN - 1]}</span> : null}
                    <span style={{ opacity: caretOn ? 1 : 0, color: C.terracotta }}>▍</span>
                  </div>
                </div>
              )}
            </div>

            <div style={{ height: 54, margin: "0 20px 18px 20px", border: "1px solid " + (granted ? C.lightBorder : "#ddd6ca"), borderRadius: 10, display: "flex", alignItems: "center", padding: "0 14px", backgroundColor: "#fff" }}>
              <div style={{ fontFamily: F, fontSize: 13.5, color: "#a49b8f" }}>Ask about this diff…</div>
              <div style={{ flex: 1 }} />
              <div style={{ width: 24, height: 24, borderRadius: 7, backgroundColor: granted ? C.terracotta : "#e6e0d4", display: "flex", alignItems: "center", justifyContent: "center", color: "#fff", fontSize: 13 }}>↑</div>
            </div>
          </div>
        </div>

        {/* OriginLM strip */}
        <div style={{ height: 54, backgroundColor: C.bg, display: "flex", alignItems: "center", padding: "0 22px" }}>
          <Mark width={17} />
          <div style={{ marginLeft: 9, fontFamily: F, fontSize: 13, fontWeight: 700, color: C.text, letterSpacing: "0.01em" }}>originlm</div>
          <div style={{ marginLeft: 10, fontFamily: MONO, fontSize: 11.5, color: C.faint }}>local control plane</div>
          <div style={{ flex: 1 }} />
          <div style={{ fontFamily: MONO, fontSize: 11.5, color: granted ? C.success : C.faint }}>
            {granted ? "session 7f3a · stream · revocable" : "no session"}
          </div>
          <div style={{ marginLeft: 12, width: 6, height: 6, borderRadius: 3, backgroundColor: granted ? C.success : C.faint }} />
        </div>

        {/* dim + grant dialog */}
        <div style={{ position: "absolute", inset: 0, backgroundColor: "rgba(12,10,11,0.28)", opacity: overlay }} />
        {overlay > 0.01 ? (
          <div
            style={{
              position: "absolute",
              left: 450,
              top: 196,
              width: 660,
              height: granted ? 272 : 386,
              boxSizing: "border-box",
              padding: 30,
              borderRadius: 20,
              backgroundColor: C.surface,
              border: "1px solid " + C.border,
              boxShadow: "0 40px 100px rgba(0,0,0,0.55)",
              opacity: overlay,
              transform: `translateY(${(1 - dialogIn) * 24}px) scale(${0.97 + dialogIn * 0.03})`,
            }}
          >
            <div style={{ display: "flex", alignItems: "center" }}>
              <Mark width={20} />
              <div style={{ marginLeft: 8, fontFamily: F, fontSize: 12, fontWeight: 700, letterSpacing: "0.18em", color: C.muted }}>ORIGINLM</div>
              <div style={{ flex: 1 }} />
              <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: "0.12em", color: C.faint }}>{granted ? "GRANTED" : "GRANT REQUEST"}</div>
            </div>

            <div style={{ marginTop: 22, fontFamily: F, fontSize: 33, fontWeight: 800, letterSpacing: "-0.02em", color: C.text, lineHeight: 1.05 }}>
              {granted ? "Session granted" : "Atlas requests a session"}
            </div>
            <div style={{ marginTop: 9, fontFamily: F, fontSize: 15, fontWeight: 500, color: C.muted }}>
              {granted ? "Claude Code · Haiku · 7f3a" : "Grants are separate and revocable."}
            </div>

            {!granted ? (
              <>
                <div style={{ marginTop: 24, display: "flex", flexDirection: "column", gap: 14 }}>
                  {["Streaming text", "Read-only host tools", "Cancel anytime"].map((label, i) => {
                    const p = ramp(t, 40 + i * 4, 48 + i * 4);
                    return (
                      <div key={label} style={{ display: "flex", alignItems: "center", gap: 12, opacity: p, transform: `translateX(${(1 - p) * 10}px)` }}>
                        <Check size={18} color={C.success} />
                        <div style={{ fontFamily: F, fontSize: 16, fontWeight: 500, color: C.text }}>{label}</div>
                      </div>
                    );
                  })}
                </div>
                <div style={{ position: "absolute", left: 30, right: 30, bottom: 28, display: "flex", gap: 12 }}>
                  <div style={{ flex: 1, height: 48, borderRadius: 10, border: "1px solid " + C.border, display: "flex", alignItems: "center", justifyContent: "center", fontFamily: F, fontSize: 15, fontWeight: 700, color: C.muted }}>Deny</div>
                  <div
                    style={{
                      flex: 1.4,
                      height: 48,
                      borderRadius: 10,
                      backgroundColor: t > 66 ? C.amberSoft : C.amber,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontFamily: F,
                      fontSize: 15,
                      fontWeight: 800,
                      color: C.ink,
                      transform: `scale(${t > 66 && t < 72 ? 0.96 : 1})`,
                    }}
                  >
                    Grant
                  </div>
                </div>
              </>
            ) : (
              <div style={{ marginTop: 26 }}>
                <div style={{ display: "flex", gap: 10 }}>
                  <div style={{ padding: "8px 12px", border: "1px solid " + C.border, borderRadius: 8, fontFamily: MONO, fontSize: 12, color: C.success }}>session 7f3a</div>
                  <div style={{ padding: "8px 12px", border: "1px solid " + C.border, borderRadius: 8, fontFamily: MONO, fontSize: 12, color: C.text }}>stream</div>
                  <div style={{ padding: "8px 12px", border: "1px solid " + C.border, borderRadius: 8, fontFamily: MONO, fontSize: 12, color: C.text }}>read-only tools</div>
                </div>
                <div style={{ marginTop: 20, fontFamily: F, fontSize: 13, color: C.muted }}>Revoke it here, or from any connected app.</div>
              </div>
            )}
          </div>
        ) : null}
      </div>

      {/* caption below the window */}
      <div style={{ position: "absolute", left: 180, top: 902, display: "flex", alignItems: "baseline", opacity: ramp(t, 136, 154) * (1 - out) }}>
        <div style={{ fontFamily: F, fontSize: 28, fontWeight: 500, color: C.muted }}>Your app stays your app.</div>
        <div style={{ flex: 1 }} />
      </div>
      <div style={{ position: "absolute", left: 1000, top: 896, width: 740, textAlign: "right", opacity: ramp(t, 140, 158) * (1 - out) }}>
        <div style={{ fontFamily: F, fontSize: 34, fontWeight: 700, letterSpacing: "-0.01em", color: C.amber }}>No provider SDK per client.</div>
      </div>
    </div>
  );
}
