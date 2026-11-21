import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/onboarding/onboarding_page.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';
import 'package:provider/provider.dart';
import 'package:hellodoctor_mobile_patient/core/l10n/locale_controller.dart';

class _Store implements TokenStore {
  _Store([Map<String, String>? values]) : values = values ?? {};
  final Map<String, String> values;
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

Widget _app(Widget home) => MaterialApp(
    theme: PatientTokens.theme(),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: home);

void main() {
  setUp(() {
    final binding = TestWidgetsFlutterBinding.ensureInitialized();
    binding.platformDispatcher.views.first.physicalSize =
        const Size(1080, 2340);
    binding.platformDispatcher.views.first.devicePixelRatio = 3;
  });
  tearDown(() {
    final view =
        TestWidgetsFlutterBinding.instance.platformDispatcher.views.first;
    view.resetPhysicalSize();
    view.resetDevicePixelRatio();
  });

  testWidgets('four slides; Next advances and Get Started finishes',
      (tester) async {
    var done = 0;
    await tester.pumpWidget(_app(OnboardingPage(onDone: () => done++)));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('1 of 4'), findsOneWidget);
    expect(find.text('Next'), findsOneWidget);
    expect(find.textContaining('Welcome to'), findsOneWidget);
    expect(find.text('AfyaMkononi'), findsOneWidget);
    const titles = [
      'Learn & Stay Safe',
      'Get Confidential Support',
      'Access Care, Anytime'
    ];
    for (var i = 2; i <= 4; i++) {
      await tester.tap(find.byKey(const ValueKey('onboarding-next')));
      await tester.pumpAndSettle();
      expect(find.bySemanticsLabel('$i of 4'), findsOneWidget);
      expect(find.text(titles[i - 2]), findsOneWidget);
    }
    expect(find.text('Get Started'), findsOneWidget);
    expect(find.text('Next'), findsNothing);
    expect(done, 0);
    await tester.tap(find.byKey(const ValueKey('onboarding-next')));
    expect(done, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Skip leaves onboarding from any slide', (tester) async {
    var done = 0;
    await tester.pumpWidget(_app(OnboardingPage(onDone: () => done++)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('onboarding-next')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('onboarding-skip')));
    expect(done, 1);
  });

  testWidgets('the gate shows onboarding once, then remembers it',
      (tester) async {
    final store = _Store();
    await tester.pumpWidget(_app(OnboardingGate(
        store: store, child: const Text('sign-in', key: ValueKey('sign-in')))));
    await tester.pumpAndSettle();
    expect(find.byType(OnboardingPage), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('onboarding-skip')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sign-in')), findsOneWidget);
    expect(store.values[onboardingSeenKey], '1');

    await tester.pumpWidget(const SizedBox());
    await tester.pumpWidget(_app(OnboardingGate(
        store: store, child: const Text('sign-in', key: ValueKey('sign-in')))));
    await tester.pumpAndSettle();
    expect(find.byType(OnboardingPage), findsNothing);
    expect(find.byKey(const ValueKey('sign-in')), findsOneWidget);
  });

  testWidgets('fits phones of other shapes without overflow', (tester) async {
    for (final size in const [
      Size(720, 1280),
      Size(1080, 2400),
      Size(1440, 3200),
      Size(1600, 2560),
    ]) {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = size.width > 1200 ? 3.5 : 2.5;
      await tester.pumpWidget(_app(OnboardingPage(onDone: () {})));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: '$size');
      final skip =
          tester.getRect(find.byKey(const ValueKey('onboarding-skip')));
      final next =
          tester.getRect(find.byKey(const ValueKey('onboarding-next')));
      final screen = tester.view.physicalSize / tester.view.devicePixelRatio;
      expect(skip.right <= screen.width && skip.top >= 0, isTrue,
          reason: '$size');
      expect(next.bottom <= screen.height && next.left >= 0, isTrue,
          reason: '$size');
    }
  });

  test('signing out clears the session but not the onboarding flag', () async {
    FlutterSecureStorage.setMockInitialValues({
      onboardingSeenKey: '1',
      'access_token': 'a',
      'refresh_token': 'r',
    });
    final store = SecureTokenStore(const FlutterSecureStorage());
    await store.clear();
    expect(await store.read('access_token'), isNull);
    expect(await store.read('refresh_token'), isNull);
    expect(await store.read(onboardingSeenKey), '1');

    FlutterSecureStorage.setMockInitialValues({'access_token': 'a'});
    await store.clear();
    expect(await store.read(onboardingSeenKey), isNull);
  });

  testWidgets('a storage error never blocks sign-in', (tester) async {
    final store = _Store()..failReads = true;
    await tester.pumpWidget(_app(OnboardingGate(
        store: store, child: const Text('sign-in', key: ValueKey('sign-in')))));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sign-in')), findsOneWidget);
  });

  testWidgets('Swahili copy is used when the app is in Swahili',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        locale: const Locale('sw'),
        theme: PatientTokens.theme(),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: OnboardingPage(onDone: () {})));
    await tester.pumpAndSettle();
    expect(find.text('Ruka'), findsOneWidget);
    expect(find.text('Endelea'), findsOneWidget);
    expect(find.bySemanticsLabel('1 kati ya 4'), findsOneWidget);
  });

  testWidgets('EN | SW beside Skip switches onboarding and keeps the choice',
      (tester) async {
    final store = _Store();
    final locale = LocaleController(store);
    await tester.pumpWidget(ListenableBuilder(
      listenable: locale,
      builder: (_, __) => ChangeNotifierProvider<LocaleController>.value(
        value: locale,
        child: MaterialApp(
            theme: PatientTokens.theme(),
            locale: locale.locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: OnboardingPage(onDone: () {})),
      ),
    ));
    await tester.pumpAndSettle();
    expect(find.text('Skip'), findsOneWidget);
    expect(find.text('EN'), findsOneWidget);
    expect(find.text('SW'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('onboarding-language-sw')));
    await tester.pumpAndSettle();
    expect(find.text('Ruka'), findsOneWidget);
    expect(find.text('Endelea'), findsOneWidget);
    expect(store.values['patient_locale_v1'], 'sw');

    await tester.tap(find.byKey(const ValueKey('onboarding-language-en')));
    await tester.pumpAndSettle();
    expect(find.text('Skip'), findsOneWidget);
    expect(store.values['patient_locale_v1'], 'en');
  });

  test('a language chosen while signed out is saved after sign-in', () async {
    final locale = LocaleController(_Store());
    var signedIn = false;
    final synced = <String>[];
    locale.bindBackendSync((code) async {
      if (!signedIn) throw StateError('signed out');
      synced.add(code);
    });
    await locale.setLocale('sw');
    await Future<void>.delayed(Duration.zero);
    expect(synced, isEmpty);
    signedIn = true;
    locale.flushPendingSync();
    await Future<void>.delayed(Duration.zero);
    expect(synced, ['sw']);
    locale.flushPendingSync();
    await Future<void>.delayed(Duration.zero);
    expect(synced, ['sw'], reason: 'nothing pending once saved');
  });
}
