import test from 'node:test';
import assert from 'node:assert/strict';
import {ADMIN_SLOTS,adminFetch,adminQueueState,withAdminSlot} from '../src/lib/adminQueue.mjs';

test('no more than two admin requests run at once and every queued task completes', async () => {
  let running = 0; let peak = 0;
  const task = (ms: number) => withAdminSlot(async () => {
    running++; peak = Math.max(peak, running);
    await new Promise(r => setTimeout(r, ms));
    running--; return ms;
  });
  const results = await Promise.all([30, 10, 20, 5, 15, 25].map(task));
  assert.equal(ADMIN_SLOTS, 2);
  assert.equal(peak, 2);
  assert.deepEqual(results, [30, 10, 20, 5, 15, 25]);
  assert.deepEqual(adminQueueState(), {active: 0, waiting: 0});
});

test('a failed task frees its slot', async () => {
  await assert.rejects(withAdminSlot(async () => { throw new Error('boom'); }));
  assert.deepEqual(adminQueueState(), {active: 0, waiting: 0});
});

test('reads turned away with 503 are retried; writes and other errors are not', async () => {
  const before = globalThis.fetch;
  try {
    let calls = 0;
    globalThis.fetch = async () => new Response('', {status: ++calls < 3 ? 503 : 200});
    assert.equal((await adminFetch('/api/proxy/admin/x', {}, 2, 1)).status, 200);
    assert.equal(calls, 3);

    calls = 0;
    globalThis.fetch = async () => { calls++; return new Response('', {status: 503}); };
    assert.equal((await adminFetch('/api/proxy/admin/x', {method: 'POST'}, 2, 1)).status, 503);
    assert.equal(calls, 1, 'a write is sent once');

    calls = 0;
    globalThis.fetch = async () => { calls++; return new Response('', {status: 500}); };
    assert.equal((await adminFetch('/api/proxy/admin/x', {}, 2, 1)).status, 500);
    assert.equal(calls, 1, 'only 503 is retried');

    calls = 0;
    globalThis.fetch = async () => { calls++; return new Response('', {status: 503}); };
    assert.equal((await adminFetch('/api/proxy/admin/x', {}, 2, 1)).status, 503);
    assert.equal(calls, 3, 'gives up after the retry budget');
  } finally { globalThis.fetch = before; }
});
