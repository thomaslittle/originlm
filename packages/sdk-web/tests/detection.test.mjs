import test from 'node:test';
import assert from 'node:assert/strict';
import {badgeLabel, detectPairing} from '../dist/index.js';

test('detection cannot imply authorization', () => {
  assert.equal(badgeLabel('pairing', ['Codex']), 'Runtime available; pairing required');
  assert.equal(badgeLabel('extension', ['Codex']), 'Extension available; connect to continue');
  assert.match(badgeLabel('pairing', ['Codex'], true), /connected/);
});

test('pairing refuses remote URLs before fetch', async () => {
  const original=globalThis.fetch;
  let calls=0;
  globalThis.fetch=async()=>{calls++;throw new Error('unexpected request');};
  try {
    for(const origin of ['https://example.com','http://127.0.0.1@evil.example','http://127.0.0.1/path','file:///tmp/runtime']) {
      assert.equal(await detectPairing(origin),false);
    }
    assert.equal(calls,0);
  } finally {globalThis.fetch=original;}
});

test('pairing requires bounded compatible JSON, not merely a reachable server', async () => {
  const original=globalThis.fetch;
  try {
    for(const body of [{}, {pairingRequired:true,protocolVersion:'2.0'}, {pairingRequired:true,protocolVersion:'1.0',padding:'x'.repeat(4096)}]) {
      globalThis.fetch=async()=>Response.json(body);
      assert.equal(await detectPairing(),false);
    }
    globalThis.fetch=async(_url, options)=>{
      assert.equal(options.credentials,'omit');
      assert.equal(options.redirect,'error');
      assert.equal(options.mode,'cors');
      return Response.json({pairingRequired:true,protocolVersion:'1.0'});
    };
    assert.equal(await detectPairing(),true);
  } finally {globalThis.fetch=original;}
});
