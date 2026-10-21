import test from 'node:test';
import assert from 'node:assert/strict';
import { redisFixtureConfiguration, assertRedisFixtureOwner } from './redis-fixture.ts';

const configuration = { TEST_REDIS_URL: 'redis://127.0.0.1:16380/14', TEST_REDIS_FIXTURE_ID: 'synthetic-fixture-owner' };
test('Redis fixture configuration has no implicit shared-service fallback', () => {
  assert.throws(() => redisFixtureConfiguration({}), /TEST_REDIS_URL/);
  assert.throws(() => redisFixtureConfiguration({ TEST_REDIS_URL: configuration.TEST_REDIS_URL }), /TEST_REDIS_FIXTURE_ID/);
});
test('Redis fixture rejects shared ports, remote hosts and credentials before connection', () => {
  for (const address of ['redis://127.0.0.1:6379/14', 'redis://localhost:16380/14', 'redis://192.0.2.1:16380/14', 'redis://secret@127.0.0.1:16380/14', 'redis://127.0.0.1:16380/0']) {
    assert.throws(() => redisFixtureConfiguration({ ...configuration, TEST_REDIS_URL: address }));
  }
});
test('Redis fixture requires an exact ownership marker', () => {
  assert.throws(() => assertRedisFixtureOwner(null, configuration.TEST_REDIS_FIXTURE_ID));
  assert.throws(() => assertRedisFixtureOwner('different-owner', configuration.TEST_REDIS_FIXTURE_ID));
  assertRedisFixtureOwner(configuration.TEST_REDIS_FIXTURE_ID, configuration.TEST_REDIS_FIXTURE_ID);
});
test('Redis fixture accepts an explicit disposable loopback endpoint', () => {
  assert.deepEqual(redisFixtureConfiguration(configuration), { address: configuration.TEST_REDIS_URL, fixtureId: configuration.TEST_REDIS_FIXTURE_ID });
});
