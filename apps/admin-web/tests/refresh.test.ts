import test from 'node:test';
import assert from 'node:assert/strict';
import { createClient } from 'redis';
import { createHash } from 'crypto';
import { refreshCoordinator } from '../src/lib/refresh.ts';
import { redisFixtureConfiguration, assertRedisFixtureOwner } from './redis-fixture.ts';
test('parallel Admin reads rotate one single-use refresh token once',async()=>{
  const rotate=refreshCoordinator(20);let calls=0;
  const issue=async()=>{calls++;await new Promise(resolve=>setTimeout(resolve,5));return {access_token:'synthetic-access',refresh_token:'synthetic-next'};};
  const results=await Promise.all([rotate('family-A',issue),rotate('family-A',issue),rotate('family-A',issue)]);
  assert.equal(calls,1);assert.deepEqual(results[0],results[1]);
  await rotate('family-B',issue);assert.equal(calls,2);
});

// h13-adminweb-refresh-coordinator-process-local: cross-process coordination.
// These exercise the Redis-backed lock path by pointing two INDEPENDENT
// refreshCoordinator() closures (standing in for two separate admin-web
// instances — the in-memory Map in refresh.ts is otherwise process-local) at
// an explicitly provisioned disposable Redis. A logical DB alone does not
// protect shared services; validate its address and ownership marker first.
//
// Each test creates and explicitly disconnects its own client(s) — and
// passes them into refreshCoordinator()'s test-only override parameter —
// rather than relying on process.env.REDIS_URL plus the module's internal
// lazy-connect-forever singleton, so the test process can exit cleanly.
const { address: REDIS_URL, fixtureId } = redisFixtureConfiguration(process.env);
const probe = createClient({ url: REDIS_URL, socket: { connectTimeout: 500, reconnectStrategy: false } });
probe.on('error', () => {});
try {
  await probe.connect();
  assertRedisFixtureOwner(await probe.get('hd:test-fixture:owner'), fixtureId);
} finally {
  if (probe.isOpen) await probe.disconnect();
}

test('two admin-web instances racing the same refresh token coordinate to exactly one rotate() call, and the loser never attempts its own rotate', async () => {
  const token = `race-token-${Date.now()}-${Math.random()}`;
  let calls = 0;
  const issue = async () => { calls++; await new Promise(r => setTimeout(r, 150)); return { access_token: 'a', refresh_token: 'b' }; };

  // Two SEPARATE Redis connections + coordinator closures = two separate
  // "processes": neither has the other's in-memory Map, so any dedupe here
  // can only come from Redis.
  const clientA = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  const clientB = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  await Promise.all([clientA.connect(), clientB.connect()]);
  const instanceA = refreshCoordinator(20, clientA as never);
  const instanceB = refreshCoordinator(20, clientB as never);

  const [resA, resB] = await Promise.all([instanceA(token, issue), instanceB(token, issue)]);
  assert.equal(calls, 1, 'rotate() must be called exactly once across both instances');
  const outcomes = [resA.outcome, resB.outcome].sort();
  assert.deepEqual(outcomes, ['raced', 'rotated']);
  await Promise.all([clientA.disconnect(), clientB.disconnect()]);
});

test('a losing instance never relays the leader\'s tokens and the leader\'s own outcome is unaffected by the race', async () => {
  const token = `race-token-2-${Date.now()}-${Math.random()}`;
  const issue = async () => { await new Promise(r => setTimeout(r, 100)); return { access_token: 'leader-access', refresh_token: 'leader-refresh' }; };
  const clientA = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  const clientB = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  await Promise.all([clientA.connect(), clientB.connect()]);
  const instanceA = refreshCoordinator(20, clientA as never);
  const instanceB = refreshCoordinator(20, clientB as never);
  const [resA, resB] = await Promise.all([instanceA(token, issue), instanceB(token, issue)]);
  const leader = resA.outcome === 'rotated' ? resA : resB;
  const loser = resA.outcome === 'rotated' ? resB : resA;
  assert.equal(leader.outcome, 'rotated');
  assert.deepEqual('tokens' in leader ? leader.tokens : null, { access_token: 'leader-access', refresh_token: 'leader-refresh' });
  assert.equal(loser.outcome, 'raced');
  assert.ok(!('tokens' in loser), 'a raced outcome must never carry token material — the caller must not set cookies from it');
  await Promise.all([clientA.disconnect(), clientB.disconnect()]);
});

test('a crashed leader\'s lock expires on its own TTL and never permanently blocks a follower', async () => {
  const token = `crash-token-${Date.now()}-${Math.random()}`;
  const fp = createHash('sha256').update(token).digest('hex');
  const client = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  await client.connect();
  // Simulate a leader that acquired the lock and then crashed before ever
  // releasing it — set the real lock key directly with the real short TTL,
  // bypassing the coordinator entirely.
  await client.set(`admin-web:refresh-lock:${fp}`, 'dead-instance', { NX: true, PX: 500 });

  const instanceB = refreshCoordinator(20, client as never);
  const start = Date.now();
  const result = await instanceB(token, async () => ({ access_token: 'x', refresh_token: 'y' }));
  const elapsedMs = Date.now() - start;

  // The follower's bounded wait (LOCK_WAIT_BUDGET_MS, 3000ms) must return —
  // never hang — regardless of whether it won the subsequently-expired lock.
  assert.ok(elapsedMs < 4000, `follower must not block beyond its wait budget (took ${elapsedMs}ms)`);
  assert.ok(result.outcome === 'raced' || result.outcome === 'rotated');
  await client.del(`admin-web:refresh-lock:${fp}`).catch(() => {});
  await client.disconnect();
});

test('no raw refresh token ever appears as a Redis key or value during coordination', async () => {
  const rawToken = `super-secret-raw-refresh-token-${Date.now()}-${Math.random()}`;
  const clientA = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  const clientB = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  const inspector = createClient({ url: REDIS_URL, socket: { reconnectStrategy: false } });
  await Promise.all([clientA.connect(), clientB.connect(), inspector.connect()]);
  const instanceA = refreshCoordinator(20, clientA as never);
  const instanceB = refreshCoordinator(20, clientB as never);

  const pending = Promise.all([
    instanceA(rawToken, async () => { await new Promise(r => setTimeout(r, 200)); return { access_token: 'a', refresh_token: 'b' }; }),
    instanceB(rawToken, async () => ({ access_token: 'a', refresh_token: 'b' })),
  ]);
  // Poll Redis for every key touching the coordination namespace WHILE the
  // race is in flight, and confirm none of them is or contains the raw token.
  const seenKeys = new Set<string>();
  const seenValues = new Set<string>();
  for (let i = 0; i < 5; i++) {
    const keys = await inspector.keys('admin-web:refresh-lock:*');
    for (const k of keys) {
      seenKeys.add(k);
      const v = await inspector.get(k).catch(() => null);
      if (v) seenValues.add(v);
    }
    await new Promise(r => setTimeout(r, 40));
  }
  await pending;

  for (const k of seenKeys) assert.ok(!k.includes(rawToken), `lock key must not contain the raw token: ${k}`);
  for (const v of seenValues) assert.ok(!v.includes(rawToken), `lock value must not contain the raw token: ${v}`);
  await Promise.all([clientA.disconnect(), clientB.disconnect(), inspector.disconnect()]);
});
