/* The single source of copy for the five concepts. Every claim here is lifted from
   README.md, docs/integration/quickstarts.md, docs/integration/shared-broker.md,
   docs/design/ADR-003-v1-product-boundaries.md or apps/site/src/data/*.ts. Concepts may
   rephrase for voice, but must not add capabilities, numbers or testimonials.

   Rules carried from ADR-003: never say "nothing leaves the machine" without limiting it
   to the local control plane; prompts go to the provider the selected route names. */
export { PROVIDERS, OP_LABELS } from "../../data/providers";
export type { ProviderEntry, OpState } from "../../data/providers";

export const VERSION = "0.1.0-alpha.25";
export const REPO = "https://github.com/thomaslittle/originlm";
export const DOCS = {
  quickstarts: `${REPO}/blob/main/docs/integration/quickstarts.md`,
  install: `${REPO}/blob/main/docs/integration/install.md`,
  broker: `${REPO}/blob/main/docs/integration/shared-broker.md`,
  matrix: `${REPO}/blob/main/docs/adapters/SUPPORT-MATRIX.md`,
  threat: `${REPO}/blob/main/docs/security/threat-model.md`,
  security: `${REPO}/blob/main/SECURITY.md`,
  privacy: `${REPO}/blob/main/docs/design/ADR-003-v1-product-boundaries.md`,
  license: `${REPO}/blob/main/LICENSE`,
};

/* Verified against docs/integration/quickstarts.md (checked 2026-11-04 there): npm's
   `latest` tag still points at an older alpha, so npm installs use @alpha. */
export const INSTALL = [
  { id: "npm", label: "npm", cmd: "npm install originlm@alpha" },
  { id: "pip", label: "pip", cmd: "pip install originlm==0.1.0a25" },
  { id: "cargo", label: "cargo", cmd: "cargo add originlm@=0.1.0-alpha.25" },
] as const;
export const STATUS = "Published alpha · MIT";
export const STATUS_LONG = "Published alpha on npm (the alpha tag), PyPI and crates.io at 0.1.0-alpha.25. The API can change between alpha revisions. MIT licensed.";

export const TAGLINE = "See your AI. Build with it.";
export const ONE_LINER = "A local control center for the AI you already have, and the developer platform that lets your applications use it.";

/* The positioning every concept leads with. */
export const PITCH = {
  headline: "Your own apps can call the models you already pay for.",
  body: "OriginLM is one local runtime that makes the AI already on your machine callable from software you build. One integration, instead of client-specific paths, metadata parsers and streaming protocols for every AI client.",
};

/* README: "Your application does not have to reproduce client-specific paths, metadata
   parsers, or streaming protocols for each AI client a user might have installed." */
export const PROBLEM = {
  line: "Every app you build re-integrates every AI client.",
  burdens: ["client-specific paths", "metadata parsers", "streaming protocols"],
  fix: "Your application does not have to reproduce client-specific paths, metadata parsers or streaming protocols for each AI client a user might have installed.",
};

/* README "Passive discovery" list, in its order. */
export const DISCOVERED = [
  "Codex", "Claude Code", "Grok", "Gemini CLI", "Antigravity", "OpenCode", "Muse Code",
  "Kiro", "Pi", "Cursor", "GitHub Copilot", "Ollama", "LM Studio",
] as const;
export const DISCOVERED_TAIL = "and API routes you configure";

export const TOOLKIT = [
  ["Embedded library", "The Rust facade embeds the runtime in-process when no helper exists."],
  ["Native helper", "The standalone originlm executable: stdio, HTTP/SSE, broker and Native Messaging transports."],
  ["Shared broker", "One local service many apps attach to, each with its own token and identity."],
  ["TypeScript & Python SDKs", "Typed protocol clients generated from the Rust contracts."],
  ["React hooks & selectors", "Optional hooks and eight selector layouts."],
] as const;

export const HOW = [
  {
    id: "discover", verb: "Discover", method: "snapshot.get",
    body: "Passive discovery finds the AI clients already installed, from metadata only. No secrets read, no network calls, nothing at import time.",
    detail: "Provider, installation, profile, account, connection, route, model, billing and quota-pool identities stay distinct. Unknown and unsupported are different states; nothing is guessed.",
  },
  {
    id: "grant", verb: "Grant", method: "grants.request",
    body: "Discovery is not permission. The host app grants execution explicitly, scoped to connections and workspaces. grants.request only asks: it returns requires_host_approval and changes nothing.",
    detail: "Metadata, execution, workspace access, tools and paid options are separate, revocable, per-app grants.",
  },
  {
    id: "call", verb: "Call", method: "turns.start",
    body: "Resolve a route by capability, open a session, stream the turn. Typed events from a cursor: text deltas, approvals, usage, final.",
    detail: "Native model IDs and option descriptors are preserved. There is no fake universal Low/Medium/High.",
  },
] as const;

export const SESSION_FEATURES = [
  "Streaming text", "Approvals", "Cancellation", "Image input",
  "Host MCP tools (read-only by default)", "Resumable sessions", "Scoped artifacts",
] as const;

