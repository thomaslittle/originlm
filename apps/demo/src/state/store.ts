import { createStore, type StoreApi } from 'zustand/vanilla';
import {
  createCatalog,
  reconcileSelection,
  resolveCandidates,
  type Candidate,
  type OptionView,
  type Selection,
  type SnapshotView,
} from '@originlm/selector-core';
import {
  readArtifactBytes,
  readCatalogSnapshot,
  type Event as SessionEvent,
  type LocalAIClient,
  type SessionCreateParams,
  type Snapshot,
  type ModelPrice,
  type TurnStartParams,
} from 'originlm';
import { validateOption } from '@originlm/selector-core';
import { filterLiveCatalog } from '../../lib/catalog_visibility.mjs';
import { debugSessionError, userFacingError } from '../../lib/errors.mjs';
import { applyFixtureFlags } from './fixture-flags';
import { markAssistantStatus, newMessageId, reduceEvents } from './events';
import type { ChatMessage, DemoFactory, DemoState, Mode, ProbeResult } from './types';

const BUSY = new Set<DemoState['sessionStatus']>(['Starting', 'Running', 'Awaiting approval']);
/** Text deltas are coalesced to roughly one commit per frame. */
const STREAM_FLUSH_MS = 16;

export const initialState: DemoState = {
  variant: 'rail',
  mode: 'synthetic',
  snapshot: null,
  catalog: null,
  selection: null,
  provider: 'codex',
  query: '',
  required: [],
  onlyEligible: false,
  allowPaid: false,
  favorites: [],
  sensitiveConsent: false,
  mobileStep: 0,
  mobileProvider: 'codex',
  mobileConnection: null,
  playgroundProviderId: null,
  hasClient: false,
  handshake: null,
  events: [],
  sessionId: null,
  nativeResumption: false,
  resumeToken: null,
  turnId: null,
  sessionStatus: 'Ready',
  chat: [],
  debugError: null,
  approval: null,
  grantConsent: false,
  simulateApproval: false,
  workspaceId: 'default',
  prompt: '',
  probing: [],
  probeResults: {},
  metadataBatch: null,
  notice: '',
  connectionError: null,
  diagnostics: null,
  costs: null,
  costError: null,
  savingPrices: false,
  detectedExpanded: true,
  paletteOpen: false,
  connectOpen: false,
  canPair: false,
};

export interface DemoActions {
  patch: (update: Partial<DemoState>) => void;
  select: (candidate: Candidate) => void;
  toggleFavorite: (key: string) => void;
  setOption: (descriptor: OptionView, value: unknown) => void;
  startTurn: () => Promise<void>;
  cancelTurn: () => Promise<void>;
  suspendSession: () => Promise<void>;
  resumeSession: () => Promise<void>;
  answerApproval: (allow: boolean) => Promise<void>;
  scan: () => Promise<void>;
  probeConnection: (id: string) => Promise<boolean>;
  probeAllConnections: () => Promise<void>;
  revokeConnection: () => Promise<void>;
  refreshDiagnostics: () => Promise<void>;
  refreshCosts: () => Promise<void>;
  savePrices: (prices: ModelPrice[]) => Promise<boolean>;
  downloadArtifact: (event: SessionEvent) => Promise<void>;
  connectLive: (endpoint: string, token: string) => Promise<void>;
  /** Leave example data and pair a native runtime. */
  pair: () => void;
}

export interface DemoStore {
  store: StoreApi<DemoState>;
  actions: DemoActions;
  dispose: () => Promise<void>;
}

const asView = (snapshot: Snapshot): SnapshotView => snapshot as unknown as SnapshotView;

function assertSnapshotShape(snapshot: Snapshot | null | undefined): asserts snapshot is Snapshot {
  if (
    !snapshot ||
    !Array.isArray(snapshot.connections) ||
    !Array.isArray(snapshot.models) ||
    !Array.isArray(snapshot.routes) ||
    !Array.isArray(snapshot.providers)
  ) {
    throw new Error('INVALID_SNAPSHOT');
  }
}

/**
 * Framework-free host state. It owns the SDK client, stream subscription, and
 * epoch bookkeeping outside React so components stay pure and subscribe only to
 * the slices they render. No component effect boots or tears down the client.
 */
