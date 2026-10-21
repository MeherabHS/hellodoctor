import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/app.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_entry_page.dart';

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
  });
  for (final width in [320.0, 390.0]) {
    for (final inset in [0.0, 24.0, 34.0, 48.0]) {
      testWidgets('Patient shell geometry at $width dp / $inset system inset',
          (tester) async {
        tester.view.physicalSize = Size(width, 900);
        tester.view.devicePixelRatio = 1;
        tester.view.padding = FakeViewPadding(bottom: inset);
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPadding);
        final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
        dio.interceptors.add(InterceptorsWrapper(onRequest: (request, handler) {
          final profile = {
            'id': 'layout-profile',
            'user_id': 'layout-patient',
            'full_name': 'Layout Patient',
            'date_of_birth': '1996-01-01',
            'gender': 'OTHER'
          };
          final Object data = switch (request.path) {
            '/patients/me' => profile,
            '/patients/me/profiles' => [profile],
            '/notifications/unread-count' => {'unread_count': 0},
            '/patients/me/appointments' ||
            '/notifications' ||
            '/articles' =>
              [],
            _ => throw StateError('Unexpected layout request: ${request.path}'),
          };
          handler.resolve(Response(
              requestOptions: request,
              statusCode: 200,
              data: {'success': true, 'data': data}));
        }));
        addTearDown(() async {
          await tester.pumpWidget(const SizedBox.shrink());
          await tester.pump(const Duration(seconds: 6));
        });
        await tester.pumpWidget(HelloDoctorPatientApp(
            apiClientFactory: (store) =>
                ApiClient(dio: dio, tokenStore: store)));
        await tester.pumpAndSettle();
        // Layout-only setup; authentication is covered separately by OTP integration widgets.
        tester
            .element(find.byType(AuthEntryPage))
            .read<AuthCubit>()
            .emit(const SignedIn());
        await tester.pumpAndSettle();
        final nav = find.byType(PatientBottomNav);
        expect(nav, findsOneWidget);
        expect(tester.getRect(nav).bottom, 900 - inset);
        for (final label in ['Home', 'Services', 'Blogs', 'Account']) {
          final item = find.descendant(of: nav, matching: find.text(label));
          expect(item, findsOneWidget);
          expect(tester.getRect(item).bottom, lessThanOrEqualTo(900 - inset));
        }
        for (final target in find
            .descendant(of: nav, matching: find.byType(InkWell))
            .evaluate()) {
          expect(tester.getSize(find.byWidget(target.widget)).height,
              greaterThanOrEqualTo(48));
        }
        await tester
            .tap(find.descendant(of: nav, matching: find.text('Services')));
        await tester.pumpAndSettle();
        expect(tester.getRect(nav).bottom, 900 - inset);
        expect(tester.takeException(), isNull);
      });
    }
  }
}
