import { Nav } from "./components/Nav";
import { Footer } from "./components/Footer";
import { Hero } from "./sections/Hero";
import { Paths } from "./sections/Paths";
import { Comparison } from "./sections/Comparison";
import { Install } from "./sections/Install";
import { HowItWorks } from "./sections/HowItWorks";
import { Integrations } from "./sections/Integrations";
import { Providers } from "./sections/Providers";
import { Security } from "./sections/Security";
import { StatusDemo } from "./sections/StatusDemo";
import { Video } from "./sections/Video";
import { FinalCta } from "./sections/FinalCta";

/* Eight sections and the footer. Paths carries the category comparison, Install carries the
   status surfaces, and the closing action lives in the footer's statement. */
export default function App() {
  return (
    <>
      <a className="skip-link" href="#main">Skip to content</a>
      <Nav />
      <main id="main">
        <Hero />
        <section className="section paths" aria-labelledby="paths-title">
          <Paths />
          <Comparison />
        </section>
        <HowItWorks />
        <section className="section band install-band" id="install" aria-labelledby="install-title">
          <Install />
          <StatusDemo />
        </section>
        <Integrations />
        <Providers />
        <Security />
        <Video />
      </main>
      <Footer>
        <FinalCta />
      </Footer>
    </>
  );
}
