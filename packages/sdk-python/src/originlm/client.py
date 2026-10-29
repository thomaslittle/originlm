"""Async-first client and a small synchronous facade over the same implementation."""
import asyncio
import json
import os
import pathlib
import re
import sys
import stat
import tempfile
import time
from typing import Any
from .errors import LocalAIError
from .stdio import AsyncStdioTransport, _process_options

PLATFORM_PACKAGES = {
    ('linux', 'x86_64'): 'originlm-cli-linux-x64',
    ('linux', 'aarch64'): 'originlm-cli-linux-arm64',
    ('darwin', 'arm64'): 'originlm-cli-darwin-arm64',
    ('darwin', 'x86_64'): 'originlm-cli-darwin-x64',
    ('win32', 'amd64'): 'originlm-cli-win32-x64',
}
BINARY_NAME = 'originlm.exe' if sys.platform == 'win32' else 'originlm'


def _usable(path: str | None) -> bool:
    if not path or not isinstance(path, str) or not pathlib.Path(path).is_absolute() or '\0' in path:
        return False
    if path.lower().endswith(('.cmd', '.bat', '.ps1')):
        return False
    candidate = pathlib.Path(path)
    return candidate.is_file() and os.access(candidate, os.X_OK)


def resolve_helper(executable: str | None = None) -> str:
    """The helper binary: explicit, then LAI_HELPER_PATH, then the bundled one."""
    if executable is not None and not _usable(executable):
        raise LocalAIError('INVALID_EXECUTABLE')
    if _usable(executable):
        return executable  # type: ignore[return-value]
    override = os.environ.get('LAI_HELPER_PATH')
    if override is not None and not _usable(override):
        raise LocalAIError('INVALID_EXECUTABLE')
    if _usable(override):
        return override  # type: ignore[return-value]
    bundled = pathlib.Path(__file__).resolve().parent / '_bin' / BINARY_NAME
    if _usable(str(bundled)):
        return str(bundled)
    raise LocalAIError('RUNTIME_MISSING')


def default_address() -> str:
    configured = os.environ.get('LAI_BROKER_ADDRESS')
    if configured:
        return configured
    if sys.platform == 'win32':
        return 'originlm-user'
    runtime = os.environ.get('XDG_RUNTIME_DIR')
    if runtime and pathlib.Path(runtime).is_absolute():
        return str(pathlib.Path(runtime) / 'originlm' / 'user.sock')
    return str(pathlib.Path.home() / '.originlm' / 'user.sock')


def default_token_file() -> pathlib.Path:
    configured = os.environ.get('LAI_BROKER_TOKEN_FILE')
    if configured:
        return pathlib.Path(configured)
    return pathlib.Path.home() / '.originlm' / 'broker-token.json'


