import { useEffect, useRef, useState } from "react";
import { Close, Github, Menu } from "./Icon";
import { BrandLogo } from "./BrandLogo";

const LINKS = [
  { label: "SDK", href: "#sdk" },
  { label: "Grants", href: "#grants" },
  { label: "Sessions", href: "#sessions" },
  { label: "Support", href: "#providers" },
  { label: "Usage", href: "#usage" },
  { label: "Docs", href: "https://github.com/thomaslittle/originlm/tree/main/docs" },
];

export function Nav() {
  const [scrolled, setScrolled] = useState(false);
  const [open, setOpen] = useState(false);
  const toggle = useRef<HTMLButtonElement>(null);
  const nav = useRef<HTMLElement>(null);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 24);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        toggle.current?.focus();
      }
    };
    const onPointer = (event: PointerEvent) => {
      if (!nav.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
  }, [open]);

  return (
    <nav ref={nav} className="nav" data-scrolled={scrolled} aria-label="Primary">
      <div className="container nav-bar">
        <a className="nav-logo" href="#top" aria-label="OriginLM home">
          <BrandLogo />
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
          <a className="btn btn-primary btn-sm" href="#install">Install</a>
          <button
            ref={toggle}
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

      {/* Always mounted so aria-controls resolves; hidden removes it from the tab order. */}
      <div className="nav-mobile" id="mobile-menu" hidden={!open}>
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
    </nav>
  );
}
