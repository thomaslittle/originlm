import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLocalAI } from '../dist/index.js';

test('managed auth calls use the advertised nested RPCs only', async () => {
  const calls = [];
  const transport = {
    async request(method, params) {
      calls.push([method, params]);
      if (method === 'runtime.handshake') return {
        protocolVersion: '1.0',
        methods: ['auth.chatgpt.status', 'auth.chatgpt.cancel'],
      };
      if (method === 'auth.chatgpt.status') return {
        state: 'signed_out', loginId: null, email: null, errorCode: null,
      };
      return {};
    },
    async close() {},
  };
  const client = await createLocalAI({ transport });
  assert.equal((await client.auth.chatgpt.status()).state, 'signed_out');
  await client.auth.chatgpt.cancel({ loginId: 'login' });
  await assert.rejects(client.auth.chatgpt.start(), { code: 'CAPABILITY_UNSUPPORTED' });
  assert.deepEqual(calls.slice(1), [
    ['auth.chatgpt.status', {}],
    ['auth.chatgpt.cancel', { loginId: 'login' }],
  ]);
  await client.close();
});
