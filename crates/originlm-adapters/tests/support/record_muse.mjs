// Records an MSP transcript from an installed Muse Code on its free `echo`
// provider, in T3 Code's transcript format (transcript_start, expect_outbound,
// emit_inbound). The requests mirror what the kit sends. Paths, the account
// label and the bearer token are replaced with placeholders.
//
//   node record_muse.mjs <muse-bin.exe or muse> <out.ndjson> <scenario>
//   scenarios: echo_host_mcp (one read-only turn with an unreachable optional
//   host MCP server), echo_multi_turn (two turns, persistent off)
import {spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {mkdtempSync, realpathSync, writeFileSync} from 'node:fs';
import {tmpdir, homedir} from 'node:os';
import {join} from 'node:path';

const [exe, out, scenario] = process.argv.slice(2);
if (!exe || !out || !['echo_host_mcp', 'echo_multi_turn'].includes(scenario)) {
  console.error('usage: record_muse.mjs <muse> <out.ndjson> echo_host_mcp|echo_multi_turn');
  process.exit(2);
}
const uuidv7 = () => {
  const b = randomBytes(16);
  const t = BigInt(Date.now());
  for (let i = 0; i < 6; i++) b[i] = Number((t >> BigInt(8 * (5 - i))) & 0xffn);
  b[6] = (b[6] & 0x0f) | 0x70;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = b.toString('hex');
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
};
const workspace = realpathSync(mkdtempSync(join(tmpdir(), 'originlm-muse-record-')));
const verbatim = process.platform === 'win32' ? `\\\\?\\${workspace}` : workspace;
const token = 'fixture-bearer-token-0123456789abcdef';
const args = ['serve', '--no-session-log', '--disable-shell', '--disable-write',
  '-c', 'tools.web_search.mode="off"', '-c', 'tools.web_fetch.enabled=false',
  '-c', 'tools.artifact.enabled=false', '-c', 'run.workflow_trigger_mode="off"',
  '--provider', 'echo'];
const env = {...process.env, MUSE_NO_AUTO_UPDATE: '1'};
delete env.META_API_KEY;
const child = spawn(exe, args, {cwd: workspace, env, stdio: ['pipe', 'pipe', 'inherit']});
const lines = [];
const secrets = [];
const sanitize = frame => {
  let text = JSON.stringify(frame);
  const swap = (value, placeholder) => {
    if (!value) return;
    for (const form of [value, JSON.stringify(value).slice(1, -1)]) text = text.split(form).join(placeholder);
  };
  swap(verbatim, '<workspace>');
  swap(workspace, '<workspace>');
  for (const secret of secrets) swap(secret, 'fixture@example.test');
  swap(token, '<bearer-token>');
  swap(homedir(), '~');
  return JSON.parse(text);
};
const record = (type, label, frame) => lines.push({type, label, frame: sanitize(frame)});
let buf = '';
let id = 0;
const waiters = new Map();
child.stdout.on('data', data => {
  buf += data;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (!line.trim()) continue;
    const frame = JSON.parse(line);
    if (frame.method && frame.id !== undefined) {
      record('emit_inbound', `request:${frame.method}`, frame);
      const reply = {jsonrpc: '2.0', id: frame.id, result: {}};
      record('expect_outbound', `reply:${frame.method}`, reply);
      child.stdin.write(JSON.stringify(reply) + '\n');
    } else if (frame.method) {
      record('emit_inbound', frame.method, frame);
      if (frame.method === 'turn/completed') waiters.get('turn')?.();
    } else {
      const waiter = waiters.get(frame.id);
      if (waiter) waiter(frame);
    }
  }
});
const send = (method, params) => {
  const frame = {jsonrpc: '2.0', id: ++id, method, ...(params === undefined ? {} : {params})};
  record('expect_outbound', method, frame);
  child.stdin.write(JSON.stringify(frame) + '\n');
  return new Promise(resolve => waiters.set(frame.id, response => {
    if (method === 'account/read' && response.result?.label) secrets.push(response.result.label);
    record('emit_inbound', `response:${method}`, response);
    resolve(response.result);
  }));
};
const turn = async text => {
  const done = new Promise(resolve => waiters.set('turn', resolve));
  await send('turn/start', {sessionId: session, input: [{type: 'text', text}], ifBusy: 'queue', commandId: uuidv7()});
  await done;
  await new Promise(resolve => setTimeout(resolve, 400));
};
lines.push({type: 'transcript_start', provider: 'muse', protocol: 'muse.msp-jsonl', version: '', scenario,
  metadata: {generatedBy: 'originlm record_muse.mjs', model: 'echo', provider: 'echo'}});
lines.push({type: 'expect_outbound', label: 'host_start', frame: {type: 'host_start', args}});
const init = await send('initialize', {clientInfo: {name: 'originlm', title: 'OriginLM', version: 'record'},
  capabilities: {experimentalApi: true, requestedCapabilities: scenario === 'echo_host_mcp' ? ['sessionMcp'] : []}});
lines[0].version = init.serverInfo.version;
child.stdin.write(JSON.stringify({jsonrpc: '2.0', method: 'initialized'}) + '\n');
record('expect_outbound', 'initialized', {jsonrpc: '2.0', method: 'initialized'});
await send('account/read');
await send('model/list', {});
await send('usage/read', {});
const session = uuidv7();
const start = {sessionId: session, workspaceRoot: verbatim, approvalMode: 'denyUnmatched', commandId: uuidv7()};
if (scenario === 'echo_host_mcp') {
  start.config = {mcpServers: {design: {transport: 'streamableHttp', url: 'http://127.0.0.1:9/mcp',
    headers: {Authorization: `Bearer ${token}`}, mode: 'optional'}}};
}
await send('session/start', start);
await new Promise(resolve => setTimeout(resolve, 300));
if (scenario === 'echo_host_mcp') {
  await turn('Use the design tools if you have them.');
} else {
  await turn('first kit turn');
  await turn('second kit turn');
}
child.stdin.end();
await new Promise(resolve => child.on('exit', resolve));
writeFileSync(out, lines.map(line => JSON.stringify(line)).join('\n') + '\n');
console.log(`wrote ${lines.length} frames to ${out}`);
