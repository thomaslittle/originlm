import { useEffect } from "react";
import { Nav } from "./components/Nav";
import { Footer } from "./components/Footer";
import { SceneLayer } from "./scene/SceneLayer";
import { Hero } from "./sections/Hero";
import { Story } from "./sections/Story";
import { Architecture } from "./sections/Architecture";
import { Sdk } from "./sections/Sdk";
import { Grants } from "./sections/Grants";
import { Sessions } from "./sections/Sessions";
import { Integrations } from "./sections/Integrations";
import { Providers } from "./sections/Providers";
import { ControlCenter } from "./sections/ControlCenter";
import { Install } from "./sections/Install";
import { Security } from "./sections/Security";
import { FinalCta } from "./sections/FinalCta";

/* One shot from the eye's point of view: it wakes (hero), finds and wires the clients
   (story), pulls back to show there is no server (how), sinks while the page reads as
   documentation (SDK first, usage lower), and returns for the install command.
   See docs/design/landing-wow-plan.md. */
export default function App() {
  useEffect(() => {
    if (!document.documentElement.classList.contains("motion")) return;
    let stop: (() => void) | undefined;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      import("./motion/director").then(({ startDirector }) => {
        if (!cancelled && document.documentElement.classList.contains("motion")) stop = startDirector();
      }).catch(() => { /* the page still works with native scroll */ });
    }, 30);
    return () => { cancelled = true; window.clearTimeout(timer); stop?.(); };
  }, []);

  return (
    <>
      <a className="skip-link" href="#main">Skip to content</a>
      <SceneLayer />
      <Nav />
      <main id="main">
        <Hero />
        <Story />
        <Architecture />
        <div className="docs">
          <span className="docs-thread" aria-hidden="true" />
          <Sdk />
          <Grants />
          <Sessions />
          <Integrations />
          <Providers />
          <ControlCenter />
          <Security />
          <section className="section install-band" id="install" aria-labelledby="install-title">
            <Install />
          </section>
        </div>
      </main>
      <Footer>
        <FinalCta />
      </Footer>
    </>
  );
}
