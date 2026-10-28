import type { Candidate, Catalog, Selection } from '@originlm/selector-core';
import type { LocalAIClient, CostReport, Snapshot, Event as SessionEvent } from 'originlm';

export type Page = 'Overview' | 'Connections' | 'Models' | 'Usage' | 'Selector lab' | 'Playground' | 'Diagnostics' | 'Settings';

export type Mode = 'synthetic' | 'live';

export type VariantId = 'rail' | 'compact' | 'palette' | 'capability' | 'compare' | 'allowance' | 'preset' | 'mobile';

export type SessionStatus =
  | 'Ready'
  | 'Starting'
  | 'Running'
  | 'Awaiting approval'
  | 'Completed'
  | 'Cancelled'
  | 'Denied'
  | 'Error'
  | 'Suspended';

export interface ProbeResult {
  ok: boolean;
  message: string;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  text: string;
  /** Assistant lifecycle: Running while streaming, then a terminal status. */
  status: string;
}

export interface DebugError {
  code: string;
  message: string;
}

export interface ApprovalRequest {
  approvalId: string;
  action: string;
  detail: string;
}

export interface FixtureFlags {
  staleAuth?: boolean;
  extraProfiles?: boolean;
  forceMode?: Mode;
  probeResults?: boolean;
}

export interface DemoFactory {
  connectFixture: () => Promise<LocalAIClient>;
  connectLive: (endpoint: string, token: string) => Promise<LocalAIClient>;
  initialClient?: LocalAIClient | null;
  initialMode?: Mode;
  /** Test observation point: called when a bulk metadata check starts (wired by the scripted fixture host only). */
  onBulkProbe?: () => void;
  /** Offered while showing example data: leave it and pair a native runtime (real accounts). */
  pair?: () => void;
  /** Route change requested by an action (for example after a live scan). */
  navigate?: (page: Page) => void;
  /** Scripted flags for automated checks (fixture bootstrap only). */
  fixtureFlags?: FixtureFlags;
}

export interface DemoState {
  variant: VariantId;
  mode: Mode;
  snapshot: Snapshot | null;
  catalog: Catalog | null;
  selection: Selection | null;
  provider: string;
  query: string;
  required: string[];
  onlyEligible: boolean;
  allowPaid: boolean;
  favorites: string[];
  sensitiveConsent: boolean;
  mobileStep: number;
  mobileProvider: string;
  mobileConnection: string | null;
  playgroundProviderId: string | null;
  hasClient: boolean;
  handshake: LocalAIClient['handshake'] | null;
  events: SessionEvent[];
  sessionId: string | null;
  nativeResumption: boolean;
  resumeToken: string | null;
  turnId: string | null;
  sessionStatus: SessionStatus;
  chat: ChatMessage[];
  debugError: DebugError | null;
  approval: ApprovalRequest | null;
  grantConsent: boolean;
  simulateApproval: boolean;
  workspaceId: string;
  prompt: string;
  /** Connection ids whose metadata check is in flight (providers are checked concurrently). */
  probing: string[];
  probeResults: Record<string, ProbeResult>;
  metadataBatch: { completed: number; total: number } | null;
  notice: string;
  connectionError: string | null;
  diagnostics: unknown;
  costs: CostReport | null;
  costError: string | null;
  savingPrices: boolean;
  detectedExpanded: boolean;
  paletteOpen: boolean;
  connectOpen: boolean;
  /** True while showing example data that the person can replace by pairing a runtime. */
  canPair: boolean;
}

export type { Candidate };
