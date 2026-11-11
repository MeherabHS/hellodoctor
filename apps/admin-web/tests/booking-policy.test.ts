import test from 'node:test';
import assert from 'node:assert/strict';
import { paymentRequiredFromPolicy } from '../src/lib/bookingPolicy.ts';

test('FINANCIAL_MODULE_MODE: the Admin UI fails safe to paymentRequired until the server explicitly defers it', () => {
  // Undefined/missing/not-yet-loaded policy must never hide real financial actions.
  assert.equal(paymentRequiredFromPolicy(undefined), true);
  assert.equal(paymentRequiredFromPolicy(null), true);
  assert.equal(paymentRequiredFromPolicy({}), true);
  assert.equal(paymentRequiredFromPolicy({ payment_required: true }), true);
  // Only an explicit `false` from the server defers the financial module.
  assert.equal(paymentRequiredFromPolicy({ payment_required: false }), false);
});
