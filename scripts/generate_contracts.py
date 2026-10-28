"""Deterministically derive public types from canonical Rust JSON Schema. Imports do no I/O."""
import argparse
import json
import keyword
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
BANNER = 'Generated from originlm-contracts. Do not edit; run python scripts/generate_contracts.py.'

def definitions(schema):
    result = schema.get('$defs', schema.get('definitions', {}))
    if not isinstance(result, dict):
        raise ValueError('Expected local schema definitions')
    for name in result:
        if not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', name) or keyword.iskeyword(name):
            raise ValueError('Unsafe schema definition name')
    return result

def reference(value, names):
    ref = value['$ref']
    if not isinstance(ref, str) or not ref.startswith(('#/$defs/', '#/definitions/')):
        raise ValueError('Only local schema references are supported')
    name = ref.rsplit('/', 1)[-1]
    if name not in names:
        raise ValueError('Schema reference is not defined')
    return name

def typescript(schema):
    names = definitions(schema)
    def emit(v):
        if v is True or v == {}:
            return 'unknown'
        if v is False:
            return 'never'
        if not isinstance(v, dict):
            raise ValueError('Invalid JSON schema')
        if '$ref' in v:
            return reference(v, names)
        if 'const' in v:
            return json.dumps(v['const'], ensure_ascii=True)
        if 'enum' in v:
            return ' | '.join(json.dumps(x, ensure_ascii=True) for x in v['enum']) or 'never'
        for key in ('oneOf', 'anyOf', 'allOf'):
            if key in v:
                separator = ' & ' if key == 'allOf' else ' | '
                return separator.join(emit(x) for x in v[key]) or ('unknown' if key == 'allOf' else 'never')
        kind = v.get('type', 'object' if 'properties' in v else None)
        if isinstance(kind, list):
            return ' | '.join(emit({**v, 'type': k}) for k in kind)
        if kind in ('string', 'boolean', 'null'):
            return kind
        if kind in ('integer', 'number'):
            return 'number'
        if kind == 'array':
            return 'Array<' + emit(v.get('items', True)) + '>'
        if kind == 'object':
            properties = v.get('properties', {})
            required = set(v.get('required', []))
            fields = [f'{json.dumps(key)}{"" if key in required else "?"}: {emit(value)};' for key, value in sorted(properties.items())]
            additional = v.get('additionalProperties', True)
            if additional is not False:
                fields.append('[key: string]: ' + emit(additional) + ';')
            return '{ ' + ' '.join(fields) + ' }'
        if kind is None:
            return 'unknown'
        raise ValueError('Unsupported schema type: ' + str(kind))
    return '// ' + BANNER + '\n\n' + '\n\n'.join(f'export type {name} = {emit(value)};' for name, value in sorted(names.items())) + '\n'

