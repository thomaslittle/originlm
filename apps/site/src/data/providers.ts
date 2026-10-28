export type OpState = "yes" | "partial" | "none";

export type ProviderOps = {
  found: OpState;
  auth: OpState;
  usage: OpState;
  models: OpState;
  text: OpState;
  images: OpState;
  approvals: OpState;
  resume: OpState;
};

export type ProviderEntry = {
  id: string;
  name: string;
  kind: "Native client" | "Local runtime" | "API route";
  glyph: string;
  ops: ProviderOps;
  note: string;
};

const ops = (partial: Partial<ProviderOps>, found: OpState = "yes"): ProviderOps => ({
  found,
  auth: "none",
  usage: "none",
  models: "none",
  text: "none",
  images: "none",
  approvals: "none",
  resume: "none",
  ...partial,
});

/**
 * Condensed from docs/adapters/SUPPORT-MATRIX.md.
 * "partial" means implemented but conditional, fixture-tested only, or gated by a
 * capability the provider or account may not report — never an inferred yes.
 */
export const PROVIDERS: ProviderEntry[] = [
  {
    id: "claude-code",
    name: "Claude Code",
    kind: "Native client",
    glyph: "CC",
    ops: ops({ auth: "yes", usage: "partial", models: "yes", text: "yes", images: "yes", approvals: "yes" }),
    note: "Plan and quota are only shown when the native client reports them. Image turns are live-proven; native approvals travel to the host.",
  },
  {
    id: "codex",
    name: "Codex",
    kind: "Native client",
    glyph: "CX",
    ops: ops({ auth: "yes", usage: "partial", models: "yes", text: "yes", images: "partial", approvals: "yes" }),
    note: "Reads native app-server account, catalog and usage fields. Unreported values stay unknown instead of being estimated.",
  },
  {
    id: "open-code",
    name: "OpenCode",
    kind: "Native client",
    glyph: "OC",
    ops: ops({ auth: "yes", usage: "partial", models: "yes", text: "yes" }),
    note: "Models are filtered to providers OpenCode reports as connected. Text only; tools are denied by the adapter.",
  },
  {
    id: "grok",
    name: "Grok",
    kind: "Native client",
    glyph: "GR",
    ops: ops({ auth: "yes", models: "yes", text: "yes", images: "partial", approvals: "yes", resume: "yes" }),
    note: "Requires an explicit native profile. Every launch removes native tools. Resume only when the ACP peer advertises loadSession.",
  },
  {
    id: "gemini-cli",
    name: "Gemini CLI",
    kind: "Native client",
    glyph: "GM",
    ops: ops({ auth: "partial", models: "yes", text: "partial", images: "partial", approvals: "yes", resume: "yes" }),
    note: "Text-only sessions are refused because its read tools cannot be disabled. Availability depends on a compatible ACP client.",
  },
  {
    id: "antigravity",
    name: "Antigravity",
    kind: "Native client",
    glyph: "AG",
    ops: ops({ auth: "partial", models: "yes", text: "partial", images: "partial", approvals: "yes", resume: "yes" }),
    note: "Needs a dedicated ACP runtime profile; the IDE launcher is rejected. Text-only sessions are restricted like Gemini CLI.",
  },
  {
    id: "muse",
    name: "Muse Code",
    kind: "Native client",
    glyph: "MU",
    ops: ops({ auth: "yes", usage: "partial", models: "yes", text: "yes", images: "partial", approvals: "yes", resume: "yes" }),
    note: "Uses the experimental MSP surface. Plan windows appear only after Muse has observed usage; per-model vision is not reported.",
  },
  {
    id: "cursor",
    name: "Cursor",
    kind: "Native client",
    glyph: "CU",
    ops: ops({ auth: "yes", usage: "partial", models: "yes" }),
    note: "Metadata only. Included usage needs the separate --allow-usage cursor grant; execution is unsupported.",
  },
  {
    id: "copilot",
    name: "GitHub Copilot",
    kind: "Native client",
    glyph: "GH",
    ops: ops({}),
    note: "Passive detection of ~/.copilot only. Session databases are not opened and nothing is executed.",
  },
  {
    id: "pi",
    name: "Pi",
    kind: "Native client",
    glyph: "PI",
    ops: ops({}),
    note: "Passive detection from the pi command and its agent directory. No session files are opened.",
  },
  {
    id: "fx",
    name: "fx",
    kind: "Native client",
    glyph: "FX",
    ops: ops({}),
    note: "Passive detection from the fx command and ~/.fx. The binary is never launched.",
  },
  {
    id: "ollama",
    name: "Ollama",
    kind: "Local runtime",
    glyph: "OL",
    ops: ops({ models: "yes", text: "yes", images: "partial" }),
    note: "Reachable local endpoint only; no network scanning and no model loading during discovery. Vision needs a model that reports it.",
  },
  {
    id: "lm-studio",
    name: "LM Studio",
    kind: "Local runtime",
    glyph: "LM",
    ops: ops({ models: "yes", text: "yes", images: "partial" }),
    note: "Local model metadata only. Embedding-only models are not offered as chat routes.",
  },
  {
    id: "openai-api",
    name: "OpenAI API",
    kind: "API route",
    glyph: "OA",
    ops: ops({ auth: "partial", models: "yes", text: "yes" }, "none"),
    note: "Explicit route configuration. Metered billing needs separate permission and never stands in for a subscription route.",
  },
  {
    id: "anthropic-api",
    name: "Anthropic API",
    kind: "API route",
    glyph: "AN",
    ops: ops({ auth: "partial", models: "yes", text: "yes" }, "none"),
    note: "A separate metered route. No Claude subscription token is ever exchanged for API access.",
  },
  {
    id: "gemini-api",
    name: "Gemini API",
    kind: "API route",
    glyph: "GA",
    ops: ops({ auth: "partial", models: "yes", text: "yes" }, "none"),
    note: "Separate from Gemini CLI and Antigravity profiles. Catalog capability evidence decides what can run.",
  },
  {
    id: "xai-api",
    name: "xAI API",
    kind: "API route",
    glyph: "XA",
    ops: ops({ auth: "partial", models: "yes", text: "yes" }, "none"),
    note: "Separate from native Grok profiles. Metered usage is only shown when the route reports it.",
  },
  {
    id: "compatible-api",
    name: "Compatible API",
    kind: "API route",
    glyph: "CA",
    ops: ops({ auth: "partial", models: "partial", text: "yes" }, "none"),
    note: "Your endpoint, your metadata. No model-name heuristics, assumed prices or silent fallback.",
  },
];

export const OP_LABELS: { key: keyof ProviderOps; label: string }[] = [
  { key: "found", label: "Found" },
  { key: "auth", label: "Auth" },
  { key: "usage", label: "Usage" },
  { key: "models", label: "Models" },
  { key: "text", label: "Text" },
  { key: "images", label: "Vision" },
  { key: "approvals", label: "Approve" },
  { key: "resume", label: "Resume" },
];
