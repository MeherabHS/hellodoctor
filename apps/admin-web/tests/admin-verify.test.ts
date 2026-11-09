import test from 'node:test';
import assert from 'node:assert/strict';
import { validateAdminVerifyBody } from '../src/lib/adminVerify.ts';

// Admin, Doctor and Patient share one single-factor-OTP MFA shape (the
// backend's AdminVerifyOtpRequest has only session_token and otp_code — no
// totp_code field exists server-side at all). A valid body must pass with
// only those two fields, regardless of any legacy authenticator-code value
// a stale client might still send, and regardless of environment — there is
// no production-only extra-factor gate here anymore.
test('a valid session_token/otp_code pair passes with no authenticator code required', () => {
  const result = validateAdminVerifyBody({ session_token: 'a'.repeat(32), otp_code: '123456' });
  assert.deepEqual(result, { session_token: 'a'.repeat(32), otp_code: '123456' });
});

test('an extraneous totp_code field is silently ignored, never required', () => {
  const withTotp = validateAdminVerifyBody({ session_token: 's', otp_code: '123456', totp_code: '999999' });
  const withoutTotp = validateAdminVerifyBody({ session_token: 's', otp_code: '123456' });
  assert.deepEqual(withTotp, withoutTotp);
  assert.deepEqual(withoutTotp, { session_token: 's', otp_code: '123456' });
});

test('missing or malformed fields are still rejected', () => {
  assert.deepEqual(validateAdminVerifyBody({}), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody({ session_token: 's' }), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody({ session_token: 's', otp_code: 'abcdef' }), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody({ session_token: '', otp_code: '123456' }), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody({ session_token: 'a'.repeat(257), otp_code: '123456' }), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody(null), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody([]), { error: 'session_token and otp_code are required.', status: 400 });
  assert.deepEqual(validateAdminVerifyBody('not an object'), { error: 'session_token and otp_code are required.', status: 400 });
});
