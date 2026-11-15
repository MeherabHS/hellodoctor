import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_entry_page.dart';
import 'package:hellodoctor_mobile_patient/features/auth/forgot_password_page.dart';

/// The illustrated auth screens must work on any screen: small and large
/// phones, a tablet, landscape, enlarged text and with the keyboard open.
/// "Work" here means no layout overflow, and the primary action can be
/// brought on screen and tapped.
void main() {
  // (width, height) in logical pixels.
  const sizes = <String, Size>{
    'small phone 320x568': Size(320, 568),
    'compact phone 360x640': Size(360, 640),
    'tall phone 411x923': Size(411, 923),
    'tablet 800x1280': Size(800, 1280),
    'landscape phone 740x360': Size(740, 360),
  };

  Future<void> pump(WidgetTester tester, Size size, double textScale,
      Widget Function(AuthCubit) page,
      {double keyboard = 0}) async {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    tester.view.viewInsets = FakeViewPadding(bottom: keyboard);
    tester.view.padding = const FakeViewPadding(top: 24);
    addTearDown(tester.view.reset);
    final auth = AuthCubit(ApiClient(dio: Dio()));
    addTearDown(auth.close);
    await tester.pumpWidget(MaterialApp(
        theme: PatientTokens.theme(),
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(textScale)),
            child: child!),
        home: page(auth)));
    await tester.pumpAndSettle();
  }

  Future<void> expectReachable(WidgetTester tester, String label) async {
    final button = find.widgetWithText(FilledButton, label);
    expect(button, findsOneWidget);
    await tester.ensureVisible(button);
    await tester.pumpAndSettle();
    // On screen and not covered by anything. It is not pressed here, because
    // pressing it would start a real sign-in request.
    expect(button.hitTestable(), findsOneWidget);
    final rect = tester.getRect(button);
    final screen = tester.view.physicalSize;
    final keyboard = tester.view.viewInsets.bottom;
    expect(rect.top, greaterThanOrEqualTo(0));
    expect(rect.bottom, lessThanOrEqualTo(screen.height - keyboard));
    expect(rect.left, greaterThanOrEqualTo(0));
    expect(rect.right, lessThanOrEqualTo(screen.width));
    expect(rect.height, greaterThanOrEqualTo(48));
    expect(tester.takeException(), isNull);
  }

  for (final entry in sizes.entries) {
    for (final scale in [1.0, 1.3]) {
      final where = '${entry.key}, text x$scale';

      testWidgets('log in fits: $where', (tester) async {
        await pump(tester, entry.value, scale, (a) => AuthEntryPage(auth: a));
        expect(tester.takeException(), isNull);
        expect(find.text('Welcome back'), findsOneWidget);
        await expectReachable(tester, 'Log in');
      });

      testWidgets('create account fits: $where', (tester) async {
        await pump(tester, entry.value, scale, (a) => AuthEntryPage(auth: a));
        final link = find.text('Create an account');
        await tester.ensureVisible(link);
        await tester.pumpAndSettle();
        await tester.tap(link);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('Create your AfyaMkononi account'), findsOneWidget);
        await expectReachable(tester, 'Create account');
      });

      testWidgets('forgot password fits: $where', (tester) async {
        await pump(
            tester, entry.value, scale, (a) => ForgotPasswordPage(auth: a));
        expect(tester.takeException(), isNull);
        // Exactly one primary action and one way back: no duplicates.
        expect(find.byType(FilledButton), findsOneWidget);
        expect(find.text('Back to log in'), findsOneWidget);
        await expectReachable(tester, 'Reset password');
      });
    }
  }

  testWidgets('log in still scrolls to its button with the keyboard open',
      (tester) async {
    await pump(tester, const Size(360, 640), 1.0, (a) => AuthEntryPage(auth: a),
        keyboard: 290);
    expect(tester.takeException(), isNull);
    await expectReachable(tester, 'Log in');
  });
}
