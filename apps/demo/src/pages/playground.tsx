import { ArrowUpIcon, DownloadSimpleIcon, InfoIcon, SparkleIcon, StopIcon } from '@phosphor-icons/react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Event as SessionEvent } from 'originlm';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Field, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from '@/components/ui/input-group';
import { Bubble, BubbleContent } from '@/components/ui/bubble';
import { Message, MessageContent } from '@/components/ui/message';
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';
import { CheckRow } from '@/components/common/check-row';
import { Section } from '@/components/common/section';
import { ModelPicker } from '@/components/model-picker';
import { reasonLabel } from '@/lib/format';
import { cn } from '@/lib/utils';
import { useActions, useDemo, useSelectedCandidate } from '@/state/context';
import type { ChatMessage, SessionStatus } from '@/state/types';
import { ConfigurationPanel } from '@/pages/selectors/configuration-panel';

/* Chat composition follows shadcn-ui/chatbot-template (MIT): an Empty state with
   suggestions, a MessageScroller thread of Message + Bubble pairs, a "Thinking…"
   shimmer, a destructive Alert for failures, and a PromptForm whose InputGroup carries
   the model control beside Send/Stop. The catalog, permission gates, and session
   engine stay OriginLM specific: every turn needs an explicit route, consent, and click. */

const SUGGESTIONS: { label: string; prompt: string }[] = [
  { label: 'Explain the selected route', prompt: 'Explain what this route reports: its account, billing, and native options.' },
  { label: 'Draft a plan with this model', prompt: 'Draft a short plan for trying this model route in an isolated workspace.' },
  { label: 'What can this model do?', prompt: 'Summarize the capabilities this model reports, without guessing.' },
  { label: 'Summarize the usage evidence', prompt: 'Summarize the usage windows linked to this route.' },
];

const BUSY: SessionStatus[] = ['Starting', 'Running', 'Awaiting approval'];

function statusDot(status: SessionStatus): string {
  if (BUSY.includes(status)) return 'bg-primary motion-safe:animate-pulse';
  if (status === 'Completed') return 'bg-success';
  if (status === 'Ready' || status === 'Suspended') return 'bg-muted-foreground';
  return 'bg-warning';
}

function ThreadMessage({ message, last }: { message: ChatMessage; last: boolean }) {
  if (message.role === 'user') {
    return (
      <Message align="end">
        <MessageContent>
          <Bubble align="end" variant="muted">
            <BubbleContent className="whitespace-pre-wrap">{message.text}</BubbleContent>
          </Bubble>
        </MessageContent>
      </Message>
    );
  }
  const failedToStart = message.status === 'Error' && !message.text;
  return (
    <Message align="start">
      <MessageContent>
        {message.text ? (
          <div id={last ? 'response-text' : undefined} className="prose prose-sm max-w-none px-1.5 dark:prose-invert prose-pre:rounded-lg prose-pre:border prose-pre:bg-surface prose-pre:text-foreground">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.text}</ReactMarkdown>
          </div>
        ) : failedToStart ? (
          <p className="px-1.5 text-sm text-muted-foreground">The turn did not start.</p>
        ) : null}
        {message.status && message.status !== 'Running' && message.text ? (
          <div className="px-1.5">
            <Badge variant={message.status === 'Completed' ? 'success' : 'warning'}>{message.status}</Badge>
          </div>
        ) : null}
      </MessageContent>
    </Message>
  );
}

function Thread() {
  const actions = useActions();
  const chat = useDemo((state) => state.chat);
  const status = useDemo((state) => state.sessionStatus);
  const thinking = (status === 'Starting' || status === 'Running') && chat.at(-1)?.text === '';

  if (chat.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <SparkleIcon />
            </EmptyMedia>
            <EmptyTitle>What can I help with?</EmptyTitle>
            <EmptyDescription>Pick a route and start a gated turn. Responses stream through the session you explicitly authorize.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <div className="flex flex-wrap justify-center gap-2">
              {SUGGESTIONS.map((suggestion) => (
                <Button key={suggestion.label} variant="outline" size="sm" onClick={() => actions.patch({ prompt: suggestion.prompt })}>
                  {suggestion.label}
                </Button>
              ))}
            </div>
          </EmptyContent>
        </Empty>
      </div>
    );
  }
  return (
    <MessageScrollerProvider>
      <MessageScroller className="flex-1">
        <MessageScrollerViewport>
          <MessageScrollerContent className="mx-auto flex w-full max-w-2xl flex-col gap-6 px-4 py-6">
            {chat.map((message, index) => (
              <MessageScrollerItem key={message.id} messageId={message.id} scrollAnchor={message.role === 'user'}>
                <ThreadMessage message={message} last={index === chat.length - 1} />
              </MessageScrollerItem>
            ))}
            {thinking ? (
              <MessageScrollerItem messageId="thinking">
                <div className="shimmer flex items-center gap-2 px-3 text-sm text-muted-foreground">Thinking…</div>
              </MessageScrollerItem>
            ) : null}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}

