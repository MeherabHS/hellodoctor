import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/onboarding/account_onboarding_gate.dart';
import 'package:hellodoctor_mobile/features/onboarding/doctor_onboarding_page.dart';
import 'package:hellodoctor_mobile/l10n/app_localizations.dart';
import 'package:hellodoctor_mobile/core/l10n/doctor_locale.dart';

class _Store implements TokenStore {
  final values = <String, String>{};
  bool failReads = false;
  @override
  Future<String?> read(String key) async {
    if (failReads) throw StateError('storage unavailable');
    return values[key];
  }

  @override
  Future<void> write(String key, String value) async => values[key] = value;
  @override
  Future<void> delete(String key) async => values.remove(key);
  @override
  Future<void> clear() async => values.clear();
}

Widget _app(Widget home, {Locale? locale}) => MaterialApp(
    theme: DoctorTokens.theme(),
    locale: locale,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: home);

const _shell = Text('dashboard', key: ValueKey('dashboard'));

Finder _key(String k) => find.byKey(ValueKey(k));

void main() {
  setUp(() {
    final view = TestWidgetsFlutterBinding.ensureInitialized()
        .platformDispatcher
        .views
        .first;
    view.physicalSize = const Size(1080, 2340);
    view.devicePixelRatio = 3;
  });
  tearDown(() {
    final view =
        TestWidgetsFlutterBinding.instance.platformDispatcher.views.first;
    view.resetPhysicalSize();
    view.resetDevicePixelRatio();
  });

  testWidgets('four slides with Back, Next and Go to Dashboard',
      (tester) async {
    var done = 0;
    await tester.pumpWidget(_app(DoctorOnboardingPage(onDone: () => done++)));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('1 of 4'), findsOneWidget);
    expect(find.textContaining('Welcome, Doctor!'), findsOneWidget);
    expect(find.text('Get Started'), findsOneWidget);
    expect(find.text('AfyaMkononi'), findsOneWidget);
    expect(_key('doctor-onboarding-back'), findsNothing);

    await tester.tap(_key('doctor-onboarding-next'));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('2 of 4'), findsOneWidget);
    expect(find.text('Provide Care Online'), findsOneWidget);

    await tester.tap(_key('doctor-onboarding-back'));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('1 of 4'), findsOneWidget);

    for (var i = 2; i <= 4; i++) {
      await tester.tap(_key('doctor-onboarding-next'));
      await tester.pumpAndSettle();
      expect(find.bySemanticsLabel('$i of 4'), findsOneWidget);
    }
    expect(find.text('Manage Appointments'), findsNothing);
    expect(find.text('You’re All Set!'), findsOneWidget);
    // Signed-in wording: the account already exists.
    expect(find.text('Go to Dashboard'), findsOneWidget);
    expect(find.text('Create Doctor Account'), findsNothing);
    expect(_key('doctor-onboarding-next'), findsNothing);
    expect(done, 0);
    await tester.tap(_key('doctor-onboarding-finish'));
    expect(done, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Skip leaves from any slide', (tester) async {
    var done = 0;
    await tester.pumpWidget(_app(DoctorOnboardingPage(onDone: () => done++)));
    await tester.pumpAndSettle();
    await tester.tap(_key('doctor-onboarding-next'));
    await tester.pumpAndSettle();
    await tester.tap(_key('doctor-onboarding-skip'));
    expect(done, 1);
  });

  testWidgets('Swahili copy', (tester) async {
    await tester.pumpWidget(
        _app(DoctorOnboardingPage(onDone: () {}), locale: const Locale('sw')));
    await tester.pumpAndSettle();
    expect(find.text('Ruka'), findsOneWidget);
    expect(find.text('Anza'), findsOneWidget);
    expect(find.textContaining('Karibu, Daktari!'), findsOneWidget);
    expect(find.text('Kinga · Pima · Tibu · Dumisha Afya'), findsOneWidget);
  });

  testWidgets('fits other screen shapes, buttons above the navigation bar',
      (tester) async {
    for (final size in const [
      Size(720, 1280),
      Size(1080, 2400),
      Size(1440, 3200),
      Size(1600, 2560),
    ]) {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = size.width > 1200 ? 3.5 : 2.5;
      tester.view.viewPadding = const FakeViewPadding(bottom: 48 * 2.5);
      tester.view.padding = const FakeViewPadding(bottom: 48 * 2.5);
      await tester.pumpWidget(_app(DoctorOnboardingPage(onDone: () {})));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: '$size');
      final screen = tester.view.physicalSize / tester.view.devicePixelRatio;
      final inset =
          tester.view.viewPadding.bottom / tester.view.devicePixelRatio;
      final next = tester.getRect(_key('doctor-onboarding-next'));
      expect(next.bottom <= screen.height - inset, isTrue, reason: '$size');
      final skip = tester.getRect(_key('doctor-onboarding-skip'));
      expect(skip.right <= screen.width && skip.top >= 0, isTrue,
          reason: '$size');
    }
    tester.view.resetViewPadding();
    tester.view.resetPadding();
  });

  testWidgets('gate: once per doctor account, then remembered', (tester) async {
    final store = _Store();
    Widget gate(String? account) => _app(
        DoctorOnboardingGate(store: store, account: account, child: _shell));

    // Profile not loaded yet: the dashboard shows, no onboarding.
    await tester.pumpWidget(gate(null));
    await tester.pumpAndSettle();
    expect(_key('dashboard'), findsOneWidget);

    await tester.pumpWidget(gate('doctor-a'));
    await tester.pumpAndSettle();
    expect(find.byType(DoctorOnboardingPage), findsOneWidget);
    await tester.tap(_key('doctor-onboarding-skip'));
    await tester.pumpAndSettle();
    expect(_key('dashboard'), findsOneWidget);
    expect(store.values[doctorOnboardingSeenKey('doctor-a')], '1');

    await tester.pumpWidget(const SizedBox());
    await tester.pumpWidget(gate('doctor-a'));
    await tester.pumpAndSettle();
    expect(find.byType(DoctorOnboardingPage), findsNothing);

    // Another doctor on the same phone still gets it.
    await tester.pumpWidget(gate('doctor-b'));
    await tester.pumpAndSettle();
    expect(find.byType(DoctorOnboardingPage), findsOneWidget);
  });

  testWidgets('prompts wait until the first-login onboarding is done',
      (tester) async {
    final store = _Store();
    var released = false;
    await tester.pumpWidget(_app(DoctorOnboardingGate(
        store: store, account: 'doctor-a', child: _shell)));
    unawaited(AccountOnboardingGate.whenHidden().then((_) => released = true));
    await tester.pumpAndSettle();
    expect(find.byType(DoctorOnboardingPage), findsOneWidget);
    expect(released, isFalse);
    await tester.tap(_key('doctor-onboarding-skip'));
    await tester.pumpAndSettle();
    expect(released, isTrue);

    // A returning doctor: released as soon as the flag is read.
    await tester.pumpWidget(const SizedBox());
    var again = false;
    await tester.pumpWidget(_app(DoctorOnboardingGate(
        store: store, account: 'doctor-a', child: _shell)));
    unawaited(AccountOnboardingGate.whenHidden().then((_) => again = true));
    await tester.pumpAndSettle();
    expect(again, isTrue);
    expect(_key('dashboard'), findsOneWidget);
  });

  testWidgets('a storage error never blocks the dashboard', (tester) async {
    final store = _Store()..failReads = true;
    await tester.pumpWidget(_app(DoctorOnboardingGate(
        store: store, account: 'doctor-a', child: _shell)));
    await tester.pumpAndSettle();
    expect(_key('dashboard'), findsOneWidget);
  });

  test('signing out clears the session but keeps onboarding flags', () async {
    FlutterSecureStorage.setMockInitialValues({
      doctorOnboardingSeenKey('doctor-a'): '1',
      doctorOnboardingSeenKey('doctor-b'): '1',
      'access_token': 'a',
      'refresh_token': 'r',
    });
    final store = SecureTokenStore(const FlutterSecureStorage());
    await store.clear();
    expect(await store.read('access_token'), isNull);
    expect(await store.read('refresh_token'), isNull);
    expect(await store.read(doctorOnboardingSeenKey('doctor-a')), '1');
    expect(await store.read(doctorOnboardingSeenKey('doctor-b')), '1');
  });

  testWidgets('EN | SW beside Skip switches onboarding and saves the choice',
      (tester) async {
    final store = _Store();
    final locale = DoctorLocaleController(store);
    final saved = <String>[];
    locale.bindServerSync((code) async => saved.add(code));
    await tester.pumpWidget(DoctorLocaleScope(
      controller: locale,
      child: ListenableBuilder(
        listenable: locale,
        builder: (_, __) =>
            _app(DoctorOnboardingPage(onDone: () {}), locale: locale.locale),
      ),
    ));
    await tester.pumpAndSettle();
    expect(find.text('Skip'), findsOneWidget);
    expect(find.text('SW'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('onboarding-language-sw')));
    await tester.pumpAndSettle();
    expect(find.text('Ruka'), findsOneWidget);
    expect(find.textContaining('Karibu, Daktari!'), findsOneWidget);
    expect(store.values['doctor_locale_v1'], 'sw');
    await locale.serverSync;
    expect(saved, ['sw']);

    // Tapping the language already shown changes nothing.
    await tester.tap(find.byKey(const ValueKey('onboarding-language-sw')));
    await tester.pumpAndSettle();
    expect(saved, ['sw']);
  });
}