export const SAFETY = [
  ["Finding is not permission", "Finding an installation does not establish an active subscription, and a discovery signal is never permission to execute."],
  ["The client keeps its credentials", "The installed client owns its credentials. Subscription tokens are never exported as API keys, and credentials are never returned to consuming apps."],
  ["No silent paid fallback", "Nothing silently falls back to paid usage or rotates accounts to evade limits. allowPaid is off by default."],
  ["Separate, revocable grants", "Metadata, execution, workspace access, tools and paid options are separate, revocable, per-application grants."],
  ["No telemetry, no relay", "History and grants stay on your machine. Authorized operations go to the provider the selected route names; local model routes can run locally."],
] as const;

export const GRANT_SCOPES = [
  { id: "metadata", label: "Metadata", field: "allowPassiveDiscovery", note: "Read the snapshot, models and usage." },
  { id: "execution", label: "Execution", field: "execution.providerIds", note: "Run turns on the named provider." },
  { id: "workspace", label: "Workspace", field: "execution.workspaces", note: "A named, absolute workspace." },
  { id: "tools", label: "Tools", field: "execution.allowNativePolicy", note: "Native CLI tools, granted separately." },
  { id: "paid", label: "Paid", field: "execution.allowPaid", note: "Metered or prepaid routes. Off by default." },
] as const;

export const CONTROL_CENTER = {
  line: "The control center: a native desktop app and a terminal UI over the same public API.",
  views: ["Overview", "Providers", "Models", "Usage charts", "Connected apps", "Sessions", "Playground", "Diagnostics"],
  note: "It shows quota windows only as the client reports them. Unknown stays unknown, never a guessed zero.",
  run: "cargo run -p originlm-desktop",
};

/* Example routes for interactive demos. Model ids are examples and are labelled as such
   wherever they appear; the query string is matched by selection.resolve against the
   model name, native id, provider id and route/connection display names
   (crates/originlm-core/src/selection.rs). */
export type DemoRoute = { id: string; name: string; providerId: string; query: string; model: string; kind: "subscription" | "local"; reply: string };
export const DEMO_ROUTES: DemoRoute[] = [
  { id: "codex", name: "Codex", providerId: "codex", query: "codex", model: "gpt-5-codex", kind: "subscription", reply: "Three notes mention the launch. Two are drafts; one has a date: Thursday." },
  { id: "claude", name: "Claude Code", providerId: "claude_code", query: "claude", model: "sonnet", kind: "subscription", reply: "Found 4 TODOs across 2 files. The oldest is in sync.ts, line 88." },
  { id: "gemini", name: "Gemini CLI", providerId: "gemini_cli", query: "gemini", model: "gemini-2.5-pro", kind: "subscription", reply: "The meeting notes list two owners for the same task. Pick one?" },
  { id: "ollama", name: "Ollama", providerId: "ollama", query: "ollama", model: "llama3.2", kind: "local", reply: "Summary: the photo set has 212 images, 31 near-duplicates." },
];

/* The real-shaped call, from apps/site/src/sections/Sdk.tsx and the generated SDK types
   (Requirements.query, SessionCreateParams, TurnStartParams). */
export function tsCall(route: DemoRoute, prompt = "Summarize the open questions in these notes."): string {
  return `import { createLocalAI } from "originlm";

const ai = await createLocalAI();
const [pick] = await ai.selection.resolve({
  query: "${route.query}",
  requiredCapabilities: ["generation.text"],
  eligibleOnly: true,
});
const session = await ai.sessions.create({
  selection: pick.selection,
  workspaceId: "notes",
  permissionMode: "read_only",
});
const events = ai.events.subscribe({ sessionId: session.id });
await ai.turns.start({
  sessionId: session.id,
  requestId: crypto.randomUUID(),
  prompt: "${prompt}",
});
for await (const { payload } of events) {
  if (payload.type === "text_delta") render(payload.text);
  if (payload.type === "final") break;
}`;
}

/* docs/integration/quickstarts.md, verbatim. */
export const QUICKSTART = {
  node: `import { createLocalAI } from 'originlm';
const ai = await createLocalAI();
try {
  const snapshot = await ai.snapshot.get();
  console.log(ai.mode, snapshot.revision);
} finally {
  await ai.close();
}`,
  python: `from originlm import Client
with Client.auto() as ai:
    print(ai.mode, ai.snapshot.get()['revision'])`,
  rust: `fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ai = originlm::LocalAi::start()?;
    let snapshot = ai.snapshot()?;
    println!("{:?}: {}", ai.mode(), snapshot.revision);
    Ok(())
}`,
};

/* A shared-broker grants manifest entry (docs/integration/shared-broker.md). */
export function grantManifest(opts: { app: string; providerId: string; scopes: Record<string, boolean> }): string {
  const { app, providerId, scopes } = opts;
  const entry: Record<string, unknown> = {
    id: app,
    tokenFile: `/home/me/.config/${app}/originlm.token`,
    allowPassiveDiscovery: !!scopes.metadata,
  };
  if (scopes.metadata) entry.probeProviderIds = [providerId];
  if (scopes.execution) {
    const execution: Record<string, unknown> = { providerIds: [providerId] };
    if (scopes.workspace) execution.workspaces = { default: `/home/me/${app}` };
    execution.allowPaid = !!scopes.paid;
    if (scopes.tools) execution.allowNativePolicy = true;
    entry.execution = execution;
  }
  return JSON.stringify({ schemaVersion: 1, apps: [entry] }, null, 2);
}
