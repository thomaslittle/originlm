/// <reference types="node" />
import { defineConfig, type Connect, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

/* Design-exploration routes (/concepts, /c1 … /c5) are served by concepts.html. The
   published landing page at / keeps index.html. */
const CONCEPT_ROUTE = /^\/(concepts|c[1-5])\/?(\?.*)?$/;
const rewrite: Connect.NextHandleFunction = (req, _res, next) => {
  if (req.url && CONCEPT_ROUTE.test(req.url)) req.url = "/concepts.html" + (req.url.split("?")[1] ? `?${req.url.split("?")[1]}` : "");
  next();
};
const conceptRoutes = (): Plugin => ({
  name: "originlm-concept-routes",
  configureServer(server) { server.middlewares.use(rewrite); },
  configurePreviewServer(server) { server.middlewares.use(rewrite); },
});

// Static, self-contained build: no server functions, no remote runtime calls.
export default defineConfig(({ isSsrBuild }) => ({
  plugins: [react(), conceptRoutes()],
  build: {
    target: "es2020",
    sourcemap: true,
    assetsInlineLimit: 2048,
    rolldownOptions: {
      ...(isSsrBuild ? {} : {
        input: {
          main: fileURLToPath(new URL("./index.html", import.meta.url)),
          concepts: fileURLToPath(new URL("./concepts.html", import.meta.url)),
        },
      }),
      output: {
        codeSplitting: {
          groups: [{ name: "react-vendor", test: /node_modules[\/](react|react-dom|scheduler)[\/]/ }],
        },
      },
    },
  },
}));
