import assert from 'node:assert/strict';
import test from 'node:test';
import { sanitize } from '../src/lib/sanitize.ts';

test('sanitize redacts sensitive keys including pluralized/suffixed forms', () => {
  const input = {
    token: 'a', tokens: 'b', access_token: 'c', access_tokens: 'd',
    refresh_token: 'e', refresh_tokens: 'f', client_secret: 'g', client_secrets: 'h',
    api_key: 'i', api_keys: 'j', authorization: 'k', password: 'l',
    object_key: 'm', object_bucket: 'n', encrypted: 'o', storage_path: 'p',
  };
  const out = sanitize(input) as Record<string, unknown>;
  for (const key of Object.keys(input)) assert.equal(key in out, false, `expected "${key}" to be redacted`);
});

test('sanitize does not over-redact unrelated ordinary words', () => {
  const input = { appointment_id: 'a', doctor_name: 'b', consultation_status: 'c', screen: 'd', backend_reference: 'e' };
  assert.deepEqual(sanitize(input), input);
});

test('sanitize recurses into nested objects and arrays', () => {
  assert.deepEqual(sanitize({ data: [{ access_tokens: 'x', id: 1 }] }), { data: [{ id: 1 }] });
});
