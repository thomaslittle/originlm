"""Explicit native-helper transport. Provider discovery belongs to the native runtime."""
import asyncio
import math
import os
import pathlib
import re
import tempfile
from typing import Any, AsyncIterator, Callable
from .errors import LocalAIError, remote_error
from .wire import encode_request, decode_message


def _process_options(cwd=None, env=None):
    if cwd is not None and (not isinstance(cwd, str) or not pathlib.Path(cwd).is_absolute() or '\0' in cwd):
        raise LocalAIError('INVALID_WORKSPACE')
    if env is not None and not isinstance(env, dict):
        raise LocalAIError('INVALID_ENVIRONMENT')
    allowed = ('PATH','Path','SystemRoot','SYSTEMROOT','WINDIR','HOME','USERPROFILE','TMP','TEMP','TMPDIR','LANG','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_STATE_HOME','XDG_RUNTIME_DIR','APPDATA','LOCALAPPDATA','LOCAL_AI_KIT_STATE_DIR','ORIGINLM_STATE_DIR','DBUS_SESSION_BUS_ADDRESS')
    inherited = {key: os.environ[key] for key in allowed if key in os.environ}
    for key, value in (env or {}).items():
        if not isinstance(key, str) or not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', key) or not isinstance(value, str) or '\0' in value or key.upper() in ('NODE_OPTIONS','LD_PRELOAD','DYLD_INSERT_LIBRARIES','PYTHONSTARTUP','PYTHONPATH','PYTHONHOME'):
            raise LocalAIError('INVALID_ENVIRONMENT')
        inherited[key] = value
    return {'cwd': cwd or tempfile.gettempdir(), 'env': inherited}


class _EventQueue:
    def __init__(self, size: int):
        self.values: asyncio.Queue = asyncio.Queue(size)
        self.error: LocalAIError | None = None

    def push(self, value):
        if self.error:
            return
        try:
            self.values.put_nowait(value)
        except asyncio.QueueFull:
            self.fail(LocalAIError('EVENT_GAP'))

    def fail(self, error):
        self.error = error
        while not self.values.empty():
            self.values.get_nowait()
        self.values.put_nowait(error)

    async def next(self):
        if self.error:
            raise self.error
        value = await self.values.get()
        if isinstance(value, LocalAIError):
            raise value
        return value


