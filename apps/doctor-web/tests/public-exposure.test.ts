import test from 'node:test';
import assert from 'node:assert/strict';
import { exposureHeaders, developmentOtpAllowed, ExposureError } from '../src/lib/exposure.ts';
const safe = { environment: 'production', origin: 'https://portal.example.test', proxySecret: 'synthetic-ingress-test-credential-123456' };
function ingress() { return new Headers({ 'x-hellodoctor-proxy': safe.proxySecret, 'x-forwarded-proto': 'https', 'x-forwarded-for': '198.51.100.12' }); }
function rejects(headers: Headers, config: typeof safe, status: number) {
  assert.throws(() => exposureHeaders(headers, config), error => error instanceof ExposureError && error.status === status);
}
test('missing or unknown environment fails closed', () => {
  rejects(ingress(), { ...safe, environment: '' }, 503);
  rejects(ingress(), { ...safe, environment: 'prodution' }, 503);
});
test('production requires HTTPS canonical origin and a private ingress credential', () => {
  rejects(ingress(), { ...safe, origin: 'http://portal.example.test' }, 503);
  rejects(ingress(), { ...safe, proxySecret: '' }, 503);
});
test('forged forwarded headers without ingress authentication are denied', () => {
  const h = ingress(); h.delete('x-hellodoctor-proxy'); rejects(h, safe, 403);
  h.set('x-hellodoctor-proxy', 'attacker'); rejects(h, safe, 403);
});
test('HTTP ingress and ambiguous proxy chains are denied', () => {
  const h = ingress(); h.set('x-forwarded-proto', 'http'); rejects(h, safe, 403);
  h.set('x-forwarded-proto', 'https'); h.set('x-forwarded-for', '198.51.100.1, 127.0.0.1'); rejects(h, safe, 403);
  h.set('x-forwarded-for', '999.0.0.1'); rejects(h, safe, 403);
});
test('authenticated TLS ingress forwards only validated transport metadata', () => {
  const result = exposureHeaders(ingress(), safe);
  assert.deepEqual(result, { 'X-Forwarded-Proto': 'https', 'X-Forwarded-For': '198.51.100.12' });
  assert.equal('X-HelloDoctor-Proxy' in result, false);
});
test('a single IPv6 client address is supported', () => {
  const h = ingress(); h.set('x-forwarded-for', '2001:db8::12');
  assert.equal(exposureHeaders(h, safe)['X-Forwarded-For'], '2001:db8::12');
});
test('explicit development ignores attacker-supplied proxy metadata', () => {
  assert.deepEqual(exposureHeaders(ingress(), { environment: 'development' }), {});
});
test('production rejects dev OTP and development requires its explicit flag', () => {
  rejects(ingress(), { ...safe, devOtp: 'true' } as typeof safe, 503);
  assert.equal(developmentOtpAllowed({ environment: 'production', devOtp: 'true' }), false);
  assert.equal(developmentOtpAllowed({ environment: 'development' }), false);
  assert.equal(developmentOtpAllowed({ environment: 'development', devOtp: 'true' }), true);
});
