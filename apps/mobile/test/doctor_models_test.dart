import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';

void main() {
  test(
      'message chronology compares real instants across fractional seconds and UTC offsets',
      () {
    final values = [
      '2026-10-16T08:00:00.000002Z',
      '2026-10-16T08:00:00Z',
      '2026-10-16T11:00:00.000001+03:00'
    ];
    values.sort(compareMessageInstants);
    expect(values, [
      '2026-10-16T08:00:00Z',
      '2026-10-16T11:00:00.000001+03:00',
      '2026-10-16T08:00:00.000002Z'
    ]);
    expect(
        compareMessageInstants(
            '2026-10-16T08:00:00.123456Z', '2026-10-16T11:00:00.123456+03:00'),
        0);
  });

  test('appointment DTO preserves server lifecycle and contextual access', () {
    final a = AppointmentDto.fromJson({
      'id': 'a1',
      'status': 'COMPLETED',
      'doctor_lifecycle': 'DONE',
      'patient_id': 'p1',
      'start_time': '2026-10-05T10:00:00Z',
      'history_access_active': false
    });
    expect(a.id, 'a1');
    expect(a.lifecycle, 'DONE');
    expect(a.patientId, 'p1');
    expect(a.historyActive, isFalse);
  });

  test('slot, chat, message, question and wallet DTOs map backend fields', () {
    expect(SlotDto.fromJson({'id': 's', 'status': 'AVAILABLE'}).status,
        'AVAILABLE');
    expect(ChatDto.fromJson({'id': 'c', 'is_locked': true}).locked, isTrue);
    expect(
        MessageDto.fromJson({'id': 'm', 'content': 'hello', 'sent_at': 'now'})
            .content,
        'hello');
    expect(
        QnaDto.fromJson({'id': 'q', 'question_text': 'What?', 'status': 'OPEN'})
            .question,
        'What?');
    expect(WalletDto.fromJson({'total_gross': '100', 'final_net': '80'}).net,
        '80');
    final wallet = WalletDto.fromJson({
      'currency': 'KES',
      'finalized_unpaid': '80',
      'scheduled_unpaid': '80',
      'total_paid': '0'
    });
    expect(wallet.currency, 'KES');
    expect(wallet.finalizedUnpaid, '80');
    expect(wallet.scheduledUnpaid, '80');
    expect(wallet.totalPaid, '0');
  });

  test('profile DTO keeps verified fields available for read-only rendering',
      () {
    final p = ProfileDto.fromJson({
      'full_name': 'Dr A',
      'license_number': 'SYNTH-KE-0001',
      'current_hospital': 'Hospital',
      'primary_specialty': 'Medicine',
      'qualifications': ['MBBS'],
      'professional_verification_status': 'VERIFIED',
      'license_authority': 'KMPDC',
      'license_country': 'KE'
    });
    expect(p.name, 'Dr A');
    expect(p.hospital, 'Hospital');
    expect(p.qualifications, ['MBBS']);
    expect(p.verified, isTrue);
  });

  test('error mapper gives domain-specific safe messages', () {
    expect(const ApiException(401, '').userMessage, contains('session'));
    expect(const ApiException(403, '').userMessage, contains('permission'));
    expect(const ApiException(404, '').userMessage, contains('available'));
    expect(const ApiException(409, 'already booked').userMessage,
        'already booked');
    expect(const ApiException(422, '').userMessage, contains('check'));
    expect(const ApiException(429, '').userMessage, contains('Too many'));
    expect(const ApiException(500, '').userMessage, contains('temporary'));
  });

  test(
      'professional review uses explicit state and never treats rejected as pending',
      () {
    for (final status in ['PENDING', 'VERIFIED', 'REJECTED']) {
      final profile = ProfileDto.fromJson({
        'professional_verification_status': status,
        'is_verified': status != 'VERIFIED',
        'license_number': 'SYNTH-KE-0001',
        'license_authority': 'KMPDC',
        'license_country': 'KE',
      });
      expect(profile.verified, status == 'VERIFIED');
      expect(profile.licenseAuthority, 'KMPDC');
      expect(profile.licenseCountry, 'KE');
      expect(
          profile.verificationLabel,
          status == 'PENDING'
              ? 'Pending review'
              : status == 'REJECTED'
                  ? 'Rejected'
                  : 'Verified');
    }
    final oldCache = ProfileDto.fromJson({'is_verified': true});
    expect(oldCache.verified, isFalse);
    expect(oldCache.verificationLabel, 'Verification unavailable');
  });
}
