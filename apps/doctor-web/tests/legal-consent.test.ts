import test from 'node:test';
import assert from 'node:assert/strict';
import { validateAcceptedDocumentsShape, allRequiredDocumentsAccepted, filterDoctorApplicableDocuments } from '../src/lib/legalConsent.ts';

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

// ---------- filterDoctorApplicableDocuments (Doctor-only document filter) ----------
// GET /legal/documents is a single shared, role-agnostic registry (no
// audience filter server-side); this is the production response shape
// actually observed (9 total records: 4 mandatory Doctor + 1 shared optional
// + 4 Patient-only) that must be reduced to exactly the 5 Doctor-applicable
// ones before reaching a Doctor registration form.

const nineDocumentProductionResponse = [
  { document_type: 'DOCTOR_TERMS', version: '1.0', title: 'Doctor Terms', required: true },
  { document_type: 'PRIVACY_NOTICE', version: '1.0', title: 'Privacy Notice', required: true },
  { document_type: 'PROFESSIONAL_VERIFICATION_DECLARATION', version: '1.0', title: 'Professional Verification Declaration', required: true },
  { document_type: 'TELEMEDICINE_PRACTITIONER_POLICY', version: '1.0', title: 'Telemedicine Practitioner Policy', required: true },
  { document_type: 'MARKETING_COMMUNICATIONS', version: '1.0', title: 'Marketing Communications', required: false },
  { document_type: 'PATIENT_DATA_SHARING_CONSENT', version: '1.0', title: 'Patient Data Sharing Consent', required: true },
  { document_type: 'PATIENT_TERMS', version: '1.0', title: 'Patient Terms', required: true },
  { document_type: 'PATIENT_PRIVACY_NOTICE', version: '1.0', title: 'Patient Privacy Notice', required: true },
  { document_type: 'PATIENT_DATA_RIGHTS', version: '1.0', title: 'Patient Data Rights', required: true },
];

test('all 4 mandatory Doctor documents are retained from the real 9-document production response', () => {
  const result = filterDoctorApplicableDocuments(nineDocumentProductionResponse);
  for (const type of ['DOCTOR_TERMS', 'PRIVACY_NOTICE', 'PROFESSIONAL_VERIFICATION_DECLARATION', 'TELEMEDICINE_PRACTITIONER_POLICY']) {
    assert.ok(result.some(doc => doc.document_type === type), `expected ${type} to be retained`);
  }
});

test('the optional MARKETING_COMMUNICATIONS document is retained, not excluded', () => {
  const result = filterDoctorApplicableDocuments(nineDocumentProductionResponse);
  assert.ok(result.some(doc => doc.document_type === 'MARKETING_COMMUNICATIONS'));
});

test('all 4 Patient-only documents are excluded, not just PATIENT_DATA_SHARING_CONSENT', () => {
  const result = filterDoctorApplicableDocuments(nineDocumentProductionResponse);
  for (const type of ['PATIENT_DATA_SHARING_CONSENT', 'PATIENT_TERMS', 'PATIENT_PRIVACY_NOTICE', 'PATIENT_DATA_RIGHTS']) {
    assert.ok(!result.some(doc => doc.document_type === type), `expected ${type} to be excluded`);
  }
});

test('the real 9-document production response reduces to exactly 5 Doctor-applicable documents', () => {
  assert.equal(filterDoctorApplicableDocuments(nineDocumentProductionResponse).length, 5);
});

test('an unrecognized future document type is excluded by default (allow-list, not exclude-list)', () => {
  const result = filterDoctorApplicableDocuments([
    ...nineDocumentProductionResponse,
    { document_type: 'SOME_NEW_PATIENT_FEATURE_CONSENT', version: '1.0', title: 'New', required: true },
  ]);
  assert.ok(!result.some(doc => doc.document_type === 'SOME_NEW_PATIENT_FEATURE_CONSENT'));
});

test('an empty response filters to an empty list without throwing', () => {
  assert.deepEqual(filterDoctorApplicableDocuments([]), []);
});
