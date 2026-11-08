/* Entry for the design-exploration routes. /c1 … /c5 and /concepts are rewritten to
   concepts.html by the dev/preview middleware in vite.config.ts; the published landing
   page at / (index.html + src/main.tsx) is untouched. Each concept is a separate chunk
   with its own stylesheet, and links between concepts are full page loads, so no
   concept's CSS leaks into another. */
import { StrictMode, type ComponentType } from "react";
import { createRoot } from "react-dom/client";

const ROUTES: Record<string, () => Promise<{ default: ComponentType }>> = {
  "/concepts": () => import("./index/ConceptsIndex"),
  "/c1": () => import("./c1/Switchboard"),
  "/c2": () => import("./c2/Manifesto"),
  "/c3": () => import("./c3/Stone"),
  "/c4": () => import("./c4/Desktop"),
  "/c5": () => import("./c5/Wire"),
};

const path = window.location.pathname.replace(/\/+$/, "").replace(/\.html$/, "") || "/concepts";
const load = ROUTES[path] ?? ROUTES["/concepts"];
const root = document.getElementById("root");
if (!root) throw new Error("Root element missing");

load().then(({ default: Page }) => {
  createRoot(root).render(<StrictMode><Page /></StrictMode>);
});
