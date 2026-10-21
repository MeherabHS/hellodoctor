import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/validation/kenya_phone.dart';

void main() {
  group('normalizeKenyaPhone', () {
    test('accepts a bare local 07... number and normalizes it', () {
      expect(normalizeKenyaPhone('0712345678'), '+254712345678');
    });

    test('accepts an already-international number unchanged', () {
      expect(normalizeKenyaPhone('+254712345678'), '+254712345678');
    });

    test('accepts an international number with spaces', () {
      expect(normalizeKenyaPhone('+254 712 345 678'), '+254712345678');
    });

    test('accepts a local number with hyphens', () {
      expect(normalizeKenyaPhone('0712-345-678'), '+254712345678');
    });

    test('accepts a bare local 01... number and normalizes it', () {
      expect(normalizeKenyaPhone('0112345678'), '+254112345678');
    });

    test('rejects by length, not by leading digit (8 national digits)', () {
      expect(normalizeKenyaPhone('+25420341371'), isNull);
      expect(normalizeKenyaPhone('020341371'), isNull);
    });

    test(
        'prefix-agnostic: a structurally complete 9-digit number starting '
        'with a non-7/1 digit still passes', () {
      expect(normalizeKenyaPhone('+254203413710'), '+254203413710');
      expect(normalizeKenyaPhone('0203413710'), '+254203413710');
    });

    test('rejects other wrong-length shapes', () {
      expect(normalizeKenyaPhone('+2542034137100'), isNull); // 10 digits
      expect(normalizeKenyaPhone('071234567'), isNull); // 8 digits
      expect(normalizeKenyaPhone('07123456789'), isNull); // 10 digits
    });

    test('rejects an obviously too-short number', () {
      expect(normalizeKenyaPhone('12345'), isNull);
    });

    test('rejects a bare country code with nothing after it', () {
      expect(normalizeKenyaPhone('+254'), isNull);
    });

    test('rejects a number containing letters', () {
      expect(normalizeKenyaPhone('0712abc678'), isNull);
    });

    test('rejects a non-Kenyan international number', () {
      expect(normalizeKenyaPhone('+8801712345678'), isNull);
    });

    test('accepts a bare international-shaped number with no leading plus', () {
      expect(normalizeKenyaPhone('254712345678'), '+254712345678');
    });
  });
}
