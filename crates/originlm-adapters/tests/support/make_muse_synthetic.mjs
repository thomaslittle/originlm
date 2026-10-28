// Derives the synthetic Muse transcripts in fixtures/muse/synthetic from the
// Muse 1.4.7 echo recording (fixtures/muse/echo_host_mcp.ndjson): the turn
// body is replaced with hand-written MSP frames that echo cannot produce.
//   node make_muse_synthetic.mjs <fixtures/muse directory>
import {readFileSync, writeFileSync} from 'node:fs';
const dir = process.argv[2];
const base = readFileSync(`${dir}/echo_host_mcp.ndjson`, 'utf8').trim().split('\n').map(l => JSON.parse(l));
const start = base.find(l => l.label === 'turn/started').frame.params;
const session = start.sessionId, turn = start.turnId;
let cursor = 900;
const n = (method, params) => ({type: 'emit_inbound', label: method, frame: {jsonrpc: '2.0', method, params: {sessionId: session, viewCursor: `v:${session}:${cursor++}`, ...params}}});
const item = (method, item) => n(method, {item: {turnId: turn, ...item}});
const tool = (id, name, extra = {}, status = 'completed') => [
  item('item/started', {itemId: id, kind: 'toolCall', revision: 1, status: 'inProgress', tool: name, args: extra.args}),
  item('item/completed', {itemId: id, kind: 'toolCall', revision: 2, status, tool: name, ...extra}),
];
const message = (id, text) => [
  item('item/started', {itemId: id, kind: 'agentMessage', revision: 1, status: 'inProgress', text: ''}),
  n('item/delta', {itemId: id, field: 'text', delta: text}),
  item('item/completed', {itemId: id, kind: 'agentMessage', revision: 2, status: 'completed', text}),
];
// Replace the recorded turn body (between turn/started and turn/completed) with `body`.
const variant = (name, body, completed, truncate = false) => {
  const out = [];
  let skipping = false;
  for (const line of base) {
    if (line.type === 'transcript_start') { out.push({...line, scenario: name, metadata: {...line.metadata, generatedBy: 'synthetic: derived from echo_host_mcp.ndjson (Muse 1.4.7 echo recording)'}}); continue; }
    if (line.label === 'turn/started') {
      out.push(line, ...body);
      if (truncate) break;
      skipping = true;
      continue;
    }
    if (line.label === 'turn/completed') {
      skipping = false;
      out.push(completed ? {...line, frame: {...line.frame, params: {...line.frame.params, ...completed}}} : line);
      continue;
    }
    if (skipping && line.label !== 'session/statusChanged') continue;
    out.push(line);
  }
  writeFileSync(`${dir}/synthetic/${name}.ndjson`, out.map(l => JSON.stringify(l)).join('\n') + '\n');
};
variant('host_tool', [
  ...tool('01a13400-0000-7000-8000-000000000001', 'mcp__design__set_fill', {args: '{"color":"#ff4d4d"}', visibleOutput: 'ok'}),
  ...message('01a13400-0000-7000-8000-000000000002', 'Filled with the brand red.'),
]);
variant('foreign_mcp_tool', [
  ...tool('01a13400-0000-7000-8000-000000000003', 'mcp__other__delete_everything', {args: '{}'}),
  ...message('01a13400-0000-7000-8000-000000000004', 'This text must not reach the host.'),
]);
variant('read_tools', [
  ...tool('01a13400-0000-7000-8000-000000000005', 'read_file', {args: '{"path":"notes.md"}', visibleOutput: 'notes'}),
  ...tool('01a13400-0000-7000-8000-000000000006', 'bash', {args: '{"command":"ls"}'}),
]);
variant('web_and_reasoning', [
  item('item/started', {itemId: '01a13400-0000-7000-8000-000000000010', kind: 'reasoning', revision: 1, status: 'inProgress', summary: []}),
  n('item/delta', {itemId: '01a13400-0000-7000-8000-000000000010', field: 'summary.0', delta: 'Checking the current Node release.'}),
  n('item/delta', {itemId: '01a13400-0000-7000-8000-000000000010', field: 'text', delta: 'raw private reasoning must stay private'}),
  item('item/completed', {itemId: '01a13400-0000-7000-8000-000000000010', kind: 'reasoning', revision: 2, status: 'completed', summary: ['Checking the current Node release.'], text: 'raw private reasoning must stay private'}),
  n('session/todoListChanged', {items: [{text: 'Search the web', status: 'completed'}, {text: 'Answer', status: 'inProgress'}]}),
  ...tool('01a13400-0000-7000-8000-000000000011', 'web_search', {args: '{"query":"node lts"}', visibleOutput: JSON.stringify({query: 'node lts', results: [{url: 'https://nodejs.org/', title: 'Node.js', snippet: 'Node 24 is LTS', extra: 1}, {url: 'javascript:alert(1)', title: '', snippet: 'No link'}]})}),
  ...tool('01a13400-0000-7000-8000-000000000012', 'web_fetch', {args: '{"url":"https://nodejs.org/en/about"}', visibleOutput: 'About Node.js'}),
  n('turn/retryScheduled', {turnId: turn, attempt: 1, nextAttempt: 2, maxAttempts: 3, reason: 'upstream 529', retryDelayMs: 10}),
  ...message('01a13400-0000-7000-8000-000000000013', 'Node 24 is the current LTS.'),
]);
const question = {type: 'emit_inbound', label: 'request:userInput/request', frame: {jsonrpc: '2.0', id: 7, method: 'userInput/request', params: {sessionId: session, turnId: turn, userInputId: '01a13400-0000-7000-8000-000000000020', itemId: '01a13400-0000-7000-8000-000000000021', toolCallId: 'call_1', toolName: 'request_user_input', viewCursor: `v:${session}:${cursor++}`, questions: [{id: 'q1', header: 'Color', question: 'Which color?', options: [{label: 'Red'}], selection: {mode: 'single'}}]}}};
variant('question', [
  question,
  {type: 'expect_outbound', label: 'reply:userInput/request', frame: {jsonrpc: '2.0', id: 7, result: {}}},
  {type: 'expect_outbound', label: 'userInput/cancel', frame: {jsonrpc: '2.0', id: 9, method: 'userInput/cancel', params: {sessionId: session, userInputId: '01a13400-0000-7000-8000-000000000020'}}},
  {type: 'emit_inbound', label: 'response:userInput/cancel', frame: {jsonrpc: '2.0', id: 9, result: {commandId: 'x', status: 'accepted', userInputId: '01a13400-0000-7000-8000-000000000020'}}},
  ...message('01a13400-0000-7000-8000-000000000022', 'Continuing without an answer.'),
]);
variant('auth_required', [], {terminal: 'failed', error: {kind: 'authRequired', message: 'Sign in with muse login (device code ABCD-1234)', retryable: false}});
variant('model_access', [
  ...message('01a13400-0000-7000-8000-000000000030', 'partial'),
], {terminal: 'failed', error: {kind: 'modelError', message: "Model 'muse-spark-internal' does not exist or you lack access to it", retryable: false}});
// The stream ends mid-reply: no item/completed, no turn/completed.
variant('truncated', [
  item('item/started', {itemId: '01a13400-0000-7000-8000-000000000040', kind: 'agentMessage', revision: 1, status: 'inProgress', text: ''}),
  n('item/delta', {itemId: '01a13400-0000-7000-8000-000000000040', field: 'text', delta: 'partial repl'}),
], undefined, true);
console.log('ok');
