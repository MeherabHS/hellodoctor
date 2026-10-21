import test from 'node:test';
import assert from 'node:assert/strict';
import { createChallengeVerifier, createAuthBodyReader, readAuthBody, AuthBodyError } from '../src/lib/turnstile.ts';

const now = Date.parse('2026-10-19T00:00:00Z');
const config = { environment: 'production', origin: 'https://portal.example.test', secret: 'synthetic-server-secret' };
const success = { success: true, action: 'admin_login', hostname: 'portal.example.test', challenge_ts: new Date(now).toISOString() };
const response = (data: unknown, status = 200) => (async () => Response.json(data, { status })) as typeof fetch;

test('slow auth bodies have a shared fail-fast reading bulkhead', async () => {
  const read = createAuthBodyReader(1);
  let finish!: () => void;
  const stream = new ReadableStream<Uint8Array>({ start(controller) {
    finish = () => { controller.enqueue(new TextEncoder().encode('{}')); controller.close(); };
  } });
  const request = new Request('http://localhost/login', { method: 'POST', body: stream, duplex: 'half' } as RequestInit);
  const pending = read(request);
  await assert.rejects(read(new Request('http://localhost/login', { method: 'POST', body: '{}' })),
    error => error instanceof AuthBodyError && error.status === 503);
  finish();
  assert.deepEqual(await pending, {});
  assert.deepEqual(await read(new Request('http://localhost/login', { method: 'POST', body: '{}' })), {});
});

test('challenge cost is bounded globally even for sequential different tokens', async () => {
  const verify = createChallengeVerifier(8, () => 0);
  let calls = 0;
  const send = (async () => { calls++; return Response.json(success); }) as typeof fetch;
  for (let index = 0; index < 60; index++) assert.deepEqual(await verify('proof-' + index, 'admin_login', config, send, now), { ok: true });
  assert.deepEqual(await verify('proof-overflow', 'admin_login', config, send, now), { ok: false, status: 503 });
  assert.equal(calls, 60);
});

test('only explicit development/test bypasses; deployed missing config fails closed', async () => {
  const verify = createChallengeVerifier();
  let called = false;
  const unexpected = (async () => { called = true; throw new Error(); }) as typeof fetch;
  for (const environment of ['development', 'test']) assert.deepEqual(await verify(null, 'admin_login', { environment }, unexpected), { ok: true });
  for (const environment of ['', undefined, 'prodution', 'staging', 'production']) {
    assert.deepEqual(await verify('proof', 'admin_login', { environment }, unexpected), { ok: false, status: 503 });
  }
  assert.equal(called, false);
});

test('missing, forged-format and oversized tokens are rejected before provider work', async () => {
  let called = false;
  const send = (async () => { called = true; throw new Error(); }) as typeof fetch;
  for (const token of [null, undefined, '', 17, {}, 'x'.repeat(2049)]) {
    assert.deepEqual(await createChallengeVerifier()(token, 'admin_login', config, send, now), { ok: false, status: 403 });
  }
  assert.equal(called, false);
});

test('server validates success, exact hostname/action and fresh challenge timestamp', async () => {
  const verify = createChallengeVerifier();
  assert.deepEqual(await verify('proof', 'admin_login', config, response(success), now), { ok: true });
  for (const data of [null, {}, { ...success, success: false }, { ...success, success: 'true' },
    { ...success, action: 'doctor_login' }, { ...success, hostname: 'attacker.test' },
    { ...success, challenge_ts: new Date(now - 300001).toISOString() },
    { ...success, challenge_ts: new Date(now + 30001).toISOString() }, { ...success, challenge_ts: 'invalid' }]) {
    assert.deepEqual(await verify('proof', 'admin_login', config, response(data), now), { ok: false, status: 403 });
  }
});

test('provider rejects expired/replayed proof and validation success is never cached', async () => {
  const verify = createChallengeVerifier();
  let calls = 0;
  const send = (async () => Response.json(++calls === 1 ? success : { success: false, 'error-codes': ['timeout-or-duplicate'] })) as typeof fetch;
  assert.deepEqual(await verify('same-proof', 'admin_login', config, send, now), { ok: true });
  assert.deepEqual(await verify('same-proof', 'admin_login', config, send, now), { ok: false, status: 403 });
  assert.equal(calls, 2);
});

test('provider errors, malformed response and timeout cannot allow authentication', async () => {
  for (const send of [response({}, 503),
    (async () => new Response('invalid-json')) as typeof fetch,
    (async () => { throw new DOMException('timed out', 'TimeoutError'); }) as typeof fetch]) {
    assert.deepEqual(await createChallengeVerifier()('proof', 'admin_login', config, send, now), { ok: false, status: 503 });
  }
});

test('Siteverify endpoint, redirects, cache and timeout remain bounded', async () => {
  const send = (async (url: string | URL | Request, init?: RequestInit) => {
    assert.equal(url, 'https://challenges.cloudflare.com/turnstile/v0/siteverify');
    assert.equal(init?.method, 'POST'); assert.equal(init?.redirect, 'error'); assert.equal(init?.cache, 'no-store');
    assert.ok(init?.signal instanceof AbortSignal);
    assert.deepEqual(JSON.parse(String(init?.body)), { secret: config.secret, response: 'proof' });
    return Response.json(success);
  }) as typeof fetch;
  assert.deepEqual(await createChallengeVerifier()('proof', 'admin_login', config, send, now), { ok: true });
});

test('outbound challenge saturation fails immediately and releases slots on failure', async () => {
  const verify = createChallengeVerifier(1);
  let complete!: (response: Response) => void;
  const send = (() => new Promise<Response>(resolve => { complete = resolve; })) as typeof fetch;
  const pending = verify('first', 'admin_login', config, send, now);
  assert.deepEqual(await verify('second', 'admin_login', config, response(success), now), { ok: false, status: 503 });
  complete(Response.json({}, { status: 503 }));
  assert.deepEqual(await pending, { ok: false, status: 503 });
  assert.deepEqual(await verify('third', 'admin_login', config, response(success), now), { ok: true });
});

test('official test secrets and insecure deployed origins fail closed', async () => {
  for (const change of [{ secret: '1x0000000000000000000000000000000AA' }, { origin: 'http://portal.example.test' },
    { origin: 'https://portal.example.test/path' }]) {
    assert.deepEqual(await createChallengeVerifier()('proof', 'admin_login', { ...config, ...change }, response(success), now), { ok: false, status: 503 });
  }
});

test('auth form reads are bounded even without Content-Length', async () => {
  for (const body of ['x'.repeat(16385), JSON.stringify({ password: 'x'.repeat(16385) })]) {
    await assert.rejects(readAuthBody(new Request('http://localhost/login', { method: 'POST', body })),
      error => error instanceof AuthBodyError && error.status === 413);
  }
  for (const body of ['null', '[]', 'bad-json']) {
    await assert.rejects(readAuthBody(new Request('http://localhost/login', { method: 'POST', body })),
      error => error instanceof AuthBodyError && error.status === 400);
  }
  assert.deepEqual(await readAuthBody(new Request('http://localhost/login', { method: 'POST', body: '{"email":"synthetic@example.test"}' })),
    { email: 'synthetic@example.test' });
});