export function createDemoStore(factory: DemoFactory): DemoStore {
  const store = createStore<DemoState>(() => ({ ...initialState }));
  const { getState: get, setState: set } = store;
  const flags = factory.fixtureFlags;

  let client: LocalAIClient | null = null;
  let controller: AbortController | null = null;
  let epoch = 0;
  let closed = false;
  let sessionContext: string | null = null;

  let pending: SessionEvent[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;

  /** View choices that belong to the person, not to a connection: they survive reconnects and revokes. */
  const uiPreferences = (): Pick<DemoState, 'variant' | 'detectedExpanded' | 'paletteOpen' | 'connectOpen'> => {
    const { variant, detectedExpanded, paletteOpen, connectOpen } = get();
    return { variant, detectedExpanded, paletteOpen, connectOpen };
  };
  // Connecting a live runtime replaces the example data, so the pair offer only applies to synthetic mode.
  const canPair = (mode: Mode): boolean => mode === 'synthetic' && Boolean(factory.pair);
  const patch = (update: Partial<DemoState>): void => set(update);
  const notify = (notice: string): void => set({ notice });
  const stale = (current: number): boolean => closed || epoch !== current;

  function flushEvents(): void {
    if (flushTimer !== null) clearTimeout(flushTimer);
    flushTimer = null;
    if (!pending.length) return;
    const batch = pending;
    pending = [];
    set((state) => reduceEvents(state, batch));
  }

  function rebuild(snapshot: Snapshot, mode: Mode, allowModeMix = false): Snapshot {
    assertSnapshotShape(snapshot);
    if (!allowModeMix) {
      if (mode === 'live' && snapshot.mode !== 'live') throw new Error('SYNTHETIC_RUNTIME_REJECTED');
      if (mode === 'synthetic' && snapshot.mode !== 'synthetic') throw new Error('LIVE_RUNTIME_REJECTED');
    }
    const visible = mode === 'live' ? filterLiveCatalog(snapshot) : snapshot;
    const catalog = createCatalog(asView(visible));
    const current = get();
    let selection = current.selection;
    let notice = current.notice;
    if (selection) {
      const reconciled = reconcileSelection(catalog, selection);
      selection = reconciled.selection;
      if (reconciled.needsReview) notice = 'The catalog changed. Review the updated selection before use.';
    }
    const resolved = resolveCandidates(catalog, { allowPaidFallback: current.allowPaid });
    // No default selection: a route is only "selected" after a person picks it.
    const selected = resolved.find((candidate) => candidate.route.id === selection?.routeId && candidate.model.id === selection?.modelId);
    // The rail defaults to the first provider; when that provider is absent from
    // this catalog, fall back to All instead of an unexplained empty list.
    const known = visible.providers.some((provider) => provider.id === current.provider);
    set({
      snapshot: visible,
      catalog,
      selection,
      notice,
      provider: known || current.provider === 'all' || current.provider === 'favorites' ? current.provider : 'all',
      playgroundProviderId: selected ? selected.route.providerId : current.playgroundProviderId,
    });
    return visible;
  }

  /** One read path so scripted fixture flags survive every refresh. */
  async function loadSnapshot(active: LocalAIClient): Promise<Snapshot> {
    const snapshot = await readCatalogSnapshot(active);
    return flags && (flags.staleAuth || flags.extraProfiles) ? applyFixtureFlags(snapshot, flags) : snapshot;
  }

  async function stopClient(): Promise<void> {
    controller?.abort();
    controller = null;
    flushEvents();
    const active = client;
    client = null;
    if (active) await active.close();
  }

  function observe(active: LocalAIClient, current: number): void {
    const abort = new AbortController();
    controller = abort;
    void (async () => {
      try {
        for await (const event of active.events.subscribe({ after: '0' }, { signal: abort.signal })) {
          if (stale(current)) break;
          if (!event?.payload || typeof event.payload.type !== 'string') continue;
          pending.push(event);
          if (event.payload.type === 'text_delta') flushTimer ??= setTimeout(flushEvents, STREAM_FLUSH_MS);
          else flushEvents();
          if (event.payload.type === 'final') void refreshCosts();
        }
      } catch (error) {
        if (!stale(current) && !abort.signal.aborted) notify(userFacingError(error as { code?: unknown }));
      } finally {
        if (!stale(current)) flushEvents();
      }
    })();
  }

  async function operate(action: () => Promise<void>, includeDebug = false): Promise<void> {
    try {
      await action();
    } catch (error) {
      set((state) => ({
        notice: userFacingError(error as { code?: unknown }),
        debugError: includeDebug ? debugSessionError(error as { code?: unknown; message?: unknown }) : state.debugError,
      }));
    }
  }

  async function scan(): Promise<void> {
    const active = client;
    if (!active) return;
    await operate(async () => {
      await active.discovery.refresh({ level: 'passive' });
      rebuild(await loadSnapshot(active), get().mode, Boolean(flags?.forceMode));
      factory.navigate?.('Connections');
    });
  }

  function describeCheck(snapshot: Snapshot, id: string, prefix: string): ProbeResult {
    const connection = snapshot.connections.find((item) => item.id === id);
    const routeIds = new Set(snapshot.routes.filter((route) => route.connectionId === id).map((route) => route.id));
    const modelCount = snapshot.models.filter((model) => routeIds.has(model.routeId)).length;
    const status = connection?.auth.state === 'known' ? connection.auth.value : 'authentication not reported';
    return { ok: true, message: `${prefix} \u00b7 ${status} \u00b7 ${modelCount} model${modelCount === 1 ? '' : 's'} reported` };
  }

  /**
   * Re-read the catalog after a probe. Single-flight: concurrent probe completions share one read, and a
   * completion that lands mid-read schedules exactly one more, so the UI updates per probe without a
   * full catalog read (about 150 ms for a few hundred models) for every probe.
   */
  let catalogRead: Promise<Snapshot> | null = null;
  let catalogDirty = false;
  function refreshCatalog(active: LocalAIClient, current: number): Promise<Snapshot> {
    if (catalogRead) {
      catalogDirty = true;
      return catalogRead;
    }
    catalogRead = (async () => {
      let snapshot: Snapshot;
      do {
        catalogDirty = false;
        snapshot = rebuild(await loadSnapshot(active), get().mode, Boolean(flags?.forceMode));
      } while (catalogDirty && !stale(current));
      return snapshot;
    })().finally(() => {
      catalogRead = null;
    });
    return catalogRead;
  }

  async function probeConnection(id: string): Promise<boolean> {
    const active = client;
    if (!active || get().probing.includes(id)) return false;
    const current = epoch;
    set((state) => ({ notice: '', probing: [...state.probing, id] }));
    try {
      await active.connections.probe({ connectionId: id });
      if (stale(current)) return false;
      const snapshot = await refreshCatalog(active, current);
      set((state) => ({ probeResults: { ...state.probeResults, [id]: describeCheck(snapshot, id, 'Metadata check complete') } }));
      return true;
    } catch (error) {
      if (!stale(current)) {
        const reason = userFacingError(error as { code?: unknown });
        set((state) => ({
          probeResults: { ...state.probeResults, [id]: { ok: false, message: `Metadata check unavailable \u00b7 ${reason}` } },
        }));
      }
      return false;
    } finally {
      if (!stale(current)) set((state) => ({ probing: state.probing.filter((item) => item !== id) }));
    }
  }

  /**
   * Check every detected profile. Different providers run concurrently (they share nothing), but the
   * profiles of ONE provider run in order: a provider's status check can refresh its own credentials,
   * and two checks must never race that refresh. Results appear as each one finishes.
   */
  async function probeAllConnections(): Promise<void> {
    const active = client;
    if (!active || get().probing.length || get().metadataBatch) return;
    factory.onBulkProbe?.();
    const current = epoch;
    const connections = get().snapshot?.connections ?? [];
    if (!connections.length) return;
    // The helper keeps the last observation of every profile and marks it fresh for 15 minutes (it may also
    // have warmed them at startup). A fresh observation is shown as is; only the rest is probed.
    const snapshotNow = get().snapshot;
    const isFresh = (connection: Snapshot['connections'][number]) => connection.auth.state === 'known' && connection.auth.freshness === 'fresh';
    const alreadyCurrent = connections.filter(isFresh);
    const needCheck = connections.filter((connection) => !isFresh(connection));
    if (snapshotNow && alreadyCurrent.length) {
      set((state) => ({
        probeResults: {
          ...state.probeResults,
          ...Object.fromEntries(alreadyCurrent.map((connection) => [connection.id, describeCheck(snapshotNow, connection.id, 'Already current')])),
        },
      }));
    }
    if (!needCheck.length) return;
    const byProvider = new Map<string, string[]>();
    for (const connection of needCheck) byProvider.set(connection.providerId, [...(byProvider.get(connection.providerId) ?? []), connection.id]);
    set({ notice: '', metadataBatch: { completed: 0, total: needCheck.length } });
    await Promise.all(
      [...byProvider.values()].map(async (ids) => {
        for (const id of ids) {
          if (stale(current) || !client) return;
          // Sequential within one provider on purpose (see above).
          // react-doctor-disable-next-line react-doctor/async-await-in-loop
          await probeConnection(id);
          if (stale(current)) return;
          set((state) => ({ metadataBatch: state.metadataBatch && { ...state.metadataBatch, completed: state.metadataBatch.completed + 1 } }));
        }
      }),
    );
    if (!stale(current)) set({ metadataBatch: null });
  }

  async function attach(create: () => Promise<LocalAIClient>, mode: Mode): Promise<void> {
    const current = ++epoch;
    await stopClient();
    sessionContext = null;
    pending = [];
    catalogRead = null;
    catalogDirty = false;
    set({ ...initialState, ...uiPreferences(), mode: flags?.forceMode ?? mode, canPair: canPair(flags?.forceMode ?? mode) });
    let opened: LocalAIClient | undefined;
    try {
      opened = await create();
      if (stale(current)) {
        await opened.close();
        return;
      }
      const snapshot = await loadSnapshot(opened);
      if (flags?.probeResults) {
        const probeResults: DemoState['probeResults'] = {};
        for (const connection of snapshot.connections) {
          probeResults[connection.id] = { ok: false, message: 'Metadata check unavailable · provider runtime unavailable' };
        }
        set({ probeResults });
      }
      if (stale(current)) {
        await opened.close();
        return;
      }
      rebuild(snapshot, flags?.forceMode ?? mode, Boolean(flags?.forceMode));
      client = opened;
      set({ hasClient: true, handshake: opened.handshake });
      void refreshCosts();
      if (opened.handshake.methods.includes('events.subscribe')) observe(opened, current);
      // Scripted fixture flags arrange their own snapshot; automated checks drive
      // metadata actions explicitly instead of auto-probing here.
      if ((flags?.forceMode ?? mode) === 'live' && !flags) {
        await scan();
        if (!stale(current) && client) await probeAllConnections();
      }
    } catch {
      await opened?.close().catch(() => undefined);
      // A newer attach owns the shared client and stream now; only the current attach may tear them down.
      if (epoch === current) {
        client = null;
        controller?.abort();
        controller = null;
        set({ hasClient: false, handshake: null, connectionError: 'Unable to connect. No account or model data has been substituted.' });
      }
    }
  }

  async function startTurn(): Promise<void> {
    const current = get();
    if (BUSY.has(current.sessionStatus)) return;
    if (!current.selection) return notify('Choose a model and account before starting a turn.');
    if (!current.grantConsent) return notify('Allow this session with the checkbox below the prompt before sending.');
    const active = client;
    if (!active) return;
    if (!active.handshake.methods.includes('turns.start')) {
      return notify('This runtime allows metadata checks only. Enable execution in the host application before sending a prompt.');
    }
    const startEpoch = epoch;
    const selection = structuredClone(current.selection);
    const previousStatus = current.sessionStatus;
    const context = JSON.stringify({ selection, workspaceId: current.workspaceId, allowPaid: current.allowPaid, mode: current.mode });
    const promptText = current.prompt;
    const userMessage: ChatMessage | null = promptText.trim()
      ? { id: newMessageId('user'), role: 'user', text: promptText, status: 'sent' }
      : null;
    const pendingAssistant: ChatMessage | null = userMessage ? { id: newMessageId('assistant'), role: 'assistant', text: '', status: 'Running' } : null;
    const withTurn = (chat: ChatMessage[]): ChatMessage[] => [...chat, ...(userMessage ? [userMessage] : []), ...(pendingAssistant ? [pendingAssistant] : [])];
    set({ sessionStatus: 'Starting', debugError: null, approval: null, notice: '', chat: withTurn(current.chat) });
    try {
      const catalog = get().catalog;
      const resolved = catalog ? resolveCandidates(catalog, { allowPaidFallback: get().allowPaid }) : [];
      const model = resolved.find((candidate) => candidate.route.id === selection.routeId && candidate.model.id === selection.modelId);
      if (!model) throw Object.assign(new Error('SELECTION_STALE'), { code: 'SELECTION_STALE' });
      const latest = get();
      const reuse = latest.sessionId && sessionContext === context && (previousStatus === 'Ready' || previousStatus === 'Completed');
      let session;
      if (reuse && latest.sessionId) {
        session = await active.sessions.get({ sessionId: latest.sessionId });
      } else {
        if (latest.sessionId) await active.sessions.close({ sessionId: latest.sessionId });
        if (latest.mode === 'synthetic') {
          await active.grants.request({ connectionIds: [model.connection.id], scopes: ['execution'], allowPaid: latest.allowPaid });
        }
        // Live sessions bind a host-granted workspace and stay read-only; synthetic ones need neither.
        session = await active.sessions.create({
          selection,
          allowPaid: latest.allowPaid,
          ...(latest.mode === 'live' ? { workspaceId: latest.workspaceId, permissionMode: 'read_only' } : {}),
        } as SessionCreateParams);
        sessionContext = context;
        // A fresh session starts a fresh visible thread.
        set({ chat: withTurn([]) });
      }
      if (epoch !== startEpoch) return;
      set({ sessionId: session.id, nativeResumption: Boolean(session.nativeResumption), resumeToken: null, turnId: null, sessionStatus: 'Running' });
      const turn = await active.turns.start({
        sessionId: session.id,
        prompt: promptText,
        ...(latest.mode === 'synthetic' ? { simulateApproval: get().simulateApproval } : { requestId: crypto.randomUUID() }),
      } as TurnStartParams);
      if (epoch !== startEpoch) return;
      set({ turnId: turn.turnId });
    } catch (error) {
      if (epoch !== startEpoch) return;
      set((state) => ({
        notice: userFacingError(error as { code?: unknown }),
        debugError: debugSessionError(error as { code?: unknown; message?: unknown }),
        sessionStatus: 'Error',
        chat: markAssistantStatus(state.chat, 'Error'),
      }));
    }
  }

  async function cancelTurn(): Promise<void> {
    const { turnId } = get();
    if (!turnId || !client) return;
    const active = client;
    await operate(async () => {
      await active.turns.cancel({ turnId });
    });
  }

  async function suspendSession(): Promise<void> {
    const { sessionId, nativeResumption } = get();
    const active = client;
    if (!active || !sessionId || !nativeResumption || !active.handshake.methods.includes('sessions.suspend')) return;
    await operate(async () => {
      const result = await active.sessions.suspend({ sessionId });
      set({ resumeToken: result.resumeToken, sessionId: null, turnId: null, sessionStatus: 'Suspended', approval: null });
    });
  }

  async function resumeSession(): Promise<void> {
    const { resumeToken } = get();
    const active = client;
    if (!active || !resumeToken || !active.handshake.methods.includes('sessions.resume')) return;
    await operate(async () => {
      const session = await active.sessions.resume({ resumeToken });
      set({ sessionId: session.id, nativeResumption: Boolean(session.nativeResumption), resumeToken: null, turnId: null, sessionStatus: 'Ready', approval: null });
    });
  }

  async function answerApproval(allow: boolean): Promise<void> {
    const { approval } = get();
    const active = client;
    if (!approval || !active) return;
    await operate(async () => {
      await active.approvals.respond({ approvalId: approval.approvalId, allow });
    });
  }

  async function revokeConnection(): Promise<void> {
    const active = client;
    if (!active) return;
    await operate(async () => {
      await active.grants.revoke({});
      await stopClient();
      const { mode } = get();
      set({ ...initialState, ...uiPreferences(), mode, notice: 'Connection revoked. Restart the native host and reload to pair again.' });
    });
  }

  async function refreshCosts(): Promise<void> {
    const active = client;
    const current = epoch;
    if (!active?.handshake.methods.includes('costs.read')) {
      set({ costs: null, costError: 'This runtime does not report cost evidence yet.' });
      return;
    }
    try {
      const costs = await active.costs.read();
      if (!stale(current)) set({ costs, costError: null });
    } catch {
      if (!stale(current)) set({ costError: 'Cost evidence could not be refreshed. Any displayed observations are from the previous read.' });
    }
  }

  async function savePrices(prices: ModelPrice[]): Promise<boolean> {
    const active = client;
    const current = epoch;
    if (!active || get().savingPrices || !get().costs?.canEditPrices) return false;
    set({ savingPrices: true, costError: null });
    try {
      await active.costs.setPrices({ prices });
      if (stale(current)) return false;
      await refreshCosts();
      return !get().costError;
    } catch {
      if (!stale(current)) set({ costError: 'Prices were not saved. Check the values and your runtime administrator access.' });
      return false;
    } finally {
      if (!stale(current)) set({ savingPrices: false });
    }
  }

  async function refreshDiagnostics(): Promise<void> {
    const active = client;
    if (!active) return;
    await operate(async () => {
      const [report, adapters, grants] = await Promise.all([active.diagnostics.report(), active.adapters.list(), active.grants.list()]);
      set({ diagnostics: { report, adapters, grants } });
    });
  }

  async function downloadArtifact(event: SessionEvent): Promise<void> {
    const active = client;
    if (!active || event.sessionId !== get().sessionId || event.payload.type !== 'artifact') return;
    if (!active.handshake.methods.includes('artifacts.read')) return;
    const startEpoch = epoch;
    const { artifactId } = event.payload;
    await operate(async () => {
      const result = await readArtifactBytes(active, event.sessionId, artifactId);
      if (stale(startEpoch)) return;
      // Always download as inert bytes; never navigate to or render a provider's executable content.
      const url = URL.createObjectURL(new Blob([result.bytes as BlobPart], { type: 'application/octet-stream' }));
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = result.displayName.replace(/[^a-zA-Z0-9_. -]/g, '_').slice(0, 128) || 'result.bin';
      anchor.rel = 'noreferrer';
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    });
  }

  function select(candidate: Candidate): void {
    set({
      selection: { routeId: candidate.route.id, modelId: candidate.model.id, revision: candidate.model.revision, options: {} },
      playgroundProviderId: candidate.route.providerId,
      sensitiveConsent: false,
      debugError: null,
      notice: '',
    });
  }

  function setOption(descriptor: OptionView, value: unknown): void {
    const { selection, sensitiveConsent } = get();
    if (!selection) return;
    if (value !== undefined) {
      const error = validateOption(descriptor, value);
      if (error) return notify(error);
      if ((descriptor.billingSensitive || descriptor.permissionSensitive) && !sensitiveConsent) {
        return notify('Review and allow sensitive configuration changes before changing this option.');
      }
    }
    const options = { ...selection.options };
    if (value === undefined) delete options[descriptor.id];
    else options[descriptor.id] = value;
    set({ selection: { ...selection, options } satisfies Selection, notice: '' });
  }

  function toggleFavorite(key: string): void {
    set((state) => ({ favorites: state.favorites.includes(key) ? state.favorites.filter((item) => item !== key) : [...state.favorites, key] }));
  }

  const actions: DemoActions = {
    patch,
    select,
    toggleFavorite,
    setOption,
    startTurn,
    cancelTurn,
    suspendSession,
    resumeSession,
    answerApproval,
    scan,
    probeConnection,
    probeAllConnections,
    revokeConnection,
    refreshDiagnostics,
    refreshCosts,
    savePrices,
    downloadArtifact,
    connectLive: (endpoint, token) => attach(() => factory.connectLive(endpoint, token), 'live'),
    pair: () => factory.pair?.(),
  };

  // Boot exactly once at creation: the store, not a component effect, owns the client.
  void attach(
    factory.initialClient ? () => Promise.resolve(factory.initialClient as LocalAIClient) : factory.connectFixture,
    factory.initialMode ?? 'synthetic',
  );

  return {
    store,
    actions,
    async dispose() {
      if (closed) return;
      closed = true;
      epoch += 1;
      await stopClient();
    },
  };
}
