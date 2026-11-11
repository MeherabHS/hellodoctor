import test from 'node:test';
import assert from 'node:assert/strict';
import {backendTransactionToStoreItem} from '../src/lib/financeProjection.ts';

for (const status of ['INITIATED', 'FAILED']) {
  test(`${status} is an unfunded attempt, excluded from inflow and escrow`, () => {
    const row = backendTransactionToStoreItem({payment_status:status,gross_amount:'800'});
    assert.equal(row.direction, 'NONE');
    assert.equal(row.escrowStatus, 'UNFUNDED');
  });
}
test('completed refund is a refund, never held escrow', () => {
  const row=backendTransactionToStoreItem({payment_status:'REFUNDED_TO_PATIENT'});
  assert.equal(row.direction,'REFUND');
  assert.equal(row.escrowStatus,'REFUNDED');
});
test('approved refund remains held until confirmed completion', () => {
  const row=backendTransactionToStoreItem({payment_status:'REFUND_PENDING'});
  assert.equal(row.direction,'NONE');
  assert.equal(row.escrowStatus,'ESCROW_LOCKED');
});
test('confirmed held funds remain visible, including late reconciliation obligations', () => {
  const row=backendTransactionToStoreItem({payment_status:'PAYMENT_HELD',reconciliation_required:true,gross_amount:'800'});
  assert.equal(row.direction,'INFLOW');
  assert.equal(row.escrowStatus,'ESCROW_LOCKED');
  assert.equal(row.grossAmount,800);
  assert.match(row.escrowStatusLabel,/reconciliation/i);
});
test('settlement and completed disbursement are not held escrow', () => {
  assert.equal(backendTransactionToStoreItem({payment_status:'SETTLED_TO_DOCTOR'}).escrowStatus,'SETTLED');
  const paid=backendTransactionToStoreItem({payment_status:'DISBURSED'});
  assert.equal(paid.escrowStatus,'SETTLED');
  assert.equal(paid.direction,'PAYOUT');
});
test('unknown financial state cannot silently become an inflow', () => {
  assert.throws(()=>backendTransactionToStoreItem({payment_status:'UNRECOGNIZED'}),/Unsupported payment state/);
});