def python_types(schema):
    names = definitions(schema)
    pending = []
    allocated = set(names)
    def fresh(hint):
        hint = re.sub('[^A-Za-z0-9_]', '_', hint)
        name = hint
        count = 1
        while name in allocated:
            count += 1
            name = f'{hint}_{count}'
        allocated.add(name)
        return name
    def emit(v, hint, root=False):
        if v is True or v == {}:
            return 'Any'
        if v is False:
            return 'Never'
        if not isinstance(v, dict):
            raise ValueError('Invalid JSON schema')
        if '$ref' in v:
            return repr(reference(v, names))
        if 'const' in v:
            return 'None' if v['const'] is None else 'Literal[' + repr(v['const']) + ']'
        if 'enum' in v:
            literal = [x for x in v['enum'] if x is not None]
            result = 'Literal[' + ', '.join(map(repr, literal)) + ']' if literal else 'None'
            return 'Union[' + result + ', None]' if None in v['enum'] and literal else result
        for key in ('oneOf', 'anyOf'):
            if key in v:
                return 'Union[' + ', '.join(emit(x, hint + '_Case' + str(i)) for i, x in enumerate(v[key])) + ']'
        if 'allOf' in v:
            if len(v['allOf']) == 1:
                return emit(v['allOf'][0], hint, root)
            # Do not erase intersections into Any. Object intersections merge required fields.
            merged = {'type': 'object', 'properties': {}, 'required': [], 'additionalProperties': False}
            for part in v['allOf']:
                if part.get('type') != 'object':
                    raise ValueError('Python intersection needs object schemas')
                merged['properties'].update(part.get('properties', {}))
                merged['required'].extend(part.get('required', []))
            return emit(merged, hint, root)
        kind = v.get('type', 'object' if 'properties' in v else None)
        if isinstance(kind, list):
            return 'Union[' + ', '.join(emit({**v, 'type': k}, hint + '_' + k) for k in kind) + ']'
        primitive = {'string': 'str', 'boolean': 'bool', 'integer': 'int', 'number': 'float', 'null': 'None'}
        if kind in primitive:
            return primitive[kind]
        if kind == 'array':
            return 'List[' + emit(v.get('items', True), hint + '_Item') + ']'
        if kind == 'object':
            properties = v.get('properties', {})
            additional = v.get('additionalProperties', True)
            if not properties:
                return 'Dict[str, ' + emit(additional, hint + '_Value') + ']'
            name = hint if root else fresh(hint)
            required = set(v.get('required', []))
            fields = []
            for key, value in sorted(properties.items()):
                typ = emit(value, name + '_' + re.sub('[^A-Za-z0-9_]', '_', key))
                fields.append(repr(key) + ': ' + ('Required' if key in required else 'NotRequired') + '[' + typ + ']')
            pending.append(f'{name} = TypedDict({name!r}, {{' + ', '.join(fields) + '}, total=False)')
            return name
        if kind is None:
            return 'Any'
        raise ValueError('Unsupported schema type')
    for name, value in sorted(names.items()):
        result = emit(value, name, root=True)
        if result != name:
            pending.append(f'{name}: TypeAlias = {result}')
    return '# ' + BANNER + '\nfrom typing import Any, Dict, List, Literal, Never, NotRequired, Required, TypeAlias, TypedDict, Union\n\n' + '\n\n'.join(pending) + '\n'

