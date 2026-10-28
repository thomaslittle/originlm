import asyncio
import os
import pathlib
import stat
import sys
import tempfile
import unittest
from unittest.mock import AsyncMock, MagicMock, patch

import importlib.util

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'src'))
from originlm import LocalAIError, default_address, default_token_file, resolve_helper  # noqa: E402
import originlm.client as client_module  # noqa: E402


class HelperResolutionTests(unittest.TestCase):
    def test_explicit_helper_wins(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = pathlib.Path(directory) / 'originlm'
            binary.write_text('#!/bin/sh\n')
            binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
            self.assertEqual(resolve_helper(str(binary)), str(binary))

    def test_invalid_explicit_helper_is_rejected(self):
        with self.assertRaises(LocalAIError) as caught:
            resolve_helper('relative/helper')
        self.assertEqual(caught.exception.code, 'INVALID_EXECUTABLE')
        with self.assertRaises(LocalAIError) as caught:
            resolve_helper('/bin/sh.cmd')
        self.assertEqual(caught.exception.code, 'INVALID_EXECUTABLE')

    def test_environment_override(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = pathlib.Path(directory) / 'originlm'
            binary.write_text('#!/bin/sh\n')
            binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
            with patch.dict(os.environ, {'LAI_HELPER_PATH': str(binary)}):
                self.assertEqual(resolve_helper(), str(binary))
            with patch.dict(os.environ, {'LAI_HELPER_PATH': str(binary) + '.missing'}):
                with self.assertRaises(LocalAIError) as caught:
                    resolve_helper()
                self.assertEqual(caught.exception.code, 'INVALID_EXECUTABLE')

    def test_missing_bundled_binary_is_runtime_missing(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(LocalAIError) as caught:
                resolve_helper()
            self.assertEqual(caught.exception.code, 'RUNTIME_MISSING')

    def test_defaults_follow_environment(self):
        with patch.dict(os.environ, {'LAI_BROKER_ADDRESS': '/tmp/x.sock', 'LAI_BROKER_TOKEN_FILE': '/tmp/t.json'}):
            self.assertEqual(default_address(), '/tmp/x.sock')
            self.assertEqual(default_token_file(), pathlib.Path('/tmp/t.json'))
        preserved = {
            key: value
            for key, value in os.environ.items()
            if key not in ('LAI_BROKER_ADDRESS', 'LAI_BROKER_TOKEN_FILE')
        }
        with patch.dict(os.environ, preserved, clear=True):
            self.assertTrue(default_address().endswith('user.sock') or default_address() == 'originlm-user')
            self.assertTrue(str(default_token_file()).endswith('broker-token.json'))

    def test_shared_token_rejects_group_readable_descriptor(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'broker-token.json'
            path.write_text('{"address":"/tmp/x.sock","token":"' + 'a' * 64 + '"}')
            path.chmod(0o644)
            if os.name == 'nt':
                # Windows has no owner-only mode bit to inspect; the descriptor
                # still has to match the address and carry a real token.
                self.assertEqual(client_module._shared_token('/tmp/x.sock', path), 'a' * 64)
                self.assertIsNone(client_module._shared_token('/tmp/other.sock', path))
            else:
                self.assertIsNone(client_module._shared_token('/tmp/x.sock', path))
                path.chmod(0o600)
                self.assertEqual(client_module._shared_token('/tmp/x.sock', path), 'a' * 64)
                self.assertIsNone(client_module._shared_token('/tmp/other.sock', path))

    def test_shared_token_rejects_non_objects_oversize_and_links(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'grant.json'
            for content in ('[]', 'null', 'x' * 16385):
                path.write_text(content)
                path.chmod(0o600)
                self.assertIsNone(client_module._shared_token('/tmp/x.sock', path))
            if os.name != 'nt':
                link = pathlib.Path(directory) / 'link.json'
                link.symlink_to(path)
                self.assertIsNone(client_module._shared_token('/tmp/x.sock', link))


class PrivateLifecycleTests(unittest.IsolatedAsyncioTestCase):
    async def test_private_broker_uses_the_same_safe_environment_and_cwd_as_the_bridge(self):
        broker = MagicMock()
        broker.returncode = None
        broker.stdout.readline = AsyncMock(return_value=b'invalid descriptor')
        broker.wait = AsyncMock(return_value=0)
        with tempfile.TemporaryDirectory() as directory:
            with (patch.dict(os.environ, {'NODE_OPTIONS': '--injected', 'UNRELATED_SECRET': 'synthetic', 'XDG_STATE_HOME': directory, 'LOCALAPPDATA': directory, 'APPDATA': directory}),
                  patch.object(asyncio, 'create_subprocess_exec', AsyncMock(return_value=broker)) as spawn):
                with self.assertRaises(LocalAIError):
                    await client_module._open_private('/fixture/helper', {'cwd': directory, 'env': {'HOME': directory}})
                self.assertEqual(spawn.call_args.kwargs['cwd'], directory)
                self.assertEqual(spawn.call_args.kwargs['env']['HOME'], directory)
                for key in ('XDG_STATE_HOME', 'LOCALAPPDATA', 'APPDATA'):
                    self.assertEqual(spawn.call_args.kwargs['env'][key], directory)
                self.assertNotIn('NODE_OPTIONS', spawn.call_args.kwargs['env'])
                self.assertNotIn('UNRELATED_SECRET', spawn.call_args.kwargs['env'])

    async def test_every_startup_failure_reaps_the_owned_broker(self):
        for failure in (ValueError('oversized frame'), asyncio.CancelledError()):
            with self.subTest(failure=type(failure).__name__):
                broker = MagicMock()
                broker.returncode = None
                broker.stdout.readline = AsyncMock(side_effect=failure)
                broker.wait = AsyncMock(return_value=0)
                with patch.object(asyncio, 'create_subprocess_exec', AsyncMock(return_value=broker)):
                    with self.assertRaises((LocalAIError, asyncio.CancelledError)):
                        await client_module._open_private('/fixture/helper', {})
                broker.wait.assert_awaited()
                self.assertTrue(broker.stdin.close.called or broker.kill.called)

    async def test_bridge_failure_reaps_the_owned_broker(self):
        broker = MagicMock()
        broker.returncode = None
        broker.stdout.readline = AsyncMock(return_value=(
            '{"protocolVersion":"1.0","address":"fixture","token":"' + 'a' * 64 + '"}\n'
        ).encode())
        broker.wait = AsyncMock(return_value=0)
        with (patch.object(client_module, '_unique_address', return_value='fixture'),
              patch.object(asyncio, 'create_subprocess_exec', AsyncMock(return_value=broker)),
              patch.object(client_module.AsyncStdioTransport, 'open_broker',
                           AsyncMock(side_effect=LocalAIError('LOCAL_ACCESS_DENIED')))):
            with self.assertRaises(LocalAIError):
                await client_module._open_private('/fixture/helper', {})
        broker.wait.assert_awaited()


class SharedFallbackTests(unittest.IsolatedAsyncioTestCase):
    async def test_protocol_mismatch_closes_shared_before_starting_private(self):
        shared = MagicMock()
        shared.request = AsyncMock(return_value={'protocolVersion': '2.0', 'methods': []})
        shared.close = AsyncMock()
        private = AsyncMock(return_value=MagicMock())
        with (patch.object(client_module, 'resolve_helper', return_value='/fixture/helper'),
              patch.object(client_module.AsyncStdioTransport, 'open_broker', AsyncMock(return_value=shared)) as opening,
              patch.object(client_module, '_open_private', private)):
            result = await client_module.open_auto_transport(token='a' * 64, passive=False)
            self.assertEqual(result.fallback_reason, 'FALLBACK_PROTOCOL_MISMATCH')
            shared.close.assert_awaited_once()
            private.assert_awaited_once()
            self.assertNotIn('passive', opening.call_args.kwargs)

    async def test_denied_grant_never_starts_another_runtime(self):
        with (patch.object(client_module, 'resolve_helper', return_value='/fixture/helper'),
              patch.object(client_module.AsyncStdioTransport, 'open_broker', AsyncMock(side_effect=LocalAIError('LOCAL_ACCESS_DENIED'))),
              patch.object(client_module, '_open_private', AsyncMock()) as private):
            with self.assertRaises(LocalAIError):
                await client_module.open_auto_transport(token='a' * 64)
            private.assert_not_awaited()


if __name__ == '__main__':
    unittest.main()
