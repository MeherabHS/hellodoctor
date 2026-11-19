import test from 'node:test';
import assert from 'node:assert/strict';
import {confirmStepUp, isReauthRequired, registerStepUpPrompt} from '../src/lib/stepUp.mjs';

test('recognises only the backend REAUTH_REQUIRED answer', () => {
  const body = JSON.stringify({success: false, error: {code: 'REAUTH_REQUIRED', message: 'Confirm your password to continue.'}});
  assert.equal(isReauthRequired(403, body), true);
  assert.equal(isReauthRequired(401, body), false);
  assert.equal(isReauthRequired(403, JSON.stringify({error: {code: 'FORBIDDEN'}})), false);
  assert.equal(isReauthRequired(403, 'not json'), false);
});

test('without a mounted dialog the step-up is declined', async () => {
  assert.equal(await confirmStepUp(), false);
});

test('concurrent sensitive actions share one password dialog', async () => {
  let opened = 0;
  let resolve: (ok: boolean) => void = () => {};
  const unregister = registerStepUpPrompt(() => { opened++; return new Promise(r => { resolve = r; }); });
  const first = confirmStepUp();
  const second = confirmStepUp();
  resolve(true);
  assert.deepEqual(await Promise.all([first, second]), [true, true]);
  assert.equal(opened, 1);
  unregister();
  assert.equal(await confirmStepUp(), false);
});
