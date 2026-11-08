import { AbsoluteFill, Composition, registerRoot, useCurrentFrame, useVideoConfig } from "raea";
import { css } from "./styles.gen";
import { C, DUR, Grid, Vignette, ramp } from "./film/kit";
import ScenePile from "./film/pile";
import SceneInventory from "./film/inventory";
import SceneApp from "./film/app";
import SceneSafety from "./film/safety";
import SceneEnd from "./film/end";

function Film() {
  const f = useCurrentFrame();
  const { width, height } = useVideoConfig();
  const g = f / (DUR - 1);
  const scale = 1.015 + 0.045 * g;
  const x = -14 * g;
  const y = -7 * g;
  const gridOpacity = 0.5 - 0.2 * ramp(f, 628, 690);

  return (
    <AbsoluteFill style={{ backgroundColor: C.bg, fontFamily: "var(--brand-font-heading)" }}>
      <style>{css}</style>

      <div style={{ position: "absolute", inset: 0, opacity: gridOpacity }}>
        <Grid shift={18 * g} opacity={0.5} />
      </div>
      <Vignette amber={0.055} />

      <div
        style={{
          position: "absolute",
          left: 0,
          top: 0,
          width: width,
          height: height,
          transformOrigin: "50% 50%",
          transform: `translate(${x}px, ${y}px) scale(${scale})`,
        }}
      >
        <ScenePile f={f} />
        <SceneInventory f={f} />
        <SceneApp f={f} />
        <SceneSafety f={f} />
        <SceneEnd f={f} />
      </div>

      {/* corner registration marks */}
      <div style={{ position: "absolute", left: 54, top: 54, width: 18, height: 1, backgroundColor: "rgba(176,168,161,0.3)" }} />
      <div style={{ position: "absolute", left: 54, top: 54, width: 1, height: 18, backgroundColor: "rgba(176,168,161,0.3)" }} />
      <div style={{ position: "absolute", right: 54, bottom: 54, width: 18, height: 1, backgroundColor: "rgba(176,168,161,0.3)" }} />
      <div style={{ position: "absolute", right: 54, bottom: 54, width: 1, height: 18, backgroundColor: "rgba(176,168,161,0.3)" }} />
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="originlm-promo" component={Film} width={1920} height={1080} fps={30} durationInFrames={DUR} />);
