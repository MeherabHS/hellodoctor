import test from 'node:test';
import assert from 'node:assert/strict';
import { hasPrototypePollution, hasTunnelingHeader, isJsonStateChangingRequest, isPerimeterBlockedPath } from '../src/lib/sideDoorGuard.ts';

test('verb-tunneling and proxy-rewrite headers are detected regardless of value', () => {
  assert.equal(hasTunnelingHeader(new Headers()), false);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-HTTP-Method-Override': 'DELETE' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'x-http-method': 'PUT' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Method-Override': 'anything' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-HTTP-Destination': '/x' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Original-URL': '/internal/admin' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Rewrite-URL': '/x' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Forwarded-Prefix': '/x' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Forwarded-Path': '/x' })), true);
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Custom-URL-Rewrite': '/x' })), true);
  // An unrelated header never trips the guard.
  assert.equal(hasTunnelingHeader(new Headers({ 'X-Forwarded-For': '198.51.100.1' })), false);
});

test('semicolon, backslash and null-byte path tricks are blocked', () => {
  assert.equal(isPerimeterBlockedPath('/api;bypass/health'), true);
  assert.equal(isPerimeterBlockedPath('/health;'), true);
  assert.equal(isPerimeterBlockedPath('/a\\..\\b'), true);
  assert.equal(isPerimeterBlockedPath('/a\0b'), true);
  assert.equal(isPerimeterBlockedPath('/ok/path'), false);
});

test('(double-)encoded traversal sequences are blocked', () => {
  assert.equal(isPerimeterBlockedPath('/a/%2e%2e/b'), true);
  assert.equal(isPerimeterBlockedPath('/a/%252e%252e/b'), true);
  assert.equal(isPerimeterBlockedPath('/a/b'), false);
});

test('cloud metadata and internal diagnostic endpoints are blocked', () => {
  assert.equal(isPerimeterBlockedPath('/latest/meta-data/iam/credentials/'), true);
  assert.equal(isPerimeterBlockedPath('/computeMetadata/v1/'), true);
  assert.equal(isPerimeterBlockedPath('/x/169.254.169.254/y'), true);
  assert.equal(isPerimeterBlockedPath('/x/100.100.100.200'), true);
  assert.equal(isPerimeterBlockedPath('/api/v1/namespaces/default/pods'), true);
  assert.equal(isPerimeterBlockedPath('/actuator/health'), true);
  assert.equal(isPerimeterBlockedPath('/actuator'), true);
  assert.equal(isPerimeterBlockedPath('/debug/pprof/heap'), true);
});

test('backup/leftover-file and dotenv probes are blocked', () => {
  for (const path of ['/config.bak', '/x.backup', '/old.old', '/x.orig', '/x.save', '/x.swp',
    '/dump.sql', '/db.dump', '/site.tar', '/site.gz', '/site.zip', '/.env', '/.env.bak', '/.env.production']) {
    assert.equal(isPerimeterBlockedPath(path), true, path);
  }
  assert.equal(isPerimeterBlockedPath('/api/documents/download'), false);
});

test('prototype-pollution shaped keys are detected at any nesting depth', () => {
  assert.equal(hasPrototypePollution({ name: 'ok' }), false);
  assert.equal(hasPrototypePollution(JSON.parse('{"__proto__":{"isAdmin":true}}')), true);
  assert.equal(hasPrototypePollution(JSON.parse('{"a":{"b":{"constructor":{"x":1}}}}')), true);
  assert.equal(hasPrototypePollution(JSON.parse('{"a":[{"prototype":1}]}')), true);
  // A legitimate free-text value that merely *contains the word* must never
  // trip this — only an actual object *key* named exactly one of the three
  // is pollution-shaped. This is the false-positive case raw-text scanning
  // would get wrong.
  assert.equal(
    hasPrototypePollution({ message: 'Please check the constructor of this device.' }),
    false,
  );
  assert.equal(hasPrototypePollution(null), false);
  assert.equal(hasPrototypePollution('a string'), false);
  assert.equal(hasPrototypePollution(42), false);
});

test('only a JSON-bodied state-changing request is eligible for body inspection', () => {
  assert.equal(isJsonStateChangingRequest('GET', 'application/json'), false);
  assert.equal(isJsonStateChangingRequest('POST', 'application/json'), true);
  assert.equal(isJsonStateChangingRequest('post', 'application/json; charset=utf-8'), true);
  assert.equal(isJsonStateChangingRequest('PUT', 'application/json'), true);
  assert.equal(isJsonStateChangingRequest('PATCH', 'application/json'), true);
  assert.equal(isJsonStateChangingRequest('DELETE', 'application/json'), true);
  assert.equal(isJsonStateChangingRequest('POST', 'multipart/form-data'), false);
  assert.equal(isJsonStateChangingRequest('POST', null), false);
});
