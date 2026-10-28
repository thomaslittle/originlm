import { useEffect, useState } from "react";
import { Close, Github, Menu } from "./Icon";
import logo from "../assets/brand/originlm-horizontal-dark.svg";

const LINKS = [
  { label: "See your AI", href: "#paths-title" },
  { label: "Developers", href: "#developers" },
  { label: "Providers", href: "#providers" },
  { label: "Security", href: "#security" },
];

export function Nav() {
  const [scrolled, setScrolled] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 24);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  return (
    <nav className="nav" data-scrolled={scrolled} aria-label="Primary">
      <div className="container nav-bar">
        <a className="nav-logo" href="#top" aria-label="OriginLM home">
          <img src={logo} alt="OriginLM" width={906} height={208} />
        </a>

        <ul className="nav-links">
          {LINKS.map((link) => (
            <li key={link.href}><a href={link.href}>{link.label}</a></li>
          ))}
        </ul>

        <div className="nav-actions">
          <a
            className="nav-icon-link"
            href="https://github.com/thomaslittle/originlm"
            aria-label="OriginLM on GitHub"
          >
            <Github size={18} />
          </a>
          <a className="btn btn-primary btn-sm" href="#install">Get OriginLM</a>
          <button
            type="button"
            className="nav-toggle"
            aria-label={open ? "Close menu" : "Open menu"}
            aria-expanded={open}
            aria-controls="mobile-menu"
            onClick={() => setOpen((value) => !value)}
          >
            {open ? <Close size={18} /> : <Menu size={18} />}
          </button>
        </div>
      </div>

      {open && (
        <div className="nav-mobile" id="mobile-menu" data-open={open}>
          <ul className="container">
            {LINKS.map((link) => (
              <li key={link.href}>
                <a href={link.href} onClick={() => setOpen(false)}>{link.label}</a>
              </li>
            ))}
            <li><a href="#install" onClick={() => setOpen(false)}>Install</a></li>
            <li>
              <a href="https://github.com/thomaslittle/originlm" onClick={() => setOpen(false)}>
                GitHub
              </a>
            </li>
          </ul>
        </div>
      )}
    </nav>
  );
}