class AsyncStdioTransport:
    @classmethod
    async def open_broker(cls, *, executable: str, address: str, token: str, **options):
        if not isinstance(address, str) or not 0 < len(address) <= 200 or '\0' in address:
            raise LocalAIError('INVALID_REQUEST')
        if not isinstance(token, str) or not re.fullmatch(r'[0-9a-f]{64}', token) or 'args' in options:
            raise LocalAIError('INVALID_REQUEST')
        transport = await cls.open(executable=executable, args=['connect', '--ipc', address], **options)
        try:
            result = await transport.request('broker.authenticate', {'token': token})
            if not isinstance(result, dict) or result.get('authenticated') is not True:
                raise LocalAIError('LOCAL_ACCESS_DENIED')
            return transport
        except BaseException:
            await transport.close()
            raise

    @classmethod
    async def open(cls, *, executable: str, args: list[str] | None = None, cwd: str | None = None,
                   env: dict[str, str] | None = None, timeout: float = 30,
                   max_frame_bytes: int = 8388608, max_pending_requests: int = 64,
                   max_queued_events: int = 256):
        if not isinstance(executable, str) or not pathlib.Path(executable).is_absolute() or executable.lower().endswith(('.cmd', '.bat', '.ps1')):
            raise LocalAIError('INVALID_EXECUTABLE')
        args = list(args or [])
        if len(args) > 128 or any(not isinstance(a, str) or '\0' in a or len(a) > 32768 for a in args):
            raise LocalAIError('INVALID_REQUEST')
        for value, lower, upper in [(max_frame_bytes, 1024, 16777216), (max_pending_requests, 1, 1024), (max_queued_events, 1, 4096)]:
            if type(value) is not int or not lower <= value <= upper:
                raise LocalAIError('INVALID_LIMIT')
        if not isinstance(timeout, (int, float)) or not math.isfinite(timeout) or not 0 < timeout <= 3600:
            raise LocalAIError('INVALID_LIMIT')
        process_options = _process_options(cwd, env)
        try:
            process = await asyncio.create_subprocess_exec(executable, *args, **process_options,
                stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, limit=max_frame_bytes + 1)
        except (OSError, ValueError):
            raise LocalAIError('RUNTIME_MISSING') from None
        self = cls()
        self._process, self._timeout, self._max = process, timeout, max_frame_bytes
        self._max_pending, self._max_events = max_pending_requests, max_queued_events
        self._pending: dict[int, tuple[asyncio.Future, Callable | None]] = {}
        self._subscriptions: dict[str, _EventQueue] = {}
        self._sequence, self._closed, self._closing = 0, False, None
        self._reader = asyncio.create_task(self._read())
        self._stderr = asyncio.create_task(self._drain_stderr())
        return self

    def _fail(self, error: LocalAIError):
        if self._closed:
            return
        self._closed = True
        for future, _ in list(self._pending.values()):
            if not future.done():
                future.set_exception(error)
        for queue in self._subscriptions.values():
            queue.fail(error)
        if self._process.returncode is None:
            try:
                self._process.terminate()
            except ProcessLookupError:
                pass

    async def _drain_stderr(self):
        while await self._process.stderr.read(4096):
            pass

    async def _read(self):
        try:
            while not self._closed:
                try:
                    raw = await self._process.stdout.readline()
                except ValueError:
                    raise LocalAIError('FRAME_TOO_LARGE') from None
                if not raw:
                    raise LocalAIError('TRANSPORT_CLOSED')
                if len(raw) > self._max:
                    raise LocalAIError('FRAME_TOO_LARGE')
                if not raw.endswith(b'\n'):
                    raise LocalAIError('PROTOCOL_INVALID')
                value = decode_message(raw)
                if 'id' in value:
                    if type(value['id']) is not int or ('result' in value) == ('error' in value):
                        raise LocalAIError('PROTOCOL_INVALID')
                    target = self._pending.get(value['id'])
                    if target is None or target[0].done():
                        continue
                    future, hook = target
                    if 'error' in value:
                        error = value['error']
                        if not isinstance(error, dict) or type(error.get('code')) is not int:
                            raise LocalAIError('PROTOCOL_INVALID')
                        future.set_exception(remote_error(error.get('data')))
                    else:
                        try:
                            if hook:
                                hook(value['result'])
                            future.set_result(value['result'])
                        except LocalAIError as error:
                            future.set_exception(error)
                elif value.get('method') == 'events.error':
                    params = value.get('params')
                    if not isinstance(params, dict) or not isinstance(params.get('subscriptionId'), str) or len(params['subscriptionId']) > 256 or not isinstance(params.get('code'), str) or not re.fullmatch(r'[A-Z_]{1,64}', params['code']):
                        raise LocalAIError('PROTOCOL_INVALID')
                    queue = self._subscriptions.get(params['subscriptionId'])
                    if queue:
                        queue.fail(LocalAIError(params['code']))
                elif value.get('method') == 'events.event':
                    params = value.get('params')
                    if not isinstance(params, dict) or not isinstance(params.get('subscriptionId'), str) or 'event' not in params:
                        raise LocalAIError('PROTOCOL_INVALID')
                    queue = self._subscriptions.get(params['subscriptionId'])
                    if queue:
                        queue.push(params['event'])
                else:
                    raise LocalAIError('PROTOCOL_INVALID')
        except asyncio.CancelledError:
            raise
        except LocalAIError as error:
            self._fail(error)
        except Exception:
            self._fail(LocalAIError('PROTOCOL_INVALID'))

    async def request(self, method: str, params: Any = None, *, timeout: float | None = None, _on_result: Callable | None = None) -> Any:
        if self._closed:
            raise LocalAIError('CLOSED')
        if len(self._pending) >= self._max_pending:
            raise LocalAIError('TOO_MANY_REQUESTS')
        if self._sequence >= 9007199254740991:
            raise LocalAIError('REQUEST_ID_EXHAUSTED')
        duration = self._timeout if timeout is None else timeout
        if not isinstance(duration, (int, float)) or not math.isfinite(duration) or not 0 < duration <= 3600:
            raise LocalAIError('INVALID_LIMIT')
        self._sequence += 1
        identifier = self._sequence
        raw = encode_request(identifier, method, {} if params is None else params, self._max)
        future = asyncio.get_running_loop().create_future()
        self._pending[identifier] = (future, _on_result)
        try:
            async with asyncio.timeout(duration):
                self._process.stdin.write(raw)
                await self._process.stdin.drain()
                return await future
        except TimeoutError:
            raise LocalAIError('TIMEOUT') from None
        except (BrokenPipeError, ConnectionResetError):
            raise LocalAIError('TRANSPORT_CLOSED') from None
        finally:
            self._pending.pop(identifier, None)
            if not future.done():
                future.cancel()

    async def subscribe(self, query: dict | None = None) -> AsyncIterator[Any]:
        identifier, queue = None, None
        def install(result):
            nonlocal identifier, queue
            value = result.get('subscriptionId') if isinstance(result, dict) else None
            if not isinstance(value, str) or not value or len(value) > 256 or value in self._subscriptions:
                raise LocalAIError('PROTOCOL_INVALID')
            identifier, queue = value, _EventQueue(self._max_events)
            self._subscriptions[identifier] = queue
        try:
            await self.request('events.subscribe', query, _on_result=install)
            while True:
                yield await queue.next()
        finally:
            if identifier:
                self._subscriptions.pop(identifier, None)
                if not self._closed:
                    try:
                        await self.request('events.unsubscribe', {'subscriptionId':identifier}, timeout=1)
                    except LocalAIError:
                        pass

    async def close(self):
        if self._closing is None:
            async def cleanup():
                self._fail(LocalAIError('CLOSED'))
                try:
                    await asyncio.wait_for(self._process.wait(), 1)
                except TimeoutError:
                    self._process.kill()
                    await self._process.wait()
                await asyncio.gather(self._reader, self._stderr, return_exceptions=True)
            self._closing = asyncio.create_task(cleanup())
        await asyncio.shield(self._closing)
