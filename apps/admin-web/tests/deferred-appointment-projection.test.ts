import test from 'node:test';
import assert from 'node:assert/strict';
import {
  backendDocToStoreItem,
  backendPatientToStoreItem,
  backendGrievanceToStoreItem,
} from '../src/lib/adminProjection.ts';

// A deferred (payment_policy='NOT_REQUIRED') appointment still carries a
// nominal consultation_fee (see appointment_service.rs::
// book_appointment_deferred's own comment on why) even though it was never
// actually charged. Every Admin Web view that surfaces a per-encounter fee
// must distinguish this, or it misrepresents a deferred consultation as real
// revenue — found during the FINANCIAL_MODULE_MODE audit.

test('doctor dossier never shows a deferred encounter\'s nominal fee as real revenue', () => {
  const doc = backendDocToStoreItem({
    id: 'd1',
    full_name: 'Dr. Test',
    encounters: [
      { modality: 'VIDEO', consultation_fee: '500', payment_policy: 'NOT_REQUIRED' },
      { modality: 'CHAT', consultation_fee: '300', payment_policy: 'REQUIRED' },
    ],
  }, 0);
  assert.equal(doc.encounters[0].fee, 'Deferred');
  assert.equal(doc.encounters[1].fee, 'KES  300');
});

test('patient total spend excludes deferred encounters from its sum', () => {
  const pt = backendPatientToStoreItem({
    id: 'p1',
    full_name: 'Patient Test',
    encounters: [
      { modality: 'VIDEO', consultation_fee: '500', payment_policy: 'NOT_REQUIRED' },
      { modality: 'CHAT', consultation_fee: '300', payment_policy: 'REQUIRED' },
    ],
  }, 0);
  assert.equal(pt.totalSpend, 300);
});

test('grievance detail never shows a deferred appointment\'s fee as charged', () => {
  const deferred = backendGrievanceToStoreItem({
    id: 'g1',
    status: 'PENDING_REVIEW',
    consultation_fee: '500',
    appointment_payment_policy: 'NOT_REQUIRED',
  });
  assert.equal(deferred.doctor.fee, 'Deferred');

  const paid = backendGrievanceToStoreItem({
    id: 'g2',
    status: 'PENDING_REVIEW',
    consultation_fee: '500',
    appointment_payment_policy: 'REQUIRED',
  });
  assert.equal(paid.doctor.fee, 'KES 500');
});
