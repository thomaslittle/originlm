import type { ReactNode } from "react";
import { Github } from "./Icon";
import { BrandLogo } from "./BrandLogo";

const COLUMNS = [
  {
    title: "On this page",
    links: [
      ["How it fits", "#how"],
      ["SDK", "#sdk"],
      ["Grants", "#grants"],
      ["Sessions", "#sessions"],
      ["Support matrix", "#providers"],
      ["Control center", "#usage"],
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
            <BrandLogo />
            <p>
              A local runtime and SDK for building on the AI clients your users already have, and a
              control center that shows them what is left. MIT-licensed alpha.
            </p>
            <a className="btn btn-ghost" href="https://github.com/thomaslittle/originlm">
              <Github size={16} /> Source on GitHub
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
