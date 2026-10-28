import asyncio
import importlib
import os
import pathlib
import subprocess
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'src'))
from originlm import AsyncStdioTransport, AsyncClient, Client, LocalAIError
PEER = str(pathlib.Path(__file__).parent / 'fixtures' / 'peer.py')
OPTIONS = dict(executable=sys.executable, args=['-I', PEER], timeout=1)

class AsyncTests(unittest.IsolatedAsyncioTestCase):
    async def open(self, **overrides):
        transport = await AsyncStdioTransport.open(**{**OPTIONS, **overrides})
        self.addAsyncCleanup(transport.close)
        return transport

    async def test_same_client_handshake_and_snapshot(self):
        t = await self.open()
        ai = await AsyncClient.connect(t)
        self.assertEqual(await ai.snapshot.get({'fields':['models']}), {'fields':['models']})

    async def test_nested_auth_rpc_identifier(self):
        t = await self.open()
        self.assertEqual(await t.request('auth.chatgpt.status', {'synthetic': True}), {'synthetic': True})
        for method in ['auth..status', '.auth.status', 'auth.status.']:
            with self.assertRaisesRegex(LocalAIError, 'INVALID_METHOD'):
                await t.request(method)

    async def test_concurrent_responses_keep_identity(self):
        t = await self.open()
        values = await asyncio.gather(t.request('test.delay', {'value':'a','delay':0.03}), t.request('test.delay', {'value':'b','delay':0.001}))
        self.assertEqual(values, ['a','b'])

    async def test_text_not_interpreted_by_shell(self):
        t = await self.open()
        text = '$(touch SHOULD_NOT_EXIST); " & whoami'
        self.assertEqual(await t.request('test.echo', {'text':text}), {'text':text})

    async def test_invalid_paths_before_spawn(self):
        with self.assertRaisesRegex(LocalAIError, 'INVALID_EXECUTABLE'):
            await self.open(executable='python')
        with self.assertRaisesRegex(LocalAIError, 'INVALID_WORKSPACE'):
            await self.open(cwd='relative')

    async def test_timeout_does_not_retry(self):
        t = await self.open()
        with self.assertRaisesRegex(LocalAIError, 'TIMEOUT'):
            await t.request('test.hang', timeout=0.02)
        self.assertEqual(await t.request('test.echo', {'ready':True}), {'ready':True})

    async def test_cancelled_task_does_not_break_later_requests(self):
        t = await self.open()
        task = asyncio.create_task(t.request('test.hang'))
        await asyncio.sleep(0)
        task.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await task
        self.assertEqual(await t.request('test.echo', {'ready':True}), {'ready':True})

    async def test_malformed_output_fails_closed(self):
        t = await self.open()
        with self.assertRaisesRegex(LocalAIError, 'PROTOCOL_INVALID'):
            await t.request('test.bad')
        with self.assertRaisesRegex(LocalAIError, 'CLOSED'):
            await t.request('test.echo')

    async def test_unterminated_output_limit(self):
        t = await self.open(max_frame_bytes=1024)
        with self.assertRaisesRegex(LocalAIError, 'FRAME_TOO_LARGE'):
            await t.request('test.large')

    async def test_outgoing_limit(self):
        t = await self.open(max_frame_bytes=1024)
        with self.assertRaisesRegex(LocalAIError, 'FRAME_TOO_LARGE'):
            await t.request('test.echo', {'text':'x'*2000})

    async def test_peer_exit_rejects_pending_requests(self):
        t = await self.open()
        with self.assertRaisesRegex(LocalAIError, 'TRANSPORT_CLOSED'):
            await t.request('test.exit')

    async def test_native_error_is_redacted(self):
        t = await self.open()
        with self.assertRaises(LocalAIError) as caught:
            await t.request('test.error')
        self.assertEqual(caught.exception.code, 'AUTH_REQUIRED')
        self.assertNotIn('SECRET_NATIVE_LOG', repr(caught.exception))

    async def test_retry_metadata_is_preserved_without_private_error_details(self):
        t = await self.open()
        with self.assertRaises(LocalAIError) as caught:
            await t.request('test.retry')
        self.assertEqual(caught.exception.code, 'RATE_LIMITED')
        self.assertTrue(caught.exception.retryable)
        self.assertEqual(caught.exception.retry_after_ms, 2750)
        self.assertNotIn('PRIVATE', repr(caught.exception))

    async def test_events_after_ack_are_not_lost(self):
        t = await self.open()
        events = t.subscribe()
        self.assertEqual([await anext(events) for _ in range(3)], [{'sequence':'1'},{'sequence':'2'},{'sequence':'3'}])
        await events.aclose()

    async def test_slow_consumer_gets_gap_not_silent_loss(self):
        t = await self.open(max_queued_events=2)
        events = t.subscribe({'after':'manual'})
        await anext(events)
        await t.request('test.flood')
        with self.assertRaisesRegex(LocalAIError, 'EVENT_GAP'):
            await anext(events)

    async def test_method_not_advertised_is_unavailable(self):
        t = await self.open()
        ai = await AsyncClient.connect(t)
        with self.assertRaisesRegex(LocalAIError, 'CAPABILITY_UNSUPPORTED'):
            await ai.turns.start({'prompt':'must not execute'})

    async def test_inputs_put_is_gated_by_handshake(self):
        t = await self.open()
        ai = await AsyncClient.connect(t)
        with self.assertRaisesRegex(LocalAIError, 'CAPABILITY_UNSUPPORTED'):
            await ai.inputs.put({'sessionId':'s','mediaType':'image/png','dataBase64':'AAAA'})

    async def test_close_idempotent(self):
        t = await self.open()
        pending = asyncio.create_task(t.request('test.hang'))
        await asyncio.sleep(0)
        await t.close()
        with self.assertRaisesRegex(LocalAIError, 'CLOSED'):
            await pending
        await t.close()

    async def test_environment_does_not_inherit_loader_hooks(self):
        with patch.dict(os.environ, {'PYTHONSTARTUP':'danger.py'}):
            t = await self.open(env={'EXPLICIT':'ok'})
            self.assertEqual(await t.request('test.env'), {'injected':None,'explicit':'ok'})

class SyncTests(unittest.TestCase):
    def test_sync_facade_uses_same_async_transport(self):
        with Client.from_stdio(**OPTIONS) as ai:
            self.assertEqual(ai.snapshot.get({'hello':'world'}), {'hello':'world'})

    def test_import_has_no_process_or_network_side_effect(self):
        with patch('subprocess.Popen', side_effect=AssertionError('spawn on import')), patch('socket.socket', side_effect=AssertionError('network on import')):
            import originlm
            importlib.reload(originlm)