function PromptForm() {
  const actions = useActions();
  const prompt = useDemo((state) => state.prompt);
  const status = useDemo((state) => state.sessionStatus);
  const turnId = useDemo((state) => state.turnId);
  const busy = BUSY.includes(status);
  const running = busy;

  function submit() {
    if (!prompt.trim() || busy) return;
    void actions.startTurn();
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <InputGroup>
        <InputGroupTextarea
          aria-label="Prompt"
          rows={3}
          value={prompt}
          disabled={busy}
          placeholder="Send a message…"
          className="p-3.5"
          onChange={(event) => actions.patch({ prompt: event.target.value })}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              submit();
            }
          }}
        />
        <InputGroupAddon align="block-end">
          <ModelPicker side="top" />
          {running ? (
            <InputGroupButton type="button" size="icon-sm" variant="outline" aria-label="Stop generating" disabled={!turnId} onClick={() => void actions.cancelTurn()} className="ml-auto">
              <StopIcon />
            </InputGroupButton>
          ) : (
            <InputGroupButton type="submit" size="icon-sm" variant="default" aria-label="Send message" disabled={!prompt.trim() || busy} className="ml-auto">
              <ArrowUpIcon />
            </InputGroupButton>
          )}
        </InputGroupAddon>
      </InputGroup>
    </form>
  );
}

function Failure() {
  const debugError = useDemo((state) => state.debugError);
  const notice = useDemo((state) => state.notice);
  if (!debugError) return null;
  return (
    <div className="grid gap-3">
      <Alert variant="destructive">
        <AlertTitle>Request failed</AlertTitle>
        <AlertDescription>{notice || `The session failed. Error code: ${debugError.code}.`}</AlertDescription>
      </Alert>
      <section data-testid="turn-debug" aria-label="Provider error debug details" className="rounded-xl border border-destructive/25 bg-destructive/5 p-4">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-xs font-semibold">Provider error details</h3>
          <Badge variant="warning">{debugError.code}</Badge>
        </div>
        <pre className="my-3 max-h-72 overflow-auto font-mono text-xs break-words whitespace-pre-wrap">{debugError.message}</pre>
        <p className="text-[0.7rem] text-muted-foreground">Credential and session-token fields are redacted.</p>
      </section>
    </div>
  );
}

function Approval() {
  const actions = useActions();
  const approval = useDemo((state) => state.approval);
  if (!approval) return null;
  return (
    <div className="rounded-xl border border-warning/30 bg-warning/10 p-4">
      <h3 className="text-sm font-semibold">{approval.action}</h3>
      <p className="mt-1 text-xs text-muted-foreground">{approval.detail}</p>
      <div className="mt-3 flex flex-wrap gap-2.5">
        <Button onClick={() => void actions.answerApproval(true)}>Approve</Button>
        <Button variant="destructive" onClick={() => void actions.answerApproval(false)}>
          Deny
        </Button>
      </div>
    </div>
  );
}

function Artifacts() {
  const actions = useActions();
  const events = useDemo((state) => state.events);
  const sessionId = useDemo((state) => state.sessionId);
  const canRead = useDemo((state) => state.handshake?.methods.includes('artifacts.read') ?? false);
  const artifacts = events.filter((event) => event.sessionId === sessionId && event.payload.type === 'artifact');
  if (!artifacts.length) return null;
  return (
    <div className="flex flex-wrap gap-2.5">
      {artifacts.map((event: SessionEvent) => (
        <Button key={event.sequence} variant="outline" size="sm" disabled={!canRead} onClick={() => void actions.downloadArtifact(event)}>
          <DownloadSimpleIcon data-icon="inline-start" />
          {`Download result (${event.payload.type === 'artifact' ? event.payload.size : 0} bytes)`}
        </Button>
      ))}
    </div>
  );
}

function ChatCard() {
  const status = useDemo((state) => state.sessionStatus);
  return (
    <Card className="gap-0 overflow-hidden bg-card/70 p-0 backdrop-blur-sm [--card-spacing:--spacing(0)]">
      <div className="flex items-center justify-between gap-3 border-b px-5 py-3.5">
        <h2 className="text-lg font-semibold tracking-tight">Stream output</h2>
        <div id="session-status" role="status" aria-live="polite" className="inline-flex items-center gap-2 rounded-full border bg-surface-subtle px-3 py-1 text-xs">
          <span aria-hidden="true" className={cn('size-1.5 rounded-full', statusDot(status))} />
          {status}
        </div>
      </div>
      <div className="flex h-[30rem] min-h-0 flex-col">
        <Thread />
      </div>
      <div className="mx-auto grid w-full max-w-2xl gap-3 px-4 pb-4">
        <Failure />
        <Approval />
        <Artifacts />
        <PromptForm />
      </div>
    </Card>
  );
}

