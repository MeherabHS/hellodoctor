import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/app.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/validation/kenya_phone.dart';
import 'package:hellodoctor_mobile/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile/features/auth/legal_consent_page.dart';

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

    test('accepts an already-international 01... number unchanged', () {
      expect(normalizeKenyaPhone('+254112345678'), '+254112345678');
    });

    test(
        'rejects by length, not by leading digit — the real bug report '
        '(8 national digits, not 9)', () {
      expect(normalizeKenyaPhone('+25420341371'), isNull);
      expect(normalizeKenyaPhone('020341371'), isNull);
    });

    test(
        'prefix-agnostic: a structurally complete 9-digit number starting '
        'with a non-7/1 digit now passes — proves the old prefix '
        'restriction is gone', () {
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

    test('rejects a too-short local number', () {
      expect(normalizeKenyaPhone('07123'), isNull);
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

  group('RegistrationPage phone validation', () {
    // Registration is two screens: account (name, email, password, phone)
    // then professional details. The phone is validated on the first.
    Future<void> enterAccountFields(WidgetTester tester, String phone) async {
      final fields = find.byType(TextField);
      await tester.enterText(fields.at(0), 'Dr Jane Doe'); // name
      await tester.enterText(fields.at(1), 'jane.doe@example.com'); // email
      await tester.enterText(fields.at(2), 'Abcdefg1!'); // password
      await tester.enterText(fields.at(3), phone);
      await tester.tap(find.text('Create Account'));
      await tester.pump();
    }

    Future<void> enterProfessionalFields(WidgetTester tester) async {
      final fields = find.byType(TextField);
      await tester.enterText(fields.at(0), 'General Medicine'); // specialty
      await tester.enterText(fields.at(1), 'KMPDC-12345'); // license
      await tester.enterText(fields.at(2), '5'); // experience
      await tester.enterText(fields.at(3), 'City Hospital'); // hospital
      await tester.enterText(fields.at(4), 'MBChB'); // qualifications
      await tester.enterText(fields.at(5), '123 Main St'); // address
      await tester.tap(find.text('Next'));
    }

    testWidgets(
        'invalid phone number blocks navigation and shows a field error',
        (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
      await tester.binding.setSurfaceSize(const Size(800, 2000));
      final auth = AuthCubit(ApiClient(dio: Dio()));
      addTearDown(auth.close);
      await tester.pumpWidget(MaterialApp(home: RegistrationPage(auth: auth)));

      await enterAccountFields(tester, '+25420341371');

      expect(
          find.textContaining('Enter a Kenyan phone number'), findsOneWidget);
      expect(find.text('Next'), findsNothing);
      expect(find.byType(LegalConsentPage), findsNothing);
    });

    testWidgets('valid phone number proceeds to the legal consent page',
        (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
      await tester.binding.setSurfaceSize(const Size(800, 2000));
      final auth = AuthCubit(ApiClient(dio: Dio()));
      addTearDown(auth.close);
      await tester.pumpWidget(MaterialApp(home: RegistrationPage(auth: auth)));

      await enterAccountFields(tester, '0712345678');
      await enterProfessionalFields(tester);
      // Not pumpAndSettle: LegalConsentPage's initState fires a real network
      // GET (`/legal/documents`) with no server behind it in this test, which
      // never settles. Pumping through just the MaterialPageRoute transition
      // is enough to prove the navigation itself happened.
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.byType(LegalConsentPage), findsOneWidget);
    });

    testWidgets(
        'a structurally valid 9-digit number with a non-7/1 prefix also '
        'proceeds — proves the prefix restriction was removed end-to-end',
        (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
      await tester.binding.setSurfaceSize(const Size(800, 2000));
      final auth = AuthCubit(ApiClient(dio: Dio()));
      addTearDown(auth.close);
      await tester.pumpWidget(MaterialApp(home: RegistrationPage(auth: auth)));

      await enterAccountFields(tester, '0203413710');
      await enterProfessionalFields(tester);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.byType(LegalConsentPage), findsOneWidget);
    });
  });
}
