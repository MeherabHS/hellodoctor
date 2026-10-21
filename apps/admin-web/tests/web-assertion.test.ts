import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash, createHmac } from 'node:crypto';
import { webAuthAssertion } from '../src/lib/web-assertion.ts';

const secret = 'synthetic-private-web-assertion-secret-123456';
test('deployed Admin assertion binds method, path and exact serialized credential body', () => {
  const body = '{"email":"synthetic@example.test","password":"synthetic"}';
  const timestamp = 1792368000;
  const nonce = '0123456789abcdef0123456789abcdef';
  const proof = webAuthAssertion(body, secret, 'production', timestamp, nonce)['x-hellodoctor-web-proof'];
  const digest = createHash('sha256').update(body).digest('hex');
  const expected = createHmac('sha256', secret).update(`v1\n${timestamp}\n${nonce}\nPOST\n/api/v1/auth/admin/login\n${digest}`).digest('hex');
  assert.equal(proof, `v1.${timestamp}.${nonce}.${expected}`);
  assert.notEqual(webAuthAssertion(body + ' ', secret, 'production', timestamp, nonce)['x-hellodoctor-web-proof'], proof);
});

test('Admin assertion configuration fails closed outside explicit development/test', () => {
  for (const environment of [undefined, 'production', 'staging', 'prodution']) {
    assert.throws(() => webAuthAssertion('{}', undefined, environment));
  }
  assert.throws(() => webAuthAssertion('{}', 'short', 'production'));
  for (const environment of ['development', 'test']) assert.deepEqual(webAuthAssertion('{}', undefined, environment), {});
});

test('new Admin assertions get independent random nonces', () => {
  assert.notEqual(webAuthAssertion('{}', secret, 'production')['x-hellodoctor-web-proof'],
    webAuthAssertion('{}', secret, 'production')['x-hellodoctor-web-proof']);
});