function ConsentFields() {
  const actions = useActions();
  const live = useDemo((state) => state.mode !== 'synthetic');
  const grantConsent = useDemo((state) => state.grantConsent);
  const simulateApproval = useDemo((state) => state.simulateApproval);
  const workspaceId = useDemo((state) => state.workspaceId);
  return (
    <div className="grid gap-3">
      <CheckRow
        label={live ? 'Allow this live request to use the selected account' : 'Allow this simulated session'}
        checked={grantConsent}
        onCheckedChange={(checked) => actions.patch({ grantConsent: checked })}
      />
      {live ? (
        <Field className="max-w-sm">
          <FieldLabel htmlFor="host-workspace">Host workspace name</FieldLabel>
          <Input id="host-workspace" aria-label="Host workspace name" value={workspaceId} onChange={(event) => actions.patch({ workspaceId: event.target.value })} />
        </Field>
      ) : (
        <CheckRow label="Simulate a tool approval" checked={simulateApproval} onCheckedChange={(checked) => actions.patch({ simulateApproval: checked })} />
      )}
    </div>
  );
}

function SessionButtons() {
  const actions = useActions();
  const live = useDemo((state) => state.mode !== 'synthetic');
  const methods = useDemo((state) => state.handshake?.methods ?? []);
  const busy = useDemo((state) => BUSY.includes(state.sessionStatus));
  const turnId = useDemo((state) => state.turnId);
  const sessionId = useDemo((state) => state.sessionId);
  const nativeResumption = useDemo((state) => state.nativeResumption);
  const resumeToken = useDemo((state) => state.resumeToken);
  const grantConsent = useDemo((state) => state.grantConsent);
  const hasSelection = useDemo((state) => state.selection !== null);
  return (
    <div className="mt-5 flex flex-wrap items-center gap-2.5">
      <Button disabled={busy || !methods.includes('turns.start') || !grantConsent || !hasSelection} onClick={() => void actions.startTurn()}>
        {live ? 'Run live turn' : 'Simulate turn'}
      </Button>
      <Button variant="outline" disabled={!busy || !turnId} onClick={() => void actions.cancelTurn()}>
        Cancel turn
      </Button>
      {methods.includes('sessions.suspend') ? (
        <Button variant="outline" disabled={busy || !sessionId || !nativeResumption} onClick={() => void actions.suspendSession()}>
          Suspend native session
        </Button>
      ) : null}
      {methods.includes('sessions.resume') ? (
        <Button variant="outline" disabled={busy || !resumeToken} onClick={() => void actions.resumeSession()}>
          Resume native session
        </Button>
      ) : null}
    </div>
  );
}

function SessionNotices() {
  const selected = useSelectedCandidate();
  const live = useDemo((state) => state.mode !== 'synthetic');
  const executable = useDemo((state) => state.handshake?.methods.includes('turns.start') ?? false);
  return (
    <>
      <Alert variant="info" role="note" className="mb-4">
        <InfoIcon />
        <AlertDescription>
          {live
            ? 'A live turn uses your selected account allowance. The host controls allowed workspaces and permissions; this demo requests read-only execution.'
            : 'Simulation only. The fixture service returns predetermined text; it does not call an AI provider.'}
        </AlertDescription>
      </Alert>
      {live && !executable ? (
        <p className="mb-4 rounded-xl border bg-surface-subtle px-3.5 py-3 text-xs">
          This runtime allows metadata checks only. Enable execution and approve a workspace in the host application before sending a prompt.
        </p>
      ) : null}
      {selected && !selected.eligible ? (
        <p data-testid="model-eligibility" className="mb-4 rounded-xl border border-warning/25 bg-warning/10 px-3.5 py-3 text-xs text-warning">
          {`This model is reported but needs attention before execution: ${selected.reasons.map(reasonLabel).join(' \u00b7 ')}`}
        </p>
      ) : null}
    </>
  );
}

function SessionControls() {
  return (
    <Section title="Session playground" description="Review the live session controls. Nothing runs until you select a route and authorize a turn.">
      <SessionNotices />
      <ConsentFields />
      <SessionButtons />
    </Section>
  );
}

function Timeline() {
  const events = useDemo((state) => state.events);
  const recent = events.slice(-12).reverse();
  return (
    <Section title="Event timeline" description={`${events.length} recent events; bounded to 100.`}>
      {recent.length ? (
        <ol className="relative grid before:absolute before:top-2 before:bottom-2 before:left-[0.42rem] before:w-px before:bg-border">
          {recent.map((event) => (
            <li key={event.sequence} className="relative flex items-center gap-4 py-2 pl-6 text-xs">
              <span aria-hidden="true" className="absolute top-1/2 left-0 size-[0.9rem] -translate-y-1/2 rounded-full border-2 border-primary/60 bg-card" />
              <span className="font-mono tabular-nums text-muted-foreground">{event.sequence}</span>
              <Badge variant="outline">{event.payload.type}</Badge>
              <small className="ml-auto truncate font-mono text-muted-foreground">{event.sessionId}</small>
            </li>
          ))}
        </ol>
      ) : (
        <p className="text-xs text-muted-foreground">Events appear here as a session streams.</p>
      )}
    </Section>
  );
}

export default function PlaygroundPage() {
  return (
    <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_21rem]">
      <div className="grid min-w-0 gap-5">
        <ChatCard />
        <SessionControls />
        <Timeline />
      </div>
      <ConfigurationPanel />
    </div>
  );
}
