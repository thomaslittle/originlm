import type { DockId } from "./wm";

const P = { fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

/** Line icons for the dock apps, drawn on a 24px grid. */
export function AppIcon({ id }: { id: DockId }) {
  switch (id) {
    case "originlm":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="8.2" {...P} />
          <circle cx="12" cy="12" r="3.2" {...P} />
          <path d="M12 3.8v2.4M12 17.8v2.4M3.8 12h2.4M17.8 12h2.4" {...P} />
        </svg>
      );
    case "app":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8.5 7.5 4 12l4.5 4.5M15.5 7.5 20 12l-4.5 4.5M13.4 5.5l-2.8 13" {...P} /></svg>;
    case "docs":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 3.8h8.5L18 7.3v12.9H6z M14.2 3.8v3.8H18 M8.8 11.5h6.4M8.8 14.6h6.4M8.8 17.6h4" {...P} /></svg>;
    case "providers":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3.8 7.2V18a1.3 1.3 0 0 0 1.3 1.3h13.8A1.3 1.3 0 0 0 20.2 18V9.4a1.3 1.3 0 0 0-1.3-1.3h-7L10 5.9H5.1a1.3 1.3 0 0 0-1.3 1.3z" {...P} /></svg>;
    case "grants":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="8" width="17" height="8" rx="4" {...P} /><circle cx="16.4" cy="12" r="2.3" {...P} /></svg>;
    case "sessions":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4.5 6.2h15v9.6H11l-4.2 3.4v-3.4H4.5z M8 10h8M8 12.8h5" {...P} /></svg>;
    case "usage":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4.6 16.6a8 8 0 1 1 14.8 0 M12 12.6l3.6-3.4" {...P} /><circle cx="12" cy="13" r="1.2" fill="currentColor" /></svg>;
    case "install":
      return <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="4.8" width="17" height="14.4" rx="2" {...P} /><path d="M7.4 9.6 10 12l-2.6 2.4M12.2 14.8h4.4" {...P} /></svg>;
  }
}

export function Lock() {
  return <svg viewBox="0 0 24 24" aria-hidden="true" width="18" height="18"><rect x="5.5" y="10.5" width="13" height="9.2" rx="1.8" {...P} /><path d="M8.6 10.5V8a3.4 3.4 0 0 1 6.8 0v2.5" {...P} /></svg>;
}