def _shared_token(address: str, token_file: str | os.PathLike | None = None) -> str | None:
    try:
        path = pathlib.Path(token_file) if token_file is not None else default_token_file()
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode):
            return None
        flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0) | getattr(os, 'O_BINARY', 0)
        with os.fdopen(os.open(path, flags), 'rb') as stream:
            opened = os.fstat(stream.fileno())
            if not stat.S_ISREG(opened.st_mode) or opened.st_size > 16384 or (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                return None
            if sys.platform != 'win32' and opened.st_mode & 0o077:
                return None
            data = stream.read(16385)
        if len(data) > 16384:
            return None
        descriptor = json.loads(data.decode('utf-8'))
        if not isinstance(descriptor, dict):
            return None
        if 'address' in descriptor and descriptor['address'] != address:
            return None
        token = descriptor.get('token')
        if not isinstance(token, str) or not re.fullmatch(r'[0-9a-f]{64}', token):
            return None
        return token
    except (OSError, ValueError):
        return None


def _unique_address() -> str:
    suffix = f'{os.getpid()}-{int(time.time() * 1000)}-{os.urandom(3).hex()}'
    if sys.platform == 'win32':
        return f'originlm-private-{suffix}'
    name = f'originlm-{suffix}.sock'
    candidate = str(pathlib.Path(tempfile.gettempdir()) / name)
    # Stay below the smallest supported Unix sun_path bound, including UTF-8.
    return candidate if len(os.fsencode(candidate)) <= 100 else str(pathlib.Path('/tmp') / name)


async def _open_private(executable: str, options: dict) -> AsyncStdioTransport:
    process_options = _process_options(options.get('cwd'), options.get('env'))
    address = _unique_address()
    args = [executable, 'serve', '--ipc', address]
    if options.pop('passive', True):
        args.append('--allow-passive-discovery')
    for provider in options.pop('probe_providers', []):
        if not isinstance(provider, str) or not re.fullmatch(r'[a-z0-9_]{1,32}', provider):
            raise LocalAIError('INVALID_REQUEST')
        args.extend(['--allow-metadata', provider])
    try:
        broker = await asyncio.create_subprocess_exec(
            *args,
            **process_options,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
    except (OSError, ValueError):
        raise LocalAIError('RUNTIME_MISSING') from None
    async def stop():
        if broker.stdin is not None:
            broker.stdin.close()
        try:
            await asyncio.wait_for(broker.wait(), timeout=2)
        except asyncio.TimeoutError:
            if broker.returncode is None:
                try:
                    broker.kill()
                except ProcessLookupError:
                    pass
            await broker.wait()

    try:
        try:
            line = await asyncio.wait_for(broker.stdout.readline(), timeout=8)
        except asyncio.TimeoutError:
            raise LocalAIError('RUNTIME_MISSING') from None
        except ValueError:
            raise LocalAIError('PROTOCOL_INCOMPATIBLE') from None
        if not line or len(line) > 4096:
            raise LocalAIError('PROTOCOL_INCOMPATIBLE')
        try:
            descriptor = json.loads(line.decode('utf-8'))
        except (UnicodeDecodeError, ValueError):
            raise LocalAIError('PROTOCOL_INCOMPATIBLE') from None
        if (
            not isinstance(descriptor, dict)
            or descriptor.get('protocolVersion') != '1.0'
            or descriptor.get('address') != address
            or descriptor.get('independent') is True
            or not isinstance(descriptor.get('token'), str)
            or not re.fullmatch(r'[0-9a-f]{64}', descriptor['token'])
        ):
            raise LocalAIError('PROTOCOL_INCOMPATIBLE')
        transport = await AsyncStdioTransport.open_broker(
            executable=executable,
            address=descriptor['address'],
            token=descriptor['token'],
            **options,
        )
    except BaseException:
        # Includes cancellation: a partially started broker still belongs to us.
        await stop()
        raise
    original_close = transport.close

    async def close():
        try:
            await original_close()
        finally:
            await stop()

    transport.close = close  # type: ignore[method-assign]
    transport.mode = 'child'  # type: ignore[attr-defined]
    return transport


async def open_auto_transport(executable: str | None = None, **options):
    """Resolve a transport: a granted shared broker first, then a private child."""
    helper = resolve_helper(executable)
    address = options.pop('address', None) or default_address()
    if not isinstance(address, str) or not address or len(address) > 200 or '\0' in address:
        raise LocalAIError('INVALID_REQUEST')
    token_file = options.pop('token_file', None)
    explicit_token = options.pop('token', None)
    token = explicit_token if explicit_token is not None else _shared_token(address, token_file)
    fallback_reason = None
    if token:
        try:
            shared_options = {key: value for key, value in options.items() if key not in ('passive', 'probe_providers')}
            transport = await AsyncStdioTransport.open_broker(executable=helper, address=address, token=token, **shared_options)
            try:
                handshake = await transport.request('runtime.handshake', {'protocolVersion': '1.0', 'clientName': 'originlm-auto'})
                if not isinstance(handshake, dict) or not isinstance(handshake.get('protocolVersion'), str) or not re.fullmatch(r'1\.\d+', handshake['protocolVersion']):
                    raise LocalAIError('PROTOCOL_INCOMPATIBLE')
                methods = handshake.get('methods')
                if not isinstance(methods, list) or len(methods) > 512 or any(not isinstance(method, str) for method in methods):
                    raise LocalAIError('PROTOCOL_INCOMPATIBLE')
                transport.mode = 'shared'  # type: ignore[attr-defined]
                return transport
            except BaseException:
                await transport.close()
                raise
        except LocalAIError as error:
            if error.code not in ('RUNTIME_MISSING', 'TRANSPORT_CLOSED', 'PROTOCOL_INCOMPATIBLE'):
                raise
            if error.code == 'PROTOCOL_INCOMPATIBLE':
                fallback_reason = 'FALLBACK_PROTOCOL_MISMATCH'
    transport = await _open_private(helper, options)
    transport.fallback_reason = fallback_reason
    return transport

FAMILIES = {
    'runtime': ('health','closeClient'), 'costs': ('read','setPrices'), 'snapshot': ('get',), 'discovery': ('refresh','watch','cancel'),
    'connections': ('list','setupInstructions','authenticate','probe'), 'models': ('list','page'),
    'usage': ('read',), 'capabilities': ('read',), 'selection': ('resolve','validate'),
    'grants': ('request','list','revoke'), 'sessions': ('create','get','suspend','resume','updateOptions','close'),
    'health': ('get',), 'issues': ('list','acknowledge','snooze'), 'notifications': ('poll','configure','ack'), 'apps': ('list','revoke'),
    'inputs': ('put',), 'turns': ('start','cancel'), 'approvals': ('respond',), 'artifacts': ('read',),
    'diagnostics': ('report',), 'adapters': ('list',), 'events': ('subscribe',), 'inventory': ('subscribe',),
}


class _Namespace:
    def __init__(self, client, name):
        self._client, self._name = client, name

    def __getattr__(self, name):
        if name not in FAMILIES[self._name]:
            raise AttributeError(name)
        if self._name == 'events' and name == 'subscribe':
            return self._client.subscribe
        if self._name == 'inventory' and name == 'subscribe':
            return self._client.subscribe_inventory
        def call(params=None, **options):
            return self._client.request(f'{self._name}.{name}', params, **options)
        return call


class _ChatgptNamespace:
    def __init__(self, client):
        self._client = client

    def __getattr__(self, name):
        if name not in ('status', 'start', 'cancel', 'disconnect'):
            raise AttributeError(name)
        def call(params=None, **options):
            return self._client.request(f'auth.chatgpt.{name}', params, **options)
        return call


class _AuthNamespace:
    def __init__(self, client):
        self.chatgpt = _ChatgptNamespace(client)


class AsyncClient:
    @classmethod
    async def connect(cls, transport):
        try:
            handshake = await transport.request('runtime.handshake', {'protocolVersion':'1.0','clientName':'originlm-python'})
            if not isinstance(handshake, dict) or not isinstance(handshake.get('protocolVersion'), str) or not re.fullmatch(r'1\.\d+', handshake['protocolVersion']):
                raise LocalAIError('PROTOCOL_INCOMPATIBLE')
            methods = handshake.get('methods')
            if not isinstance(methods, list) or len(methods) > 512 or any(not isinstance(m, str) for m in methods):
                raise LocalAIError('PROTOCOL_INCOMPATIBLE')
        except BaseException:
            await transport.close()
            raise
        self = cls()
        self._transport, self._methods, self._closed = transport, frozenset(methods), False
        self.handshake = {**handshake, 'methods':tuple(methods)}
        self.mode = getattr(transport, 'mode', 'custom')
        self.fallback_reason = getattr(transport, 'fallback_reason', None)
        return self

    @classmethod
    async def auto(cls, executable: str | None = None, **options):
        """Connect without naming a binary or transport.

        Tries a granted shared broker, then a private parent-owned helper. Raises
        RUNTIME_MISSING only when no helper binary is bundled or configured.
        """
        transport = await open_auto_transport(executable, **options)
        try:
            return await cls.connect(transport)
        except BaseException:
            await transport.close()
            raise

    def __getattr__(self, name):
        if name == 'auth':
            return _AuthNamespace(self)
        if name not in FAMILIES:
            raise AttributeError(name)
        return _Namespace(self, name)

    def _check(self, method):
        if self._closed:
            raise LocalAIError('CLOSED')
        if method not in self._methods:
            raise LocalAIError('CAPABILITY_UNSUPPORTED')

    async def request(self, method: str, params: Any = None, **options) -> Any:
        self._check(method)
        return await self._transport.request(method, params, **options)

    async def subscribe(self, query=None):
        self._check('events.subscribe')
        async for event in self._transport.subscribe(query):
            yield event

    async def subscribe_inventory(self, after=None):
        self._check('events.subscribe')
        query = {'scope': 'inventory'}
        if after is not None:
            query['after'] = after
        async for event in self._transport.subscribe(query):
            yield event

    async def close(self):
        if not self._closed:
            self._closed = True
            try:
                if 'runtime.closeClient' in self._methods:
                    await self._transport.request('runtime.closeClient', {}, timeout=1)
            except Exception:
                pass  # Remote cleanup cannot prevent local shutdown or expose error bodies.
            finally:
                await self._transport.close()

    async def __aenter__(self):
        return self

    async def __aexit__(self, *_args):
        await self.close()


class Client:
    @classmethod
    def auto(cls, executable: str | None = None, **options):
        """Synchronous dependency-first connect. See AsyncClient.auto."""
        try:
            asyncio.get_running_loop()
        except RuntimeError:
            pass
        else:
            raise LocalAIError('USE_ASYNC_CLIENT')
        runner = asyncio.Runner()
        async def connect():
            return await AsyncClient.auto(executable, **options)
        try:
            client = runner.run(connect())
        except BaseException:
            runner.close()
            raise
        self = cls()
        self._runner, self._client, self._closed = runner, client, False
        self.handshake = client.handshake
        self.mode = client.mode
        self.fallback_reason = client.fallback_reason
        return self

    @classmethod
    def from_stdio(cls, **options):
        try:
            asyncio.get_running_loop()
        except RuntimeError:
            pass
        else:
            raise LocalAIError('USE_ASYNC_CLIENT')
        runner = asyncio.Runner()
        async def connect():
            return await AsyncClient.connect(await AsyncStdioTransport.open(**options))
        try:
            client = runner.run(connect())
        except BaseException:
            runner.close()
            raise
        self = cls()
        self._runner, self._client, self._closed = runner, client, False
        self.handshake = client.handshake
        return self

    def __getattr__(self, name):
        if name == 'auth':
            return _AuthNamespace(self)
        if name not in FAMILIES:
            raise AttributeError(name)
        return _Namespace(self, name)

    def request(self, method, params=None, **options):
        if self._closed:
            raise LocalAIError('CLOSED')
        return self._runner.run(self._client.request(method, params, **options))

    def subscribe(self, query=None):
        if self._closed:
            raise LocalAIError('CLOSED')
        iterator = self._client.subscribe(query)
        try:
            while not self._closed:
                try:
                    yield self._runner.run(anext(iterator))
                except StopAsyncIteration:
                    break
        finally:
            if not self._closed:
                self._runner.run(iterator.aclose())

    def subscribe_inventory(self, after=None):
        if self._closed:
            raise LocalAIError('CLOSED')
        iterator = self._client.subscribe_inventory(after)
        try:
            while not self._closed:
                try:
                    yield self._runner.run(anext(iterator))
                except StopAsyncIteration:
                    break
        finally:
            if not self._closed:
                self._runner.run(iterator.aclose())

    def close(self):
        if not self._closed:
            self._closed = True
            try:
                self._runner.run(self._client.close())
            finally:
                self._runner.close()

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()
