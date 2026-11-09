import test from 'node:test';
import assert from 'node:assert/strict';
import { validateAcceptedDocumentsShape, allRequiredDocumentsAccepted } from '../src/lib/legalConsent.ts';

// ---------- validateAcceptedDocumentsShape (BFF forwarding/validation) ----------

test('a well-formed accepted_documents array passes through unchanged', () => {
  const value = [
    { type: 'DOCTOR_TERMS', version: '1.0-afyamkononi' },
    { type: 'PRIVACY_NOTICE', version: '1.0-afyamkononi' },
  ];
  assert.deepEqual(validateAcceptedDocumentsShape(value), value);
});

test('missing accepted_documents (undefined) is rejected', () => {
  assert.equal(validateAcceptedDocumentsShape(undefined), null);
});

test('an empty array is rejected -- this is the exact production bug: the field was silently absent/empty', () => {
  assert.equal(validateAcceptedDocumentsShape([]), null);
});

test('not an array at all is rejected', () => {
  assert.equal(validateAcceptedDocumentsShape('DOCTOR_TERMS'), null);
  assert.equal(validateAcceptedDocumentsShape({ type: 'DOCTOR_TERMS', version: '1.0' }), null);
  assert.equal(validateAcceptedDocumentsShape(null), null);
});

test('an item missing type or version is rejected', () => {
  assert.equal(validateAcceptedDocumentsShape([{ version: '1.0' }]), null);
  assert.equal(validateAcceptedDocumentsShape([{ type: 'DOCTOR_TERMS' }]), null);
});

test('an item with empty-string type or version is rejected', () => {
  assert.equal(validateAcceptedDocumentsShape([{ type: '', version: '1.0' }]), null);
  assert.equal(validateAcceptedDocumentsShape([{ type: 'DOCTOR_TERMS', version: '' }]), null);
});

test('an item with the wrong field types is rejected', () => {
  assert.equal(validateAcceptedDocumentsShape([{ type: 123, version: '1.0' }]), null);
  assert.equal(validateAcceptedDocumentsShape([{ type: 'DOCTOR_TERMS', version: ['1.0'] }]), null);
});

test('one malformed item invalidates the whole submission, even alongside otherwise-valid ones', () => {
  const value = [
    { type: 'DOCTOR_TERMS', version: '1.0-afyamkononi' },
    { type: 'PRIVACY_NOTICE' },
  ];
  assert.equal(validateAcceptedDocumentsShape(value), null);
});

// ---------- allRequiredDocumentsAccepted (client-side pre-submit gate) ----------

const documents = [
  { document_type: 'DOCTOR_TERMS', required: true },
  { document_type: 'PRIVACY_NOTICE', required: true },
  { document_type: 'MARKETING_COMMUNICATIONS', required: false },
];

test('all required documents checked, optional unchecked -- allowed', () => {
  assert.equal(
    allRequiredDocumentsAccepted(documents, { DOCTOR_TERMS: true, PRIVACY_NOTICE: true }),
    true,
  );
});

test('missing one required document -- blocked', () => {
  assert.equal(
    allRequiredDocumentsAccepted(documents, { DOCTOR_TERMS: true }),
    false,
  );
});

test('no documents checked at all -- blocked', () => {
  assert.equal(allRequiredDocumentsAccepted(documents, {}), false);
});

test('the optional document never blocks submission by itself', () => {
  assert.equal(
    allRequiredDocumentsAccepted(documents, { DOCTOR_TERMS: true, PRIVACY_NOTICE: true, MARKETING_COMMUNICATIONS: false }),
    true,
  );
});

test('an empty document list trivially passes -- the real gate is the backend requiring a non-empty submission', () => {
  assert.equal(allRequiredDocumentsAccepted([], {}), true);
});
