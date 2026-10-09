import assert from 'node:assert/strict';
import test from 'node:test';
import { canonicalLoopbackRedirect } from '../src/lib/origin.ts';

test('localhost preview navigates to configured loopback origin without relaxing CSRF', () => {
  assert.equal(canonicalLoopbackRedirect('http://localhost:3001/', 'http://127.0.0.1:3001'), 'http://127.0.0.1:3001/');
  assert.equal(canonicalLoopbackRedirect('http://localhost:3001/?screen=queue', 'http://127.0.0.1:3001'), 'http://127.0.0.1:3001/?screen=queue');
  assert.equal(canonicalLoopbackRedirect('http://127.0.0.1:3001/', 'http://127.0.0.1:3001'), null);
  assert.equal(canonicalLoopbackRedirect('http://localhost:3001/', 'http://127.0.0.1:3001', '127.0.0.1:3001'), null);
  assert.equal(canonicalLoopbackRedirect('http://localhost:3001/', 'http://127.0.0.1:3001', 'attacker.example:3001'), null);
});

test('canonical redirect rejects foreign hosts, ports, schemes and unsafe configuration', () => {
  for (const request of ['http://attacker.example:3001/', 'http://localhost:3002/', 'https://localhost:3001/']) {
    assert.equal(canonicalLoopbackRedirect(request, 'http://127.0.0.1:3001'), null);
  }
  for (const configured of ['https://doctor.example', 'http://user:password@127.0.0.1:3001', 'http://127.0.0.1:3001/unsafe', 'invalid']) {
    assert.equal(canonicalLoopbackRedirect('http://localhost:3001/', configured), null);
  }
});
