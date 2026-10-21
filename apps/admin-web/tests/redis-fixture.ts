import assert from 'node:assert/strict';

// A logical database number is not isolation: require the disposable fixture's
// explicit address and ownership marker before making any test connection.
export function redisFixtureConfiguration(environment: NodeJS.ProcessEnv) {
  const address = environment.TEST_REDIS_URL;
  const fixtureId = environment.TEST_REDIS_FIXTURE_ID;
  assert.ok(address, 'TEST_REDIS_URL must name an explicitly provisioned disposable Redis fixture');
  assert.ok(fixtureId && /^[a-zA-Z0-9_-]{12,100}$/.test(fixtureId), 'TEST_REDIS_FIXTURE_ID must identify the disposable fixture');
  const url = new URL(address);
  assert.equal(url.protocol, 'redis:');
  assert.equal(url.hostname, '127.0.0.1', 'Redis tests require literal loopback');
  assert.ok(url.port && url.port !== '6379', 'Redis tests reject the shared/default Redis port');
  assert.equal(url.pathname, '/14', 'Admin refresh tests use only fixture database 14');
  assert.equal(url.username + url.password + url.search + url.hash, '', 'Redis fixture URL must not include credentials or options');
  return { address, fixtureId };
}

export function assertRedisFixtureOwner(actual: string | null, expected: string) {
  assert.equal(actual, expected, 'Redis ownership marker must match TEST_REDIS_FIXTURE_ID');
}