def protocol_documents(schema):
    names = definitions(schema)
    inputs = names.get('MethodInputs', {}).get('properties', {})
    outputs = names.get('MethodOutputs', {}).get('properties', {})
    if inputs.keys() != outputs.keys():
        raise ValueError('Public request and response method maps disagree')
    methods = [{'name': name, 'paramsSchema': inputs[name], 'resultSchema': outputs[name],
                'availability': 'Negotiate runtime.handshake; per-client grants apply.'}
               for name in sorted(inputs)]
    manifest = {'protocolVersion': '1.0', 'methods': methods, '$defs': names}
    def refs(value):
        if isinstance(value, list):
            return [refs(x) for x in value]
        if isinstance(value, dict):
            result = {key: refs(item) for key, item in value.items()}
            if '$ref' in value:
                result['$ref'] = '#/components/schemas/' + reference(value, names)
            return result
        return value
    def response(value, description='Successful response'):
        return {'description': description, 'content': {'application/json': {'schema': value}}}
    request_alternatives = [
        {'type': 'object', 'required': ['jsonrpc', 'id', 'method'], 'additionalProperties': False,
         'properties': {'jsonrpc': {'const': '2.0'}, 'id': {'type': ['integer', 'string']},
                        'method': {'const': m['name']}, 'params': refs(m['paramsSchema'])}}
        for m in methods
    ]
    rpc_response = {'type': 'object', 'required': ['jsonrpc', 'id'], 'properties': {
        'jsonrpc': {'const': '2.0'}, 'id': {'type': ['integer', 'string', 'null']},
        'result': {}, 'error': {'type': 'object', 'required': ['code', 'message'], 'properties': {
            'code': {'type': 'integer'}, 'message': {'type': 'string'},
            'data': refs({'$ref': '#/$defs/DomainError'}) if 'DomainError' in names else {}}}},
        'oneOf': [{'required': ['result'], 'not': {'required': ['error']}}, {'required': ['error'], 'not': {'required': ['result']}}]}
    paths = {
        '/runtime/info': {'get': {'operationId': 'runtimeInfo', 'security': [],
            'responses': {'200': response({'type': 'object'}, 'Public protocol and pairing requirement only; no account data')}}},
        '/runtime/pairing-code': {
            'get': {'operationId': 'getPairingCode', 'security': [],
                'description': 'Read the active loopback pairing code without consuming it. Same-origin only; no-store. Expires after five minutes and becomes unavailable after redemption or ten failed attempts.',
                'responses': {'200': response({'type': 'object', 'required': ['code', 'expiresInSeconds'], 'additionalProperties': False,
                    'properties': {'code': {'type': 'string', 'pattern': '^[a-f0-9]{12}$'}, 'expiresInSeconds': {'type': 'integer', 'minimum': 1, 'maximum': 300}}}),
                    '403': response({'type': 'object'}, 'Pairing code unavailable or request origin denied')}},
            'post': {'operationId': 'issuePairingCode', 'security': [],
                'description': 'Explicitly issue a fresh single-use loopback pairing code. Replaces the current unused code without revoking already paired clients. Same-origin only; no-store.',
                'responses': {'200': response({'type': 'object', 'required': ['code', 'expiresInSeconds'], 'additionalProperties': False,
                    'properties': {'code': {'type': 'string', 'pattern': '^[a-f0-9]{12}$'}, 'expiresInSeconds': {'const': 300}}}),
                    '403': response({'type': 'object'}, 'Request origin denied')}}},
        '/v1/pair': {'post': {'operationId': 'pair', 'security': [],
            'description': 'Redeem a host-issued single-use code. Does not widen host grants. Codes expire after five minutes.',
            'requestBody': {'required': True, 'content': {'application/json': {'schema': {
                'type': 'object', 'required': ['code'], 'additionalProperties': False, 'properties': {'code': {'type': 'string', 'minLength': 12, 'maxLength': 12}}}}}},
            'responses': {'200': response({'type': 'object', 'properties': {'token': {'type': 'string'}, 'protocolVersion': {'type': 'string'}, 'expiresInSeconds': {'type': 'integer'}}}),
                          '403': response({'type': 'object'}, 'Pairing denied')}}},
        '/v1/rpc': {'post': {'operationId': 'rpc', 'description': 'JSON-RPC 2.0. Negotiate runtime.handshake first. Only advertised and authorized methods may be invoked.',
            'requestBody': {'required': True, 'content': {'application/json': {'schema': {'oneOf': request_alternatives}}}},
            'responses': {'200': response(rpc_response), '401': response({'type': 'object'}, 'Authentication required')}}},
        '/v1/events': {'get': {'operationId': 'events', 'description': 'Authorized server-sent events. Explicit event-gap errors; reconnecting never re-runs a turn.',
            'parameters': [{'name': k, 'in': 'query', 'required': False, 'schema': {'type': 'string'}} for k in ('sessionId', 'after')],
            'responses': {'200': {'description': 'SSE update/gap events', 'content': {'text/event-stream': {'schema': {'type': 'string'}}}}}}},
    }
    reads = {'snapshot': 'snapshot.get', 'connections': 'connections.list', 'models': 'models.list', 'usage': 'usage.read', 'diagnostics': 'diagnostics.report'}
    for path, method in reads.items():
        paths['/v1/' + path] = {'get': {'operationId': path, 'description': 'Same permission-scoped data as ' + method + '; no implicit enrichment.',
            'responses': {'200': response(refs(outputs.get(method, {}))), '401': response({'type': 'object'}, 'Authentication and handshake required')}}}
    api = {'openapi': '3.1.0', 'info': {'title': 'OriginLM local runtime', 'version': '1.0',
            'description': 'Generated from Rust method/domain contracts. The local runtime is paired, user-scoped, and never a public credential proxy.'},
           'security': [{'localBearer': []}], 'paths': paths,
           'components': {'securitySchemes': {'localBearer': {'type': 'http', 'scheme': 'bearer'}}, 'schemas': refs(names)}}
    return manifest, api

