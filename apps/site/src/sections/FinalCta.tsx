import { Title } from "../components/Title";
import { ArrowRight } from "../components/Icon";
import { CopyButton } from "../components/CopyButton";

/* The closing shot. The scene comes back up behind this block; the eye settles, blinks
   once and turns to the install command (see motion/director.ts). */
export function FinalCta() {
  return (
    <div className="final-cta">
      <div className="final-cta-still" aria-hidden="true" />
      <Title id="cta-title" className="final-title" text="The AI is already on the machine. Build on it." />
      <div className="final-cta-side">
        <div className="cmd cmd--lg" data-magnetic>
          <span className="prompt" aria-hidden="true">$</span>
          <code>npm install originlm@alpha</code>
          <CopyButton value="npm install originlm@alpha" label="Copy" />
        </div>
        <div className="actions">
          <a className="btn btn-primary" data-magnetic href="https://github.com/thomaslittle/originlm/blob/main/docs/integration/quickstarts.md">
            Framework quickstarts <ArrowRight size={16} />
          </a>
          <a className="btn btn-ghost" data-magnetic href="https://github.com/thomaslittle/originlm/tree/main/examples">Examples</a>
        </div>
      </div>
    </div>
  );
}
