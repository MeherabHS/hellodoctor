import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/app.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/widgets/patient_widgets.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_entry_page.dart';
import 'package:hellodoctor_mobile_patient/features/auth/email_otp_verify_page.dart';

const profile = {
  'id': 'patient-y-profile',
  'user_id': 'patient-y',
  'full_name': 'Patient Y',
  'date_of_birth': '1996-01-01',
  'gender': 'OTHER',
};
const documentTypes = [
  'PATIENT_DATA_SHARING_CONSENT',
  'PATIENT_TERMS',
  'PATIENT_PRIVACY_NOTICE',
  'PATIENT_DATA_RIGHTS',
];

Finder field(String label) => find.descendant(
    of: find.byWidgetPredicate((w) => w is PatientField && w.label == label),
    matching: find.byType(TextField));

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() {
    FlutterSecureStorage.setMockInitialValues({});
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(
        const MethodChannel('dev.fluttercommunity.plus/connectivity'),
        (_) async => ['wifi']);
    messenger.setMockMethodCallHandler(
        const MethodChannel('dev.fluttercommunity.plus/connectivity_status'),
        (_) async => null);
    messenger.setMockMethodCallHandler(
        const MethodChannel('hellodoctor/build_metadata'),
        (_) async => {'app_version': 'test', 'build_number': 'test'});
  });

  for (final registration in [true, false]) {
    for (final acceptedOtp in [true, false]) {
      testWidgets(
          '${registration ? 'registration' : 'login'} OTP '
          '${acceptedOtp ? 'opens main menu immediately and logout clears protected routes' : 'rejection stays on verification without granting access'}',
          (tester) async {
        tester.view.physicalSize = const Size(412, 1200);
        tester.view.devicePixelRatio = 1;
        tester.view.padding = const FakeViewPadding(bottom: 24);
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPadding);
        final requests = <RequestOptions>[];
        late ApiClient api;
        final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
        dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
          requests.add(r);
          dynamic data;
          switch (r.path) {
            case '/auth/patient/register':
            case '/auth/patient/login':
              data = {'challenge_token': 'challenge-y', 'expires_in': 600};
            case '/legal/documents':
              data = [
                for (final type in documentTypes)
                  {
                    'document_type': type,
                    'version': '1',
                    'title': type,
                    'body': 'Current test notice',
                  }
              ];
            case '/auth/patient/verify-otp':
              if (!acceptedOtp) {
                h.reject(
                    DioException(
                        requestOptions: r,
                        type: DioExceptionType.badResponse,
                        response: Response(
                            requestOptions: r,
                            statusCode: 401,
                            data: {'message': 'Invalid code'})),
                    true);
                return;
              }
              data = {
                'access_token': 'patient-y-access',
                'refresh_token': 'patient-y-refresh',
                'user': {
                  'id': 'patient-y',
                  'role': 'PATIENT',
                  'phone_number': '+254712345678'
                }
              };
            case '/patients/me':
              data = profile;
            case '/patients/me/profiles':
              data = [profile];
            case '/patients/me/appointments':
            case '/notifications':
            case '/articles':
              data = [];
            case '/notifications/unread-count':
              data = {'unread_count': 0};
            case '/auth/logout':
              data = null;
            default:
              throw StateError('Unexpected request: ${r.method} ${r.path}');
          }
          h.resolve(Response(
              requestOptions: r,
              statusCode: 200,
              data: {'success': true, 'data': data}));
        }));
        addTearDown(() async {
          await tester.pumpWidget(const SizedBox.shrink());
          await tester.pump(const Duration(seconds: 6));
        });
        await tester.pumpWidget(HelloDoctorPatientApp(
            apiClientFactory: (store) =>
                api = ApiClient(dio: dio, tokenStore: store)));
        await tester.pumpAndSettle();
        expect(find.byType(AuthEntryPage), findsOneWidget);
        if (registration) {
          await tester.tap(find.text('Create an account'));
          await tester.pumpAndSettle();
          await tester.enterText(field('Full name'), 'Patient Y');
          await tester.tap(field('Date of birth'));
          await tester.pumpAndSettle();
          await tester.tap(find.text('OK'));
          await tester.pumpAndSettle();
          await tester.enterText(field('Phone number'), '0712345678');
        }
        await tester.enterText(field('Email address'), 'patient.y@example.com');
        await tester.enterText(field('Password'), 'PatientPassword!2026');
        final submit = find.widgetWithText(
            FilledButton, registration ? 'Continue' : 'Sign in');
        await tester.ensureVisible(submit);
        await tester.tap(submit);
        await tester.pumpAndSettle();
        expect(find.byType(EmailOtpVerifyPage), findsOneWidget);
        expect(find.byType(PatientShell), findsNothing);
        expect(requests.where((r) => r.path.startsWith('/patients/')), isEmpty);
        if (registration) {
          expect(find.byType(Checkbox), findsNWidgets(4));
          for (var index = 0; index < 4; index++) {
            final checkbox = find.byType(Checkbox).at(index);
            await tester.ensureVisible(checkbox);
            await tester.tap(checkbox);
            await tester.pump();
          }
        }
        await tester.enterText(field('Verification code'), '123456');
        final verify = find.widgetWithText(FilledButton, 'Verify and continue');
        await tester.ensureVisible(verify);
        await tester.tap(verify);
        await tester.pumpAndSettle();
        final verification =
            requests.singleWhere((r) => r.path == '/auth/patient/verify-otp');
        expect(verification.data['challenge_token'], 'challenge-y');
        expect(verification.data['otp_code'], '123456');
        if (registration) {
          expect(verification.data['accepted_documents'], [
            for (final type in documentTypes) {'type': type, 'version': '1'}
          ]);
        }
        if (!acceptedOtp) {
          expect(find.byType(EmailOtpVerifyPage), findsOneWidget);
          expect(find.text('The verification code is invalid or expired.'),
              findsWidgets);
          expect(find.byType(PatientShell), findsNothing);
          expect(await api.read('access_token'), isNull);
          expect(
              requests.where((r) => r.path.startsWith('/patients/')), isEmpty);
        } else {
          // No app restart, manual back navigation or local success substitute.
          expect(await api.read('access_token'), 'patient-y-access');
          expect(find.byType(PatientShell), findsOneWidget);
          expect(find.byType(PatientBottomNav), findsOneWidget);
          expect(tester.getRect(find.byType(PatientBottomNav)).bottom,
              lessThanOrEqualTo(1176));
          expect(find.byType(EmailOtpVerifyPage, skipOffstage: false),
              findsNothing);
          expect(find.byType(AuthEntryPage, skipOffstage: false), findsNothing);
          final context = tester.element(find.byType(PatientShell));
          expect(Navigator.of(context).canPop(), isFalse);
          final auth = context.read<AuthCubit>();
          Navigator.of(context).push(MaterialPageRoute<void>(
              builder: (_) =>
                  const Scaffold(body: Text('Private consultation detail'))));
          await tester.pumpAndSettle();
          expect(find.text('Private consultation detail'), findsOneWidget);
          final logout = auth.logout();
          await tester.pumpAndSettle();
          await logout;
          expect(find.byType(AuthEntryPage), findsOneWidget);
          expect(find.byType(PatientShell, skipOffstage: false), findsNothing);
          expect(find.text('Private consultation detail', skipOffstage: false),
              findsNothing);
          expect(await api.read('access_token'), isNull);
          expect(
              Navigator.of(tester.element(find.byType(AuthEntryPage))).canPop(),
              isFalse);
        }
        expect(tester.takeException(), isNull);
      });
    }
  }
}