def client_stub(schema):
    names = definitions(schema)
    inputs = names['MethodInputs']['properties']
    outputs = names['MethodOutputs']['properties']
    def typename(value):
        if not isinstance(value, dict):
            return 'Any'
        if '$ref' in value:
            return reference(value, names)
        if value.get('type') == 'array':
            return 'List[' + typename(value.get('items', {})) + ']'
        return 'Any'
    families = {}
    for method in sorted(inputs):
        family, name = method.split('.', 1)
        if family in ('events', 'auth') or method == 'runtime.handshake':
            continue
        families.setdefault(family, []).append((name, typename(inputs[method]), typename(outputs[method])))
    lines = ['# ' + BANNER, 'from typing import Any, AsyncIterator, Iterator, List',
             'from .generated import *', 'from .stdio import AsyncStdioTransport', '']
    for prefix, asynchronous in [('Async', True), ('Sync', False)]:
        lines.append(f'class _{prefix}Events:')
        lines.append(f'    def subscribe(self, query: EventsParams | None = ...) -> {"AsyncIterator" if asynchronous else "Iterator"}[Event]: ...')
        lines.append('')
        lines.append(f'class _{prefix}Chatgpt:')
        lines.append(f'    {"async " if asynchronous else ""}def status(self, params: EmptyParams | None = ..., **options: Any) -> ChatgptAuthStatus: ...')
        lines.append(f'    {"async " if asynchronous else ""}def start(self, params: EmptyParams | None = ..., **options: Any) -> ChatgptAuthStart: ...')
        lines.append(f'    {"async " if asynchronous else ""}def cancel(self, params: ChatgptCancelParams, **options: Any) -> Any: ...')
        lines.append(f'    {"async " if asynchronous else ""}def disconnect(self, params: EmptyParams | None = ..., **options: Any) -> Any: ...')
        lines.append('')
        lines.append(f'class _{prefix}Auth:')
        lines.append('    @property')
        lines.append(f'    def chatgpt(self) -> _{prefix}Chatgpt: ...')
        lines.append('')
        for family, methods in families.items():
            lines.append(f'class _{prefix}{family.title()}:')
            for method, input_type, output_type in methods:
                # Projections can be partial; the no-params Snapshot overload remains precise.
                lines.append(f'    {"async " if asynchronous else ""}def {method}(self, params: {input_type} | None = ..., **options: Any) -> {output_type}: ...')
            lines.append('')
        name = 'AsyncClient' if asynchronous else 'Client'
        lines.append(f'class {name}:')
        lines.append('    handshake: dict[str, Any]')
        lines.append('    mode: str')
        lines.append('    fallback_reason: str | None')
        lines.extend(['    @property', f'    def events(self) -> _{prefix}Events: ...'])
        lines.extend(['    @property', f'    def auth(self) -> _{prefix}Auth: ...'])
        if asynchronous:
            lines.extend(['    @classmethod', '    async def connect(cls, transport: Any) -> AsyncClient: ...'])
            lines.extend(['    @classmethod', '    async def auto(cls, executable: str | None = ..., **options: Any) -> AsyncClient: ...'])
        else:
            lines.extend(['    @classmethod', '    def from_stdio(cls, **options: Any) -> Client: ...'])
            lines.extend(['    @classmethod', '    def auto(cls, executable: str | None = ..., **options: Any) -> Client: ...'])
        for family in families:
            lines.extend(['    @property', f'    def {family}(self) -> _{prefix}{family.title()}: ...'])
        lines.extend([f'    {"async " if asynchronous else ""}def request(self, method: str, params: Any = ..., **options: Any) -> Any: ...',
                      f'    def subscribe(self, query: EventsParams | None = ...) -> {"AsyncIterator" if asynchronous else "Iterator"}[Event]: ...',
                      f'    {"async " if asynchronous else ""}def close(self) -> None: ...'])
        if asynchronous:
            lines.extend(['    async def __aenter__(self) -> AsyncClient: ...', '    async def __aexit__(self, *args: Any) -> None: ...'])
        else:
            lines.extend(['    def __enter__(self) -> Client: ...', '    def __exit__(self, *args: Any) -> None: ...'])
        lines.append('')
    return '\n'.join(lines) + '\n'

def render_files(schema):
    manifest, api = protocol_documents(schema)
    return {
        'protocol/methods.json': json.dumps(manifest, indent=2, sort_keys=True) + '\n',
        'protocol/openapi.json': json.dumps(api, indent=2, sort_keys=True) + '\n',
        'protocol/contracts.schema.json': json.dumps(schema, indent=2, ensure_ascii=True, sort_keys=True) + '\n',
        'packages/sdk-typescript/src/generated.ts': typescript(schema),
        'packages/sdk-python/src/originlm/generated.py': python_types(schema),
        'packages/sdk-python/src/originlm/client.pyi': client_stub(schema),
    }

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'originlm-contracts', '--example', 'schema'], cwd=ROOT, capture_output=True)
    if result.returncode:
        sys.stderr.buffer.write(result.stderr)
        return result.returncode
    schema = json.loads(result.stdout)
    failures = []
    for path, text in render_files(schema).items():
        target = ROOT / path
        if args.check:
            if not target.is_file() or target.read_text(encoding='utf-8') != text:
                failures.append(path)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding='utf-8', newline='\n')
            print('Generated:', path)
    if failures:
        print('Generated contracts are stale: ' + ', '.join(failures), file=sys.stderr)
        return 1
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
