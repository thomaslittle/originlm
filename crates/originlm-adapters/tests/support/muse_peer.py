"""Replays a recorded Muse Session Protocol transcript as a fake `muse serve`.

Usage: muse_peer.py TRANSCRIPT LOG [MODE]

TRANSCRIPT uses T3 Code's format: `expect_outbound` frames are what the client
sent when the transcript was recorded, `emit_inbound` frames what Muse sent.
The peer waits for each expected request (matched by method), then emits the
recorded inbound frames up to the next expectation. Ids the client mints
(session id, command ids, request ids) replace the recorded ones in every
emitted frame. `account/read`, `model/list` and `usage/read` get canned,
deterministic answers so every transcript shares one catalog. Every client
frame is appended to LOG (one JSON object per line) for assertions.

MODE: normal | logged_out | no_session_mcp | bad_schema | session_missing | exit_at_end
"""
import json, sys

transcript, log_path = sys.argv[1], sys.argv[2]
mode = sys.argv[3] if len(sys.argv) > 3 else 'normal'
with open(transcript, encoding='utf-8') as handle:
    entries = [json.loads(line) for line in handle if line.strip()]
header = entries[0] if entries and entries[0].get('type') == 'transcript_start' else {}
model = (header.get('metadata') or {}).get('model') or 'muse-spark-1.3-contributor'
if model == 'echo':
    model = 'muse-spark-1.3-contributor'
entries = [e for e in entries if e.get('type') in ('expect_outbound', 'emit_inbound') and e.get('label') != 'host_start']
pos = 0
ids = {}
requests = {}
out = sys.stdout.buffer
log = open(log_path, 'a', encoding='utf-8')


def write(frame):
    out.write(json.dumps(frame, separators=(',', ':')).encode('utf-8') + b'\n')
    out.flush()


def rewrite(frame):
    text = json.dumps(frame)
    for recorded, actual in ids.items():
        text = text.replace(recorded, actual)
    frame = json.loads(text)
    if 'method' not in frame and 'id' in frame:
        frame['id'] = requests.get(frame['id'], frame['id'])
    return frame


def flush():
    global pos
    while pos < len(entries) and entries[pos]['type'] == 'emit_inbound':
        write(rewrite(entries[pos]['frame']))
        pos += 1


def expected():
    return entries[pos] if pos < len(entries) and entries[pos]['type'] == 'expect_outbound' else None


def skip_recorded(method):
    """Drop a recorded request/response pair the peer answers itself."""
    global pos
    current = expected()
    if current and current['frame'].get('method') == method:
        pos += 1
        if pos < len(entries) and entries[pos]['type'] == 'emit_inbound' and entries[pos]['label'] == 'response:' + method:
            pos += 1


CATALOG = {
    'providerId': 'meta', 'profileId': 'tbh', 'source': 'providerCatalog',
    'models': [{
        'modelId': model, 'displayLabel': 'Muse Spark (fixture)', 'providerId': 'meta', 'profileId': None,
        'releaseDate': None, 'description': None, 'contextLimit': 262144, 'outputLimit': None, 'cost': None,
        'isActive': False, 'isDefault': True, 'variants': ['low', 'medium', 'high', 'xhigh'],
        'defaultReasoningEffort': 'high', 'reasoningEffortVariants': [{'tier': 'high', 'description': 'Default'}],
    }],
}
ACCOUNT = {'state': 'loggedOut', 'credentialRequired': True} if mode == 'logged_out' else \
    {'state': 'accountLogin', 'label': 'fixture@example.test', 'credentialRequired': True}

CANNED = ('account/read', 'model/list', 'usage/read')

for line in sys.stdin:
    # Recorded metadata reads are answered from the canned catalog instead.
    for skipped in CANNED:
        skip_recorded(skipped)
    message = json.loads(line)
    log.write(json.dumps(message) + '\n')
    log.flush()
    if message.get('jsonrpc') != '2.0':
        sys.exit(7)
    method = message.get('method')
    params = message.get('params') or {}
    if method == 'initialize':
        skip_recorded('initialize')
        result = {
            'serverInfo': {'name': 'muse', 'version': '1.4.7'}, 'userAgent': 'muse-build/1.4.7 (fixture)',
            'museHome': '~/.local/share/muse', 'platformFamily': 'unix', 'platformOs': 'linux',
            'schema': {'version': 2 if mode == 'bad_schema' else 1, 'fingerprint': 'sha256:' + '0' * 64},
            'grantedCapabilities': [] if mode == 'no_session_mcp' else
            [c for c in (params.get('capabilities') or {}).get('requestedCapabilities', []) if c == 'sessionMcp'],
            'experimentalApi': bool((params.get('capabilities') or {}).get('experimentalApi')),
            'sessionDurability': 'ephemeral',
        }
        write({'jsonrpc': '2.0', 'id': message['id'], 'result': result})
        continue
    if method in ('account/read', 'model/list', 'usage/read'):
        skip_recorded(method)
        result = ACCOUNT if method == 'account/read' else CATALOG if method == 'model/list' else {}
        write({'jsonrpc': '2.0', 'id': message['id'], 'result': result})
        continue
    if method == 'session/resume' and mode == 'session_missing':
        write({'jsonrpc': '2.0', 'id': message['id'], 'error': {'code': -32020, 'message': 'session not found', 'data': {'kind': 'sessionNotFound', 'retryable': False}}})
        continue
    current = expected()
    if method is None and 'id' in message:
        # A presentation receipt for a server request.
        if current and 'method' not in current['frame'] and 'id' in current['frame']:
            pos += 1
            flush()
        continue
    if method == 'initialized':
        if current and current['frame'].get('method') == 'initialized':
            pos += 1
            flush()
        continue
    # A resume replays the recorded session start under the resumed id.
    resumes_start = method == 'session/resume' and current and current['frame'].get('method') == 'session/start'
    if current and (current['frame'].get('method') == method or resumes_start):
        recorded = current['frame'].get('params') or {}
        for key in ('sessionId', 'commandId'):
            if isinstance(recorded.get(key), str) and isinstance(params.get(key), str):
                ids[recorded[key]] = params[key]
        if 'id' in current['frame'] and 'id' in message:
            requests[current['frame']['id']] = message['id']
        pos += 1
        flush()
        if mode == 'exit_at_end' and pos >= len(entries):
            sys.exit(0)
        continue
    if 'id' in message:
        # Not in the recording: acknowledge generically and keep the script.
        result = {'commandId': params.get('commandId'), 'status': 'accepted'}
        if method == 'session/start':
            result = {'session': {'sessionId': params.get('sessionId'), 'status': 'idle', 'activeTurnId': None,
                                  'modelId': params.get('modelId')}, 'viewCursor': 'v:0'}
        if method == 'session/resume':
            result = {'session': {'sessionId': params.get('sessionId'), 'status': 'idle', 'activeTurnId': None,
                                  'modelId': model}, 'history': {'items': None, 'mode': 'none', 'snapshot': None}}
        write({'jsonrpc': '2.0', 'id': message['id'], 'result': result})
