import { debugSessionError, userFacingSessionError } from '../../lib/errors.mjs';
import type { Event as SessionEvent } from 'originlm';
import type { ApprovalRequest, ChatMessage, DemoState, SessionStatus } from './types';

const MAX_EVENTS = 100;
const MAX_TEXT = 65_536;

let messageCounter = 0;
export function newMessageId(prefix: string): string {
  messageCounter += 1;
  return `${prefix}-${Date.now().toString(36)}-${messageCounter}`;
}

function appendAssistantText(chat: ChatMessage[], text: string): ChatMessage[] {
  const last = chat.at(-1);
  if (!last || last.role !== 'assistant') {
    return [...chat, { id: newMessageId('assistant'), role: 'assistant', text: text.slice(-MAX_TEXT), status: 'Running' }];
  }
  return [...chat.slice(0, -1), { ...last, text: (last.text + text).slice(-MAX_TEXT) }];
}

export function markAssistantStatus(chat: ChatMessage[], status: string): ChatMessage[] {
  const last = chat.at(-1);
  if (!last || last.role !== 'assistant') return chat;
  return [...chat.slice(0, -1), { ...last, status }];
}

function finalStatus(status: string): SessionStatus {
  if (status === 'cancelled') return 'Cancelled';
  if (status === 'denied') return 'Denied';
  if (status === 'completed') return 'Completed';
  return 'Error';
}

/**
 * Pure reducer: folds a batch of stream events into state. Batching keeps a fast
 * `text_delta` stream from re-rendering the app once per token.
 */
export function reduceEvents(
  state: Pick<DemoState, 'events' | 'sessionId' | 'chat' | 'approval' | 'sessionStatus' | 'notice' | 'debugError'>,
  batch: readonly SessionEvent[],
): Partial<DemoState> {
  let { chat, approval, sessionStatus, notice, debugError } = state;
  for (const event of batch) {
    if (event.sessionId !== state.sessionId) continue;
    const payload = event.payload;
    switch (payload.type) {
      case 'text_delta':
        if (typeof payload.text === 'string') chat = appendAssistantText(chat, payload.text);
        break;
      case 'approval_request':
        approval = { approvalId: payload.approvalId, action: payload.action, detail: payload.detail } satisfies ApprovalRequest;
        sessionStatus = 'Awaiting approval';
        break;
      case 'approval_resolved':
        approval = null;
        // The turn resumes after an answer; a final event follows when it ends.
        if (sessionStatus === 'Awaiting approval') sessionStatus = 'Running';
        break;
      case 'final':
        sessionStatus = finalStatus(payload.status);
        approval = null;
        chat = markAssistantStatus(chat, sessionStatus);
        break;
      case 'error':
        sessionStatus = 'Error';
        notice = userFacingSessionError(payload);
        debugError = debugSessionError(payload);
        chat = markAssistantStatus(chat, 'Error');
        break;
      default:
        break;
    }
  }
  return { events: [...state.events, ...batch].slice(-MAX_EVENTS), chat, approval, sessionStatus, notice, debugError };
}
