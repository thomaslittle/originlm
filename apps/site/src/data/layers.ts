export type Layer = {
  id: string;
  label: string;
  title: string;
  detail: string;
  meta: string;
};

/** The story the 3D hero and the promo video share. Same order, same words. */
export const LAYERS: Layer[] = [
  {
    id: "app",
    label: "Your app",
    title: "Your app",
    detail: "Tauri, Electron, GPUI or the web. One small dependency, no provider code of your own.",
    meta: "Tauri, Electron, GPUI, web",
  },
  {
    id: "sdk",
    label: "SDK",
    title: "Thin, typed clients",
    detail: "Rust, npm and Python surfaces over one protocol. No scanning on import, no secrets in the API.",
    meta: "Rust, npm, Python",
  },
  {
    id: "transport",
    label: "Transport",
    title: "Local transport only",
    detail: "stdio for owned helpers, IPC for the shared broker, Native Messaging for registered extensions.",
    meta: "stdio, IPC, Native Messaging",
  },
  {
    id: "runtime",
    label: "Runtime",
    title: "Grants, discovery, sessions",
    detail: "Every app gets its own revocable grant. Discovery is staged; execution needs permission.",
    meta: "Local and permissioned",
  },
  {
    id: "providers",
    label: "Providers",
    title: "The AI already installed",
    detail: "Claude Code, Codex, Cursor, Grok, Gemini, OpenCode, Copilot, Ollama, LM Studio and more.",
    meta: "18 providers documented",
  },
];
