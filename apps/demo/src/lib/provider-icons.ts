// Provider brand marks. Source, licenses and modifications are recorded in
// src/assets/providers/README.md. Marks are trademarks of their owners and are
// used only to identify the provider. Unknown ids return undefined so callers
// can render their own neutral fallback.
import anthropic from "../assets/providers/anthropic.svg";
import antigravity from "../assets/providers/antigravity.svg";
import claude from "../assets/providers/claude.svg";
import claudeCode from "../assets/providers/claude-code.svg";
import codex from "../assets/providers/codex.svg";
import gemini from "../assets/providers/gemini.svg";
import grok from "../assets/providers/grok.svg";
import lmstudio from "../assets/providers/lmstudio.svg";
import local from "../assets/providers/local.svg";
import ollama from "../assets/providers/ollama.svg";
import openai from "../assets/providers/openai.svg";
import opencode from "../assets/providers/opencode.svg";
import xai from "../assets/providers/xai.svg";

const ICON_URLS: Readonly<Record<string, string>> = {
  anthropic,
  antigravity,
  claude,
  claude_code: claudeCode,
  codex,
  gemini,
  gemini_cli: gemini,
  grok,
  lm_studio: lmstudio,
  lmstudio,
  local,
  ollama,
  openai,
  opencode,
  xai,
};

export const PROVIDER_NAMES: Record<string, string> = {
  anthropic: "Anthropic",
  antigravity: "Antigravity",
  claude: "Claude",
  claude_code: "Claude Code",
  codex: "Codex",
  gemini: "Gemini",
  gemini_cli: "Gemini CLI",
  grok: "Grok",
  lm_studio: "LM Studio",
  lmstudio: "LM Studio",
  local: "Local runtime",
  ollama: "Ollama",
  openai: "OpenAI",
  opencode: "OpenCode",
  xai: "xAI",
};

function normalize(providerId: string): string {
  return providerId.trim().toLowerCase().replace(/[\s-]+/g, "_");
}

/** Vite asset URL of the monochrome mark for a provider id, or undefined if none is known. */
export function providerIconUrl(providerId: string): string | undefined {
  const key = normalize(providerId);
  return Object.prototype.hasOwnProperty.call(ICON_URLS, key) ? ICON_URLS[key] : undefined;
}

/** Readable product name for a provider id, preferring the catalog's own name. */
export function providerName(providerId: string, catalogName?: string): string {
  return catalogName || PROVIDER_NAMES[providerId] || providerId;
}
