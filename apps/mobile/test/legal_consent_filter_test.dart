import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/features/auth/legal_consent_page.dart';

// GET /legal/documents is a single shared, role-agnostic registry (no
// audience filter server-side); this is the real 9-record production
// response shape (4 mandatory Doctor + 1 shared optional + 4 Patient-only)
// that must reduce to exactly the 5 Doctor-applicable types. Regression
// coverage for the bug where only 'PATIENT_DATA_SHARING_CONSENT' was
// excluded by name, silently still showing the other three Patient-only
// types on this screen.
const _nineProductionDocumentTypes = [
  'DOCTOR_TERMS',
  'PRIVACY_NOTICE',
  'PROFESSIONAL_VERIFICATION_DECLARATION',
  'TELEMEDICINE_PRACTITIONER_POLICY',
  'MARKETING_COMMUNICATIONS',
  'PATIENT_DATA_SHARING_CONSENT',
  'PATIENT_TERMS',
  'PATIENT_PRIVACY_NOTICE',
  'PATIENT_DATA_RIGHTS',
];

void main() {
  test('all 4 mandatory Doctor document types are applicable', () {
    for (final type in [
      'DOCTOR_TERMS',
      'PRIVACY_NOTICE',
      'PROFESSIONAL_VERIFICATION_DECLARATION',
      'TELEMEDICINE_PRACTITIONER_POLICY',
    ]) {
      expect(isDoctorApplicableDocumentType(type), isTrue, reason: type);
    }
  });

  test('the optional MARKETING_COMMUNICATIONS document type is applicable', () {
    expect(isDoctorApplicableDocumentType('MARKETING_COMMUNICATIONS'), isTrue);
  });

  test(
      'all 4 Patient-only document types are rejected, not just '
      'PATIENT_DATA_SHARING_CONSENT', () {
    for (final type in [
      'PATIENT_DATA_SHARING_CONSENT',
      'PATIENT_TERMS',
      'PATIENT_PRIVACY_NOTICE',
      'PATIENT_DATA_RIGHTS',
    ]) {
      expect(isDoctorApplicableDocumentType(type), isFalse, reason: type);
    }
  });

  test(
      'an unrecognized future document type is rejected by default '
      '(allow-list, not exclude-list)', () {
    expect(isDoctorApplicableDocumentType('SOME_NEW_PATIENT_FEATURE_CONSENT'),
        isFalse);
  });

  test(
      'the real 9-type production response reduces to exactly 5 '
      'Doctor-applicable types', () {
    final applicable =
        _nineProductionDocumentTypes.where(isDoctorApplicableDocumentType);
    expect(applicable.length, 5);
  });
}
