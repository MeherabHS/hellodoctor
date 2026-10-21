import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/validation/otp_validation.dart';

void main() {
  group('isValidOtpFormat', () {
    test('accepts a 6-digit code', () {
      expect(isValidOtpFormat('123456'), isTrue);
    });

    test('tolerates surrounding whitespace', () {
      expect(isValidOtpFormat('  123456  '), isTrue);
    });

    test('rejects a short code', () {
      expect(isValidOtpFormat('12345'), isFalse);
    });

    test('rejects a long code', () {
      expect(isValidOtpFormat('1234567'), isFalse);
    });

    test('rejects non-digit characters', () {
      expect(isValidOtpFormat('12a456'), isFalse);
    });

    test('rejects an empty string', () {
      expect(isValidOtpFormat(''), isFalse);
    });
  });
}
