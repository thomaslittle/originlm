import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { existsSync } from "node:fs";

const filmAvailable = ["originlm-promo.mp4", "originlm-promo.webm", "originlm-promo-poster.jpg"].every(
  (name) => existsSync(new URL(`./public/media/${name}`, import.meta.url)),
);

// Static, self-contained build: no server functions, no remote runtime calls.
export default defineConfig({
  plugins: [react()],
  define: { __ORIGINLM_FILM_AVAILABLE__: JSON.stringify(filmAvailable) },
  build: {
    target: "es2020",
    sourcemap: true,
    assetsInlineLimit: 2048,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            { name: "three-core", test: /node_modules[\\/]three[\\/]/ },
            { name: "react-vendor", test: /node_modules[\\/](react|react-dom|scheduler)[\\/]/ },
          ],
        },
      },
    },
  },
});
