import type { ReactNode } from "react";
import { Github } from "./Icon";
import logo from "../assets/brand/originlm-horizontal-dark.svg";

const COLUMNS = [
  {
    title: "Product",
    links: [
      ["See your AI", "#paths-title"],
      ["Build with it", "#developers"],
      ["How it works", "#how"],
      ["Providers", "#providers"],
      ["Security", "#security"],
    ],
  },
  {
    title: "Developers",
    links: [
      ["Documentation", "https://github.com/thomaslittle/originlm/tree/main/docs"],
      ["Integrations", "#integrations"],
      ["Support matrix", "https://github.com/thomaslittle/originlm/blob/main/docs/adapters/SUPPORT-MATRIX.md"],
      ["Examples", "https://github.com/thomaslittle/originlm/tree/main/examples"],
      ["Install", "#install"],
    ],
  },
  {
    title: "Project",
    links: [
      ["GitHub", "https://github.com/thomaslittle/originlm"],
      ["Changelog", "https://github.com/thomaslittle/originlm/blob/main/CHANGELOG.md"],
      ["Threat model", "https://github.com/thomaslittle/originlm/blob/main/docs/security/threat-model.md"],
      ["License", "https://github.com/thomaslittle/originlm/blob/main/LICENSE"],
    ],
  },
];

export function Footer({ children }: { children?: ReactNode }) {
  return (
    <footer className="footer">
      <div className="container">
        {children}

        <div className="footer-grid">
          <div className="footer-brand">
            <img src={logo} alt="OriginLM" width={906} height={208} />
            <p>
              A local control center for every AI client on your machine, and the one dependency that
              puts the user’s own AI in your app. Private by design.
            </p>
            <a className="btn btn-ghost" href="https://github.com/thomaslittle/originlm">
              <Github size={16} /> Star on GitHub
            </a>
          </div>

          {COLUMNS.map((column) => (
            <div className="footer-col" key={column.title}>
              <h3>{column.title}</h3>
              <ul>
                {column.links.map(([label, href]) => (
                  <li key={label}><a href={href}>{label}</a></li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="footer-bottom">
          <span>© 2026 OriginLM. MIT-licensed and local-first.</span>
          <span>
            No telemetry on this page. <a href="https://github.com/thomaslittle/originlm/blob/main/docs/security/threat-model.md">Threat model</a>
          </span>
        </div>
      </div>
    </footer>
  );
}
